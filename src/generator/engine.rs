//! engine.rs — high-throughput FK-safe mock data generation.
//!
//! Performance design:
//!   • Rows are generated in-memory in CHUNK_SIZE batches (default 5_000 rows
//!     per chunk = 10 INSERT statements of 500 rows each).
//!   • Each chunk is dispatched to a pool of WORKER_COUNT concurrent tokio
//!     tasks that execute the INSERT statements in parallel.
//!   • FK ID pools are capped at FK_POOL_CAP entries — we never need to store
//!     millions of IDs; a random sample is enough for FK distribution.
//!   • RETURNING is used only to seed the FK pool for downstream tables, and
//!     only on the *first* chunk (we stop collecting once the pool is full).
//!   • The progress bar is updated per-row so the user sees live throughput.
use std::cmp::max;
use std::collections::HashMap;
use std::sync::Arc;

use crate::logger::print_sample_table;
use async_trait::async_trait;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::sync::Semaphore;

use crate::config::tuning;
use crate::config::TuningParams;
use crate::core::driver::DatabaseDriver;
use crate::core::generator::{DataGenerator, GenerationReport};
use crate::core::schema::Schema;
use crate::errors::{MockerError, Result};
use crate::fieldconfig::generate::reset_unique_counters;
use crate::fieldconfig::infer::MockConfig;
use crate::generator::batch::build_insert_batches;
use crate::generator::batch::generate_sample_rows;
use crate::generator::batch::UniqueConstraintTracker;
use crate::generator::dependency::topological_sort;

// ── tuning constants ──────────────────────────────────────────────────────────

//
/// Rows per INSERT statement.
// const INSERT_ROWS: usize = 5_000;

// /// How many INSERT statements to send concurrently per table.
// const CONCURRENCY: usize = 10;

// /// Maximum number of FK IDs we keep in memory per referenced table.
// /// A random sample of this size gives good distribution.
// const FK_POOL_CAP: usize = 8_000;

// ─────────────────────────────────────────────────────────────────────────────

pub struct MockEngine {
    driver: Arc<dyn DatabaseDriver>,
    insert_rows: usize,
    concurrency: usize,
    fk_pool_cap: usize,
    debug: bool,
}

impl MockEngine {
    pub fn new(driver: Arc<dyn DatabaseDriver>, tuning: TuningParams) -> Self {
        Self {
            driver,
            insert_rows: tuning.insert_rows,
            concurrency: tuning.concurrency,
            fk_pool_cap: tuning.fk_pool_cap,
            debug: false,
        }
    }
    pub fn set_debug(&mut self, enabled: bool) {
        self.debug = enabled;
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
        mock_config: Option<&MockConfig>,
    ) -> Result<GenerationReport> {
        if self.debug {
            eprintln!("[DEBUG] generate() called");
            eprintln!("[DEBUG] preview = {}", preview);
            eprintln!("[DEBUG] row_counts = {:?}", row_counts);
            eprintln!(
                "[DEBUG] concurrency = {}, insert_rows = {}, fk_pool_cap = {}",
                self.concurrency, self.insert_rows, self.fk_pool_cap
            );
        }

        reset_unique_counters();

        let requested: Vec<String> = row_counts.keys().cloned().collect();
        for t in &requested {
            if schema.get_table(t).is_none() {
                return Err(MockerError::TableNotFound { table: t.clone() });
            }
        }

        let sorted = topological_sort(schema, &requested)?;
        let db_type = self.driver.db_type().to_string();
        let mut report = GenerationReport::default();
        let mut fk_pools: HashMap<String, Vec<String>> = HashMap::new();

        // ── pre-load FK pools for tables not being inserted ───────────────────
        if !preview {
            let referenced = collect_referenced_tables(schema, &requested);
            if self.debug {
                eprintln!(
                    "[DEBUG] Referenced tables (not being inserted): {:?}",
                    referenced
                );
            }
            for ref_table in referenced {
                if requested.contains(&ref_table) {
                    continue;
                }
                let pk_col = pk_col_name(schema, &ref_table);
                if self.debug {
                    eprintln!(
                        "[DEBUG] Pre-loading FK pool for '{}' (pk_col={})",
                        ref_table, pk_col
                    );
                }
                match self
                    .driver
                    .query_ids(&ref_table, &pk_col, self.fk_pool_cap)
                    .await
                {
                    Ok(ids) => {
                        if self.debug {
                            eprintln!("[DEBUG]   Loaded {} ids from '{}'", ids.len(), ref_table);
                        }
                        if !ids.is_empty() {
                            fk_pools.insert(ref_table, ids);
                        } else if self.debug {
                            eprintln!("[DEBUG]   Table '{}' is empty (0 rows)", ref_table);
                        }
                    }
                    Err(e) => {
                        if self.debug {
                            eprintln!("[DEBUG]   Failed to query ids from '{}': {}", ref_table, e);
                        }
                    }
                }
            }
        }

        // ── multi-progress: one bar per table + one overall ───────────────────
        let mp = MultiProgress::new();
        let bar_style = ProgressStyle::default_bar()
            .template(
                "  {msg:<30} [{bar:35.cyan/blue}] {pos:>9}/{len:9} rows  {per_sec:>12}  {elapsed}",
            )
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏ ");

        let total_rows: u64 = row_counts.values().map(|&n| n as u64).sum();
        let overall_style = ProgressStyle::default_bar()
            .template("  {msg:<30} [{bar:35.green/white}] {pos:>9}/{len:9} rows  {per_sec:>12}  {elapsed}")
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏ ");

        let overall = mp.add(ProgressBar::new(total_rows));
        overall.set_style(overall_style);
        overall.set_message("total");

        // ── per-table loop ────────────────────────────────────────────────────
        for table_name in &sorted {
            let ts = schema.get_table(table_name).unwrap();
            let constraint_tracker = if !ts.unique_constraints.is_empty() {
                Some(UniqueConstraintTracker::new(&ts.unique_constraints))
            } else {
                None
            };
            let row_count = *row_counts.get(table_name).unwrap_or(&0);
            if row_count == 0 {
                if self.debug {
                    eprintln!("[DEBUG] Skipping table '{}' (0 rows)", table_name);
                }
                continue;
            }
            let self_ref_cols: Vec<String> = ts
                .foreign_keys
                .iter()
                .filter(|fk| fk.referenced_table == *table_name)
                .map(|fk| fk.column.clone())
                .collect();

            let table_cfg = mock_config.and_then(|mc| mc.get(table_name));
            let pk_col = pk_col_name(schema, table_name);

            if self.debug {
                eprintln!(
                    "[DEBUG] Processing table: '{}', rows={}",
                    table_name, row_count
                );
                eprintln!("[DEBUG]   self_ref_cols = {:?}", self_ref_cols);
                eprintln!("[DEBUG]   foreign_keys = {:?}", ts.foreign_keys);
                eprintln!("[DEBUG]   unique_constraints = {:?}", ts.unique_constraints);
                eprintln!(
                    "[DEBUG]   fk_pools currently contain keys: {:?}",
                    fk_pools.keys().collect::<Vec<_>>()
                );
            }

            let bar = mp.add(ProgressBar::new(row_count as u64));
            bar.set_style(bar_style.clone());
            bar.set_message(table_name.clone());

            if preview {
                if self.debug {
                    eprintln!("[DEBUG] Preview mode for '{}'", table_name);
                }
                let stmts = build_insert_batches(
                    ts,
                    row_count,
                    &db_type,
                    &fk_pools,
                    &self_ref_cols,
                    table_cfg,
                    self.insert_rows,
                    constraint_tracker.as_ref(),
                );
                for s in &stmts {
                    report.sql_statements.push(s.clone());
                }
                let sample_size = row_count.min(20);
                let sample_rows = generate_sample_rows(
                    ts,
                    sample_size,
                    &db_type,
                    &fk_pools,
                    &self_ref_cols,
                    table_cfg,
                );
                if !sample_rows.is_empty() {
                    let headers: Vec<String> = ts
                        .columns
                        .iter()
                        .filter(|c| !c.is_auto_increment)
                        .map(|c| c.name.clone())
                        .collect();
                    println!(
                        "\n📊 Sample data for table '{}' ({} rows):",
                        table_name, sample_size
                    );
                    print_sample_table(&sample_rows, &headers);
                } else {
                    println!("No columns to display for table '{}'", table_name);
                }
                fk_pools.insert(
                    table_name.clone(),
                    synthetic_pool(row_count, self.fk_pool_cap),
                );
                bar.finish_with_message(format!("{} (dry-run)", table_name));
                overall.inc(row_count as u64);
                report.tables_processed += 1;
                continue;
            }

            // ── real insert: concurrent chunks ────────────────────────────────
            if self.debug {
                eprintln!("[DEBUG] Real insert mode for '{}'", table_name);
                eprintln!("[DEBUG]   Starting build_insert_batches (direct call)...");
            }
            let stmts = build_insert_batches(
                ts,
                row_count,
                &db_type,
                &fk_pools,
                &self_ref_cols,
                table_cfg,
                self.insert_rows,
                constraint_tracker.as_ref(),
            );
            if self.debug {
                eprintln!(
                    "[DEBUG] build_insert_batches returned {} statements",
                    stmts.len()
                );
            }
            let n_stmts = stmts.len();
            let rows_total = row_count as u64;
            if self.debug {
                eprintln!(
                    "[DEBUG]   build_insert_batches returned {} statements",
                    n_stmts
                );
            }

            // We collect IDs only from the first FK_POOL_CAP rows so we can
            // seed downstream tables — after that we discard RETURNING results.
            let need_ids = !fk_pools.contains_key(table_name);
            if self.debug {
                eprintln!("[DEBUG]   need_ids for '{}' = {}", table_name, need_ids);
                if need_ids && !self_ref_cols.is_empty() {
                    eprintln!("[DEBUG]   Table has self-ref columns but no existing pool; will attempt to collect IDs from RETURNING");
                }
            }
            let mut collected_ids: Vec<String> =
                Vec::with_capacity(self.fk_pool_cap.min(row_count));
            let mut inserted: u64 = 0;
            let mut errors: Vec<String> = Vec::new();

            // Semaphore limits in-flight concurrent INSERT tasks.
            let sem = Arc::new(Semaphore::new(self.concurrency));
            let mut handles = Vec::with_capacity(n_stmts);

            for (i, sql) in stmts.into_iter().enumerate() {
                let permit = sem.clone().acquire_owned().await.unwrap();
                let driver = self.driver.clone();
                let pk2 = pk_col.clone();
                let want_ids = need_ids && collected_ids.len() < self.fk_pool_cap;
                let rows_in_batch = sql
                    .bytes()
                    .filter(|&b| b == b'(')
                    .count()
                    .saturating_sub(1) // leading INSERT (...) also has a (
                    .max(1) as u64;
                // We'll just use INSERT_ROWS as the expected count per batch.
                let expected_rows = self.insert_rows as u64;

                if self.debug && i == 0 {
                    eprintln!(
                        "[DEBUG]   Spawning first task (sql preview: {} chars)",
                        sql.len()
                    );
                }
                handles.push(tokio::spawn(async move {
                    let result = if want_ids {
                        driver
                            .execute_batch_returning_ids(vec![sql], "", &pk2)
                            .await
                            .map(|ids| (ids.len() as u64, ids))
                    } else {
                        driver.execute_batch(vec![sql]).await.map(|n| (n, vec![]))
                    };
                    drop(permit);
                    result
                }));
            }

            if self.debug {
                eprintln!(
                    "[DEBUG]   Spawned {} tasks, waiting for results...",
                    handles.len()
                );
            }

            // Collect results as tasks complete
            for handle in handles {
                match handle.await {
                    Ok(Ok((n, ids))) => {
                        inserted += n;
                        if need_ids && collected_ids.len() < self.fk_pool_cap {
                            let remaining = self.fk_pool_cap - collected_ids.len();
                            collected_ids.extend(ids.into_iter().take(remaining));
                        }
                        bar.inc(n.min(self.insert_rows as u64));
                        overall.inc(n.min(self.insert_rows as u64));
                    }
                    Ok(Err(e)) => {
                        let msg = format!("Error inserting into {}: {}", table_name, e);
                        eprintln!("\n⚠️  {}", msg);
                        errors.push(msg);
                    }
                    Err(e) => {
                        let msg = format!("Task panic for {}: {}", table_name, e);
                        eprintln!("\n⚠️  {}", msg);
                        errors.push(msg);
                    }
                }
            }

            if self.debug {
                eprintln!(
                    "[DEBUG]   Finished tasks for '{}', inserted {} rows, collected {} ids",
                    table_name,
                    inserted,
                    collected_ids.len()
                );
            }

            report.total_rows_inserted += inserted;
            report.errors.extend(errors);
            report.tables_processed += 1;

            // Seed FK pool for downstream tables
            if collected_ids.is_empty() && need_ids {
                if self.debug {
                    eprintln!(
                        "[DEBUG]   No IDs collected via RETURNING; trying query_ids fallback..."
                    );
                }
                // RETURNING wasn't used (no downstream FK) or nothing landed
                // Try a lightweight re-query for a sample
                if let Ok(ids) = self
                    .driver
                    .query_ids(table_name, &pk_col, self.fk_pool_cap)
                    .await
                {
                    if self.debug {
                        eprintln!("[DEBUG]   query_ids returned {} ids", ids.len());
                    }
                    fk_pools.insert(table_name.clone(), ids);
                } else {
                    if self.debug {
                        eprintln!(
                            "[DEBUG]   query_ids failed; using synthetic_pool (size={})",
                            inserted
                        );
                    }
                    fk_pools.insert(
                        table_name.clone(),
                        synthetic_pool(inserted as usize, self.fk_pool_cap),
                    );
                }
            } else if !collected_ids.is_empty() {
                if self.debug {
                    eprintln!(
                        "[DEBUG]   Inserting {} collected ids into fk_pools for '{}'",
                        collected_ids.len(),
                        table_name
                    );
                }
                fk_pools.insert(table_name.clone(), collected_ids);
            } else {
                // Already had a pool or no IDs needed
                if !fk_pools.contains_key(table_name) {
                    if self.debug {
                        eprintln!(
                            "[DEBUG]   No pool yet; using synthetic_pool (size={})",
                            inserted
                        );
                    }
                    fk_pools.insert(
                        table_name.clone(),
                        synthetic_pool(inserted as usize, self.fk_pool_cap),
                    );
                }
            }

            bar.finish_with_message(format!("{} ✓ {} rows", table_name, inserted));
        }

        overall.finish_with_message(format!("total ✓ {} rows", report.total_rows_inserted));
        if self.debug {
            eprintln!("[DEBUG] Generation completed successfully.");
        }
        Ok(report)
    }
}
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

/// A small sequential pool used when real IDs are unavailable.
fn synthetic_pool(count: usize, fk_pool_cap: usize) -> Vec<String> {
    let cap = count.min(fk_pool_cap);
    (1..=cap).map(|i| i.to_string()).collect()
}
