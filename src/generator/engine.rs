//! engine.rs — orchestrates FK-safe, dependency-ordered mock data generation.
//!
//! FK pool strategy (in priority order for each referenced table):
//!
//!   1. **Already generated this session** → pool built from RETURNING IDs.
//!   2. **Exists in DB but not in this session** → query real IDs via query_ids().
//!   3. **Nowhere** (table not in DB yet and not requested) → empty pool.
//!      FK columns that are nullable → NULL; non-nullable → error surfaced.
//!
//! Self-referencing FKs (parent_id → same table):
//!   • First batch is generated with self-ref column forced to NULL (allowed
//!     only when the column is nullable, which parent_id always is in practice).
//!   • After inserting, real IDs are queried and subsequent rows can reference
//!     them.  Since we generate everything in one shot we just set all
//!     self-ref FKs to NULL — this is valid for tree roots.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use indicatif::{ProgressBar, ProgressStyle};

use crate::core::driver::DatabaseDriver;
use crate::core::generator::{DataGenerator, GenerationReport};
use crate::core::schema::{Schema, TableSchema};
use crate::errors::{MockerError, Result};
use crate::fieldconfig;
use crate::fieldconfig::MockConfig;
use crate::generator::batch::build_insert_batches;
use crate::generator::dependency::topological_sort;

// ─────────────────────────────────────────────────────────────────────────────

pub struct MockEngine {
    driver: Arc<dyn DatabaseDriver>,
    mock_config: Option<MockConfig>,
}

impl MockEngine {
    pub fn new(driver: Arc<dyn DatabaseDriver>,mock_config: Option<MockConfig>) -> Self {
        Self { driver,mock_config }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[async_trait]
impl DataGenerator for MockEngine {
    async fn generate(
        &self,
        schema: &Schema,
        row_counts: &HashMap<String, usize>,
        preview: bool,
        _mock_config: Option<&MockConfig>,


    ) -> Result<GenerationReport> {
        // Validate all requested tables exist in schema
        let requested: Vec<String> = row_counts.keys().cloned().collect();
        for t in &requested {
            if schema.get_table(t).is_none() {
                return Err(MockerError::TableNotFound { table: t.clone() });
            }
        }

        let sorted   = topological_sort(schema, &requested)?;
        let db_type  = self.driver.db_type().to_string();
        let mut report = GenerationReport::default();

        // ── FK id pools ───────────────────────────────────────────────────────
        // Maps: table_name → Vec<SQL literal> of known PK values
        let mut fk_pools: HashMap<String, Vec<String>> = HashMap::new();

        // Pre-load real IDs from the DB for every table that has FK
        // relationships with our requested tables (whether requested or not).
        let referenced_tables = collect_referenced_tables(schema, &requested);
        if !preview {
            for ref_table in &referenced_tables {
                let pk_col = pk_col_name(schema, ref_table);
                match self.driver.query_ids(ref_table, &pk_col, 2000).await {
                    Ok(ids) if !ids.is_empty() => {
                        fk_pools.insert(ref_table.clone(), ids);
                    }
                    Ok(_) => {
                        // Table exists but is empty — pool stays absent.
                    }
                    Err(_) => {
                        // Table may not exist yet; that is fine.
                    }
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

            // Detect self-referencing FKs (parent_id → same table).
            // These are passed to batch builder so it can emit NULL for them.
            let self_ref_cols: Vec<String> = ts
                .foreign_keys
                .iter()
                .filter(|fk| fk.referenced_table == *table_name)
                .map(|fk| fk.column.clone())
                .collect();

            let stmts = build_insert_batches(
                ts,
                row_count,
                &db_type,
                &fk_pools,
                &self_ref_cols,
                None,
            );

            if preview {
                for s in &stmts {
                    println!("{};", s);
                    report.sql_statements.push(s.clone());
                }
                // For dry-run, build a synthetic sequential pool so downstream
                // FK tables render correctly.
                let pool = synthetic_int_pool(row_count);
                fk_pools.insert(table_name.clone(), pool);
            } else {
                let pk_col = pk_col_name(schema, table_name);

                // Execute with RETURNING to capture actual inserted IDs.
                match self
                    .driver
                    .execute_batch_returning_ids(stmts, table_name, &pk_col)
                    .await
                {
                    Ok(ids) => {
                        let inserted = ids.len() as u64;
                        report.total_rows_inserted += inserted;
                        // Use real IDs for downstream FKs.
                        fk_pools.insert(table_name.clone(), ids);
                    }
                    Err(e) => {
                        let msg = format!("Error inserting into {}: {}", table_name, e);
                        eprintln!("⚠️  {}", msg);
                        report.errors.push(msg);
                        // Even on error, try to fetch whatever landed.
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

/// Collect all tables that are FK-referenced by any of the requested tables
/// (including transitively through the schema, but one level is enough for
/// pool pre-loading).
fn collect_referenced_tables(schema: &Schema, requested: &[String]) -> Vec<String> {
    let mut refs: std::collections::HashSet<String> = std::collections::HashSet::new();
    for table_name in requested {
        if let Some(ts) = schema.get_table(table_name) {
            for fk in &ts.foreign_keys {
                // Skip self-references (handled separately)
                if fk.referenced_table != *table_name {
                    refs.insert(fk.referenced_table.clone());
                }
            }
        }
    }
    refs.into_iter().collect()
}

/// Return the first PK column name, or "id" as fallback.
fn pk_col_name(schema: &Schema, table_name: &str) -> String {
    schema
        .get_table(table_name)
        .and_then(|ts| ts.primary_keys.first().cloned())
        .unwrap_or_else(|| "id".to_string())
}

fn synthetic_int_pool(count: usize) -> Vec<String> {
    (1..=count).map(|i| i.to_string()).collect()
}
