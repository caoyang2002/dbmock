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
use crate::config::TuningParams;
use crate::core::driver::DatabaseDriver;
use crate::core::generator::{DataGenerator, GenerationReport};
use crate::core::schema::Schema;
use crate::core::schema::TableSchema;
use crate::errors::{MockerError, Result};
use crate::fieldconfig::generate::reset_unique_counters;
use crate::fieldconfig::infer::MockConfig;
use crate::fieldconfig::FieldConfig;
use crate::generator::batch::generate_sample_rows;
use crate::generator::batch::{build_insert_batches, UniqueConstraintTracker};
use crate::generator::dependency::topological_sort;
use crate::logger::print_sample_table;
use async_trait::async_trait;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rand::Rng;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::Semaphore;

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
    /// 专门处理自引用表的插入（两阶段）
    /// 专用于自引用表的两阶段插入
    /// 第一阶段：生成 INSERT，自引用列填 NULL，并执行插入（收集所有返回的主键）
    /// 第二阶段：根据主键列表，为每行随机分配一个已存在的父行（索引更小），然后执行 UPDATE

    async fn insert_self_referential_table(
        &self,
        ts: &TableSchema,
        row_count: usize,
        db_type: &str,
        table_cfg: Option<&BTreeMap<String, FieldConfig>>,
        constraint_tracker: Option<&UniqueConstraintTracker>,
        fk_pools: &mut HashMap<String, Vec<String>>,
        bar: &ProgressBar,
        overall: &ProgressBar,
    ) -> Result<(u64, Vec<String>, Vec<String>)> {
        let table_name = &ts.name;
        let pk_col = ts.primary_keys.first().unwrap_or(&"id".to_string()).clone();
        let self_fk_col = ts
            .foreign_keys
            .iter()
            .find(|fk| fk.referenced_table == *table_name)
            .map(|fk| fk.column.clone())
            .ok_or_else(|| MockerError::TableNotFound {
                table: table_name.clone(),
            })?;

        // 生成 INSERT 语句（自引用列 = NULL）
        let stmts = build_insert_batches(
            ts,
            row_count,
            db_type,
            fk_pools,
            &[self_fk_col.clone()],
            table_cfg,
            self.insert_rows,
            constraint_tracker,
            self.debug,
        );

        let sem = Arc::new(Semaphore::new(self.concurrency));
        let mut inserted = 0u64;
        let mut errors = Vec::new();
        let mut all_ids = Vec::with_capacity(row_count);
        let debug = self.debug;
        let table_name_owned = table_name.to_string();

        if debug && !stmts.is_empty() {
            eprintln!("[DEBUG] First INSERT SQL for {}:\n{}", table_name, stmts[0]);
        }

        let mut handles = Vec::new();
        for sql in stmts {
            let permit = sem.clone().acquire_owned().await.unwrap();
            let driver = self.driver.clone();
            let pk = pk_col.clone();
            // let debug_task = debug;
            // let table_name_task = table_name_owned.clone();
            handles.push(tokio::spawn(async move {
                let result = driver
                    .execute_batch_returning_ids(vec![sql.clone()], "", &pk)
                    .await
                    .map(|ids| (ids.len() as u64, ids));
                drop(permit);
                (result, sql) // 返回 SQL 用于错误日志
            }));
        }

        for handle in handles {
            match handle.await {
                Ok((Ok((n, ids)), sql)) => {
                    inserted += n;
                    all_ids.extend(ids);
                    bar.inc(n.min(self.insert_rows as u64));
                    overall.inc(n.min(self.insert_rows as u64));
                }
                Ok((Err(e), sql)) => {
                    let msg = format!("Error inserting into {}: {}\nSQL: {}", table_name, e, sql);
                    eprintln!("\n⚠️  {}", msg);
                    errors.push(msg);
                }
                Err(e) => {
                    let msg = format!("Task panic for {}: {}", table_name, e);
                    errors.push(msg);
                }
            }
        }

        if inserted == 0 {
            return Ok((0, vec![], errors));
        }

        if all_ids.len() < row_count {
            if let Ok(ids) = self.driver.query_ids(table_name, &pk_col, row_count).await {
                all_ids = ids;
            } else {
                all_ids = synthetic_pool(row_count, row_count);
            }
        }

        // 第二阶段：UPDATE 自引用外键
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::from_entropy();
        for idx in 1..all_ids.len() {
            let pk_val = &all_ids[idx];
            let parent_idx = rng.gen_range(0..idx);
            let parent_pk = &all_ids[parent_idx];
            let sql = format!(
                "UPDATE {} SET {} = '{}' WHERE {} = '{}'",
                table_name, self_fk_col, parent_pk, pk_col, pk_val
            );
            if let Err(e) = self.driver.execute_batch(vec![sql]).await {
                let msg = format!("Error updating {} {}: {}", table_name, pk_val, e);
                errors.push(msg);
            }
        }

        Ok((inserted, all_ids, errors))
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
            let row_count = *row_counts.get(table_name).unwrap_or(&0);
            if row_count == 0 {
                if self.debug {
                    eprintln!("[DEBUG] Skipping table '{}' (0 rows)", table_name);
                }
                continue;
            }

            // 判断是否自引用
            let self_ref_cols: Vec<String> = ts
                .foreign_keys
                .iter()
                .filter(|fk| fk.referenced_table == *table_name)
                .map(|fk| fk.column.clone())
                .collect();
            let has_self_ref = !self_ref_cols.is_empty();

            // 创建进度条
            let bar = mp.add(ProgressBar::new(row_count as u64));
            bar.set_style(bar_style.clone());
            bar.set_message(table_name.clone());

            // ── 预览模式 ───────────────────────────────────────────────────────
            if preview {
                if self.debug {
                    eprintln!("[DEBUG] Preview mode for '{}'", table_name);
                }
                let constraint_tracker = if !ts.unique_constraints.is_empty() {
                    Some(UniqueConstraintTracker::new(&ts.unique_constraints))
                } else {
                    None
                };
                // 生成 SQL 语句（自引用列会填 NULL）
                let stmts = build_insert_batches(
                    ts,
                    row_count,
                    &db_type,
                    &fk_pools,
                    &self_ref_cols,
                    mock_config.and_then(|mc| mc.get(table_name)),
                    self.insert_rows,
                    constraint_tracker.as_ref(),
                    self.debug,
                );
                for s in &stmts {
                    report.sql_statements.push(s.clone());
                }
                // 生成样本数据
                let sample_size = row_count.min(20);
                let sample_rows = generate_sample_rows(
                    ts,
                    sample_size,
                    &db_type,
                    &fk_pools,
                    &self_ref_cols,
                    mock_config.and_then(|mc| mc.get(table_name)),
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
                // 预览模式也把合成主键加入池中，使下游预览能拿到外键值
                fk_pools.insert(
                    table_name.clone(),
                    synthetic_pool(row_count, self.fk_pool_cap),
                );
                bar.finish_with_message(format!("{} (dry-run)", table_name));
                overall.inc(row_count as u64);
                report.tables_processed += 1;
                continue;
            }

            // ── 真实插入 ───────────────────────────────────────────────────────
            let constraint_tracker = if !ts.unique_constraints.is_empty() {
                Some(UniqueConstraintTracker::new(&ts.unique_constraints))
            } else {
                None
            };

            let table_cfg = mock_config.and_then(|mc| mc.get(table_name));
            let pk_col = pk_col_name(schema, table_name);
            let need_ids = !fk_pools.contains_key(table_name);

            if has_self_ref {
                // 自引用表：两阶段插入（第一阶段填 NULL，第二阶段 UPDATE）
                let (inserted, collected_ids, errors) = self
                    .insert_self_referential_table(
                        ts,
                        row_count,
                        &db_type,
                        table_cfg,
                        constraint_tracker.as_ref(),
                        &mut fk_pools,
                        &bar,
                        &overall,
                    )
                    .await?;

                report.total_rows_inserted += inserted;
                report.errors.extend(errors);
                report.tables_processed += 1;

                // 将收集到的 ID 放入 fk_pools
                if !collected_ids.is_empty() {
                    fk_pools.insert(table_name.clone(), collected_ids);
                } else if inserted > 0 {
                    fk_pools.insert(
                        table_name.clone(),
                        synthetic_pool(inserted as usize, self.fk_pool_cap),
                    );
                }
                bar.finish_with_message(format!("{} ✓ {} rows", table_name, inserted));
                continue;
            }

            // ── 普通表（无自引用）：原有并发批量插入逻辑 ────────────────────────
            if self.debug {
                eprintln!("[DEBUG] Real insert mode for '{}'", table_name);
            }

            let stmts = build_insert_batches(
                ts,
                row_count,
                &db_type,
                &fk_pools,
                &self_ref_cols, // 这里为空
                table_cfg,
                self.insert_rows,
                constraint_tracker.as_ref(),
                self.debug,
            );

            let n_stmts = stmts.len();
            let mut inserted = 0u64;
            let mut errors = Vec::new();
            let mut collected_ids = Vec::with_capacity(self.fk_pool_cap.min(row_count));

            let sem = Arc::new(Semaphore::new(self.concurrency));
            let mut handles = Vec::with_capacity(n_stmts);

            for sql in stmts {
                let permit = sem.clone().acquire_owned().await.unwrap();
                let driver = self.driver.clone();
                let pk = pk_col.clone();
                let want_ids = need_ids && collected_ids.len() < self.fk_pool_cap;
                handles.push(tokio::spawn(async move {
                    let result = if want_ids {
                        driver
                            .execute_batch_returning_ids(vec![sql], "", &pk)
                            .await
                            .map(|ids| (ids.len() as u64, ids))
                    } else {
                        driver.execute_batch(vec![sql]).await.map(|n| (n, vec![]))
                    };
                    drop(permit);
                    result
                }));
            }

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

            report.total_rows_inserted += inserted;
            report.errors.extend(errors);
            report.tables_processed += 1;

            // 将主键池提供给下游表
            if collected_ids.is_empty() && need_ids {
                if let Ok(ids) = self
                    .driver
                    .query_ids(table_name, &pk_col, self.fk_pool_cap)
                    .await
                {
                    fk_pools.insert(table_name.clone(), ids);
                } else {
                    fk_pools.insert(
                        table_name.clone(),
                        synthetic_pool(inserted as usize, self.fk_pool_cap),
                    );
                }
            } else if !collected_ids.is_empty() {
                fk_pools.insert(table_name.clone(), collected_ids);
            } else if !fk_pools.contains_key(table_name) && inserted > 0 {
                fk_pools.insert(
                    table_name.clone(),
                    synthetic_pool(inserted as usize, self.fk_pool_cap),
                );
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
