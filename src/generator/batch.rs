//! batch.rs — pure value-generation; no I/O.
//!
//! `build_insert_batches` is called from a `spawn_blocking` thread so all
//! CPU-bound work (rand, string formatting) stays off the async executor.
//!
//! Value resolution priority per column:
//!   1. Auto-increment / Skip    → excluded from column list
//!   2. Self-referencing FK      → NULL
//!   3. FK with pool             → random pool entry   (O(1), no clone)
//!   4. FK nullable, empty pool  → NULL
//!   5. FieldConfig override     → generate_with_config()
//!   6. Fallback                 → schema-driven generate_value()

use rand::Rng;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::core::schema::{ColumnSchema, TableSchema};
use crate::datapool::UniqueGenerator;
use crate::fieldconfig::generate::generate_with_config;
use crate::fieldconfig::infer::TableFieldConfig;
use crate::fieldconfig::types::FieldKind;
use crate::generator::value::generate_value;

/// 生成少量样本行（用于 dry-run 预览），不构建完整的 INSERT 语句。
/// 返回 Vec<Vec<String>>，每个内部 Vec 是一行的所有列值。
pub fn generate_sample_rows(
    table: &TableSchema,
    sample_count: usize,
    db_type: &str,
    fk_id_pools: &HashMap<String, Vec<String>>,
    self_ref_cols: &[String],
    table_config: Option<&TableFieldConfig>,
) -> Vec<Vec<String>> {
    if sample_count == 0 {
        return vec![];
    }

    // 构建列列表（与 build_insert_batches 一致）
    let cols: Vec<&ColumnSchema> = table
        .columns
        .iter()
        .filter(|c| {
            if c.is_auto_increment {
                return false;
            }
            if let Some(cfg) = table_config {
                if let Some(fc) = cfg.get(&c.name) {
                    if fc.kind == FieldKind::Skip {
                        return false;
                    }
                }
            }
            true
        })
        .collect();

    if cols.is_empty() {
        return vec![];
    }

    let strategies: Vec<ColStrategy> = cols
        .iter()
        .map(|col| resolve_strategy(col, table, fk_id_pools, self_ref_cols, table_config))
        .collect();

    let mut rows = Vec::with_capacity(sample_count);
    for _ in 0..sample_count {
        let mut row_values = Vec::with_capacity(cols.len());
        for (strat, col) in strategies.iter().zip(cols.iter()) {
            row_values.push(apply_strategy(strat, col, db_type));
        }
        rows.push(row_values);
    }
    rows
}

// ─────────────────────────────────────────────────────────────────────────────
// 唯一约束跟踪器（公开发布，供 engine.rs 使用）
// ─────────────────────────────────────────────────────────────────────────────

/// 管理一张表的所有唯一约束
pub struct UniqueConstraintTracker {
    // 每个约束对应一个生成器，存储已使用的组合键字符串
    generators: Vec<Mutex<UniqueGenerator<String>>>,
    constraint_cols: Vec<Vec<String>>,
}

impl UniqueConstraintTracker {
    pub fn new(constraints: &[Vec<String>]) -> Self {
        let generators = constraints
            .iter()
            .map(|_| Mutex::new(UniqueGenerator::new()))
            .collect();
        Self {
            generators,
            constraint_cols: constraints.to_vec(),
        }
    }

    /// 检查一行是否满足所有唯一约束，若满足则记录并返回 true；否则返回 false
    pub fn check_and_insert(&self, row_values: &[String], col_names: &[String]) -> bool {
        for (idx, cols) in self.constraint_cols.iter().enumerate() {
            // 构建组合键：将约束中涉及的列的值按顺序拼接
            let mut key_parts = Vec::new();
            for c in cols {
                if let Some(pos) = col_names.iter().position(|name| name == c) {
                    key_parts.push(row_values[pos].clone());
                } else {
                    // 该列不在当前插入列表中（可能因 auto_increment 或 skip 被排除）
                    // 此时无法验证完整性，跳过该约束（但实际不应发生）
                    continue;
                }
            }
            if key_parts.is_empty() {
                continue;
            }
            let key = key_parts.join("|");
            let mut gen = self.generators[idx].lock().unwrap();
            if !gen.insert(key) {
                return false; // 冲突
            }
        }
        true
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 批量生成 INSERT 语句（核心函数）
// ─────────────────────────────────────────────────────────────────────────────

/// Build all INSERT statements for `table`.
///
/// - `row_count`         : total rows to generate
/// - `insert_rows`       : rows per INSERT statement (e.g. 1_000)
/// - `fk_id_pools`       : table → pool of SQL literals for FK values
/// - `self_ref_cols`     : column names with self-referencing FK (→ NULL)
/// - `table_config`      : optional per-column FieldConfig overrides
/// - `constraint_tracker`: optional tracker for unique constraints
pub fn build_insert_batches(
    table: &TableSchema,
    row_count: usize,
    db_type: &str,
    fk_id_pools: &HashMap<String, Vec<String>>,
    self_ref_cols: &[String],
    table_config: Option<&TableFieldConfig>,
    insert_rows: usize,
    constraint_tracker: Option<&UniqueConstraintTracker>,
    debug: bool,
) -> Vec<String> {
    if row_count == 0 || insert_rows == 0 {
        return vec![];
    }

    // ── build the ordered, filtered column list ───────────────────────────────
    let cols: Vec<&ColumnSchema> = table
        .columns
        .iter()
        .filter(|c| {
            if c.is_auto_increment {
                return false;
            }
            if let Some(cfg) = table_config {
                if let Some(fc) = cfg.get(&c.name) {
                    if fc.kind == FieldKind::Skip {
                        return false;
                    }
                }
            }
            true
        })
        .collect();

    if cols.is_empty() {
        return vec![];
    }

    let table_q = qi(&table.name, db_type);
    let col_list = cols
        .iter()
        .map(|c| qi(&c.name, db_type))
        .collect::<Vec<_>>()
        .join(", ");
    let col_names: Vec<String> = cols.iter().map(|c| c.name.clone()).collect();

    // ── pre-compute per-column strategy ─────────────────────────────
    let strategies: Vec<ColStrategy> = cols
        .iter()
        .map(|col| resolve_strategy(col, table, fk_id_pools, self_ref_cols, table_config))
        .collect();

    // ── generate rows ─────────────────────────────────────────────────────────
    let n_stmts = row_count.div_ceil(insert_rows);
    let mut statements: Vec<String> = Vec::with_capacity(n_stmts);
    let mut batch: Vec<String> = Vec::with_capacity(insert_rows);
    let mut values_buf: Vec<String> = Vec::with_capacity(cols.len());

    const MAX_RETRIES: usize = 100;

    for row_idx in 0..row_count {
        let mut retries = 0;
        let final_values = loop {
            values_buf.clear();
            // 生成所有列的值
            for (strat, col) in strategies.iter().zip(cols.iter()) {
                let val = apply_strategy(strat, col, db_type);
                values_buf.push(val);
            }

            if let Some(tracker) = constraint_tracker {
                let ok = tracker.check_and_insert(&values_buf, &col_names);
                if ok {
                    break values_buf.clone();
                } else {
                    retries += 1;
                    if retries >= MAX_RETRIES {
                        break values_buf.clone();
                    }
                    continue;
                }
            } else {
                break values_buf.clone();
            }
        };

        batch.push(format!("({})", final_values.join(", ")));

        if batch.len() >= insert_rows || row_idx == row_count - 1 {
            statements.push(make_insert(&table_q, &col_list, &batch));
            batch.clear();
        }
    }

    statements
}

// ─────────────────────────────────────────────────────────────────────────────
// 辅助类型和函数
// ─────────────────────────────────────────────────────────────────────────────

enum ColStrategy<'a> {
    ForceNull,
    /// Borrow the pool slice — no clone of the entire Vec.
    FkPool(&'a [String]),
    Configured {
        fc: &'a crate::fieldconfig::types::FieldConfig,
        unique_key: String,
    },
    Generate,
}

//
fn resolve_strategy<'a>(
    col: &ColumnSchema,
    table: &TableSchema,
    fk_id_pools: &'a HashMap<String, Vec<String>>,
    self_ref_cols: &[String],
    table_config: Option<&'a TableFieldConfig>,
) -> ColStrategy<'a> {
    // 1. Self-ref FK
    if self_ref_cols.contains(&col.name) {
        return ColStrategy::ForceNull;
    }

    // 2. Regular FK
    if let Some(fk) = table.foreign_keys.iter().find(|fk| fk.column == col.name) {
        if let Some(pool) = fk_id_pools.get(&fk.referenced_table) {
            if !pool.is_empty() {
                return ColStrategy::FkPool(pool.as_slice());
            }
        }
        if col.is_nullable {
            return ColStrategy::ForceNull;
        }
        // Non-nullable FK, empty pool → fall through; DB will report the error.
    }

    // 3. FieldConfig override
    if let Some(cfg) = table_config {
        if let Some(fc) = cfg.get(&col.name) {
            let unique_key = format!("{}.{}", table.name, col.name);
            return ColStrategy::Configured { fc, unique_key };
        }
    }

    // 4. Schema-driven default
    ColStrategy::Generate
}

#[inline]
fn apply_strategy(strat: &ColStrategy<'_>, col: &ColumnSchema, db_type: &str) -> String {
    match strat {
        ColStrategy::ForceNull => "NULL".to_string(),

        ColStrategy::FkPool(pool) => {
            let idx = rand::thread_rng().gen_range(0..pool.len());
            pool[idx].clone()
        }

        ColStrategy::Configured { fc, unique_key } => {
            match generate_with_config(col, fc, unique_key, db_type) {
                Some(v) if v == "__SKIP__" => "NULL".to_string(),
                Some(v) => v,
                None => generate_value(col, db_type),
            }
        }

        ColStrategy::Generate => generate_value(col, db_type),
    }
}

fn make_insert(table_q: &str, col_list: &str, rows: &[String]) -> String {
    let mut out = String::with_capacity(
        "INSERT INTO  () VALUES ".len() + table_q.len() + col_list.len() + rows.len() * 60,
    );
    out.push_str("INSERT INTO ");
    out.push_str(table_q);
    out.push_str(" (");
    out.push_str(col_list);
    out.push_str(") VALUES ");
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(row);
    }
    out
}

#[inline]
fn qi(name: &str, db_type: &str) -> String {
    if db_type == "mysql" || db_type == "mariadb" {
        format!("`{}`", name)
    } else {
        format!("\"{}\"", name)
    }
}
