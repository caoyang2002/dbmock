use crate::core::driver::DatabaseDriver;
use crate::core::generator::{DataGenerator, GenerationReport};
use crate::core::schema::Schema;
use crate::errors::{MockerError, Result};
use crate::generator::batch::build_insert_batches;
use crate::generator::dependency::topological_sort;
use crate::generator::value::generate_value;
use async_trait::async_trait;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::sync::Arc;

pub struct MockEngine {
    driver: Arc<dyn DatabaseDriver>,
}

impl MockEngine {
    pub fn new(driver: Arc<dyn DatabaseDriver>) -> Self {
        Self { driver }
    }
}

#[async_trait]
impl DataGenerator for MockEngine {
    async fn generate(
        &self,
        schema: &Schema,
        row_counts: &HashMap<String, usize>,
        dry_run: bool,
    ) -> Result<GenerationReport> {
        let requested_tables: Vec<String> = row_counts.keys().cloned().collect();

        // Validate all requested tables exist in schema
        for table in &requested_tables {
            if schema.get_table(table).is_none() {
                return Err(MockerError::TableNotFound {
                    table: table.clone(),
                });
            }
        }

        // Topological sort
        let sorted_tables = topological_sort(schema, &requested_tables)?;

        let mut report = GenerationReport::default();
        let db_type = self.driver.db_type();

        // Track generated PKs for FK resolution
        // For each table, we store a pool of "representative" PK values we'll pretend exist
        let mut fk_id_pools: HashMap<String, Vec<String>> = HashMap::new();

        // Also populate id pools for tables NOT being generated, using sequential ints
        for table_schema in &schema.tables {
            if !requested_tables.contains(&table_schema.name) {
                // Assume some rows exist (use 1..=100 as pool)
                let pool: Vec<String> = (1..=100).map(|i| i.to_string()).collect();
                fk_id_pools.insert(table_schema.name.clone(), pool);
            }
        }

        let bar = ProgressBar::new(sorted_tables.len() as u64);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("##-"),
        );

        for table_name in &sorted_tables {
            let row_count = *row_counts.get(table_name).unwrap_or(&0);
            if row_count == 0 {
                continue;
            }

            let table_schema = schema.get_table(table_name).unwrap();
            bar.set_message(format!("Generating {} rows for {}", row_count, table_name));

            let statements = build_insert_batches(table_schema, row_count, db_type, &fk_id_pools);

            if dry_run {
                for stmt in &statements {
                    println!("{};", stmt);
                    report.sql_statements.push(stmt.clone());
                }
            } else {
                match self.driver.execute_batch(statements.clone()).await {
                    Ok(rows) => {
                        report.total_rows_inserted += rows;
                    }
                    Err(e) => {
                        let msg = format!("Error inserting into {}: {}", table_name, e);
                        eprintln!("⚠️  {}", msg);
                        report.errors.push(msg);
                    }
                }
            }

            // Build id pool for this table (sequential since we don't do RETURNING)
            let pk_pool: Vec<String> = (1..=(row_count as i64)).map(|i| i.to_string()).collect();
            fk_id_pools.insert(table_name.clone(), pk_pool);

            report.tables_processed += 1;
            bar.inc(1);
        }

        bar.finish_with_message("Done!");
        Ok(report)
    }
}
