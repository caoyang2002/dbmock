//! engine.rs — orchestrates FK-safe, dependency-ordered mock data generation.
//!
//! FK pool strategy (in priority order):
//!   1. Already generated this session → RETURNING IDs
//!   2. Exists in DB but not requested → query_ids()
//!   3. Nowhere → nullable FK → NULL; non-nullable → DB error surfaced
//!
//! Self-referencing FKs (e.g. boards.parent_id → boards.id):
//!   All set to NULL so every row is a valid root node.
//!
//! Config integration:
//!   When a MockConfig is provided, each table's FieldConfig overrides are
//!   passed to build_insert_batches. FieldKind::Skip columns are excluded
//!   from the INSERT column list entirely.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use indicatif::{ProgressBar, ProgressStyle};

use crate::core::driver::DatabaseDriver;
use crate::core::generator::{DataGenerator, GenerationReport};
use crate::core::schema::Schema;
use crate::errors::{MockerError, Result};
use crate::fieldconfig::generate::reset_unique_counters;
use crate::fieldconfig::infer::MockConfig;
use crate::generator::batch::build_insert_batches;
use crate::generator::dependency::topological_sort;

// ─────────────────────────────────────────────────────────────────────────────

pub struct MockEngine {
    driver: Arc<dyn DatabaseDriver>,
}

impl MockEngine {
    pub fn new(driver: Arc<dyn DatabaseDriver>) -> Self {
        Self { driver }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[async_trait]
impl DataGenerator for MockEngine {
    async fn generate(
        &self,
        schema: &Schema,
        row_counts: &HashMap<String, usize>,
        dry_run: bool,
        mock_config: Option<&MockConfig>,
    ) -> Result<GenerationReport> {
        // Reset unique/sequence counters for a fresh run
        reset_unique_counters();

        // Validate requested tables
        let requested: Vec<String> = row_counts.keys().cloned().collect();
        for t in &requested {
            if schema.get_table(t).is_none() {
                return Err(MockerError::TableNotFound { table: t.clone() });
            }
        }

        let sorted  = topological_sort(schema, &requested)?;
        let db_type = self.driver.db_type().to_string();
        let mut report = GenerationReport::default();

        // ── pre-load FK pools from DB ─────────────────────────────────────────
        let mut fk_pools: HashMap<String, Vec<String>> = HashMap::new();
        let referenced = collect_referenced_tables(schema, &requested);

        if !dry_run {
            for ref_table in &referenced {
                let pk_col = pk_col_name(schema, ref_table);
                match self.driver.query_ids(ref_table, &pk_col, 2000).await {
                    Ok(ids) if !ids.is_empty() => {
                        fk_pools.insert(ref_table.clone(), ids);
                    }
                    _ => {} // empty or missing — handled per-column in batch
                }
            }
        }

        // ── progress bar ──────────────────────────────────────────────────────
        let bar = ProgressBar::new(sorted.len() as u64);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("##-"),
        );

        // ── main loop ─────────────────────────────────────────────────────────
        for table_name in &sorted {
            let row_count = *row_counts.get(table_name).unwrap_or(&0);
            if row_count == 0 {
                bar.inc(1);
                continue;
            }

            let ts = schema.get_table(table_name).unwrap();
            bar.set_message(format!("{} → {} rows", table_name, row_count));

            // Self-referencing FK columns → always NULL
            let self_ref_cols: Vec<String> = ts
                .foreign_keys
                .iter()
                .filter(|fk| fk.referenced_table == *table_name)
                .map(|fk| fk.column.clone())
                .collect();

            // Per-table field config (if a mock_config was provided)
            let table_cfg = mock_config.and_then(|mc| mc.get(table_name));

            let stmts = build_insert_batches(
                ts,
                row_count,
                &db_type,
                &fk_pools,
                &self_ref_cols,
                table_cfg,
            );

            if dry_run {
                for s in &stmts {
                    println!("{};", s);
                    report.sql_statements.push(s.clone());
                }
                fk_pools.insert(table_name.clone(), synthetic_int_pool(row_count));
            } else {
                let pk_col = pk_col_name(schema, table_name);

                match self
                    .driver
                    .execute_batch_returning_ids(stmts, table_name, &pk_col)
                    .await
                {
                    Ok(ids) => {
                        report.total_rows_inserted += ids.len() as u64;
                        fk_pools.insert(table_name.clone(), ids);
                    }
                    Err(e) => {
                        let msg = format!("Error inserting into {}: {}", table_name, e);
                        eprintln!("⚠️  {}", msg);
                        report.errors.push(msg);
                        // Rescue whatever actually landed
                        if let Ok(ids) = self
                            .driver
                            .query_ids(table_name, &pk_col, row_count * 2)
                            .await
                        {
                            if !ids.is_empty() {
                                fk_pools.insert(table_name.clone(), ids);
                            }
                        }
                    }
                }
            }

            report.tables_processed += 1;
            bar.inc(1);
        }

        bar.finish_with_message("Done!");
        Ok(report)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn collect_referenced_tables(schema: &Schema, requested: &[String]) -> Vec<String> {
    let mut refs = std::collections::HashSet::new();
    for tname in requested {
        if let Some(ts) = schema.get_table(tname) {
            for fk in &ts.foreign_keys {
                if fk.referenced_table != *tname {
                    refs.insert(fk.referenced_table.clone());
                }
            }
        }
    }
    refs.into_iter().collect()
}

fn pk_col_name(schema: &Schema, table_name: &str) -> String {
    schema
        .get_table(table_name)
        .and_then(|ts| ts.primary_keys.first().cloned())
        .unwrap_or_else(|| "id".to_string())
}

fn synthetic_int_pool(count: usize) -> Vec<String> {
    (1..=count).map(|i| i.to_string()).collect()
}
