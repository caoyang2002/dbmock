//! batch.rs — pure value-generation; no I/O.
//!
//! `build_insert_batches` is called from a `spawn_blocking` thread so all
//! CPU-bound work (rand, string formatting) stays off the async executor.
//!
//! Value resolution priority per column:
//!   1. Auto-increment / Skip    → excluded from column list
//!   2. Self-referencing FK      → NULL
//!   3. FieldConfig override     → generate_with_config()   ← 用户配置优先于 FK 推断
//!   4. FK with pool             → random pool entry
//!   5. FK nullable, empty pool  → NULL
//!   6. Fallback                 → schema-driven generate_value()

use rand::seq::SliceRandom;
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
// 唯一约束跟踪器
// ─────────────────────────────────────────────────────────────────────────────

pub struct UniqueConstraintTracker {
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

    pub fn check_and_insert(&self, row_values: &[String], col_names: &[String]) -> bool {
        for (idx, cols) in self.constraint_cols.iter().enumerate() {
            let mut key_parts = Vec::new();
            for c in cols {
                if let Some(pos) = col_names.iter().position(|name| name == c) {
                    key_parts.push(row_values[pos].clone());
                } else {
                    continue;
                }
            }
            if key_parts.is_empty() {
                continue;
            }
            let key = key_parts.join("|");
            let mut gen = self.generators[idx].lock().unwrap();
            if !gen.insert(key) {
                return false;
            }
        }
        true
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 批量生成 INSERT 语句（核心函数）
// ─────────────────────────────────────────────────────────────────────────────

pub fn build_insert_batches(
    table: &TableSchema,
    row_count: usize,
    db_type: &str,
    fk_id_pools: &HashMap<String, Vec<String>>,
    self_ref_cols: &[String],
    table_config: Option<&TableFieldConfig>,
    insert_rows: usize,
    constraint_tracker: Option<&UniqueConstraintTracker>,
    _debug: bool,
) -> Vec<String> {
    if row_count == 0 || insert_rows == 0 {
        return vec![];
    }

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

    let strategies: Vec<ColStrategy> = cols
        .iter()
        .map(|col| resolve_strategy(col, table, fk_id_pools, self_ref_cols, table_config))
        .collect();

    let mut sequential_allocators: HashMap<String, (Vec<String>, usize)> = HashMap::new();
    for constraint in &table.unique_constraints {
        if constraint.len() != 1 {
            continue;
        }
        let col_name = &constraint[0];

        // 若该列已被 FieldConfig 显式配置，跳过顺序分配器：
        // FieldConfig 自身负责唯一性（如 Username/Email 走全局 HashSet）。
        if let Some(cfg) = table_config {
            if cfg.contains_key(col_name) {
                continue;
            }
        }

        let mut is_fk = false;
        if let Some(fk) = table.foreign_keys.iter().find(|fk| fk.column == *col_name) {
            if let Some(pool) = fk_id_pools.get(&fk.referenced_table) {
                if !pool.is_empty() {
                    let mut shuffled = pool.clone();
                    shuffled.shuffle(&mut rand::thread_rng());
                    sequential_allocators.insert(col_name.clone(), (shuffled, 0));
                    is_fk = true;
                }
            }
        }
        if !is_fk {
            let mut values: Vec<String> = (1..=row_count).map(|i| i.to_string()).collect();
            values.shuffle(&mut rand::thread_rng());
            sequential_allocators.insert(col_name.clone(), (values, 0));
        }
    }

    let n_stmts = row_count.div_ceil(insert_rows);
    let mut statements = Vec::with_capacity(n_stmts);
    let mut batch = Vec::with_capacity(insert_rows);
    let mut values_buf = Vec::with_capacity(cols.len());

    const MAX_RETRIES: usize = 100;

    for row_idx in 0..row_count {
        let mut retries = 0;
        let final_values = loop {
            values_buf.clear();
            for (strat, col) in strategies.iter().zip(cols.iter()) {
                if let Some((pool, idx)) = sequential_allocators.get_mut(&col.name) {
                    let v = pool[*idx % pool.len()].clone();
                    values_buf.push(v);
                } else {
                    let v = apply_strategy(strat, col, db_type);
                    values_buf.push(v);
                }
            }

            let ok = if let Some(tracker) = constraint_tracker {
                tracker.check_and_insert(&values_buf, &col_names)
            } else {
                true
            };

            if ok {
                for col in cols.iter() {
                    if let Some((_pool, idx)) = sequential_allocators.get_mut(&col.name) {
                        *idx += 1;
                    }
                }
                break values_buf.clone();
            } else {
                retries += 1;
                if retries >= MAX_RETRIES {
                    for col in cols.iter() {
                        if let Some((_pool, idx)) = sequential_allocators.get_mut(&col.name) {
                            *idx += 1;
                        }
                    }
                    break values_buf.clone();
                }
                continue;
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
// 策略解析
// ─────────────────────────────────────────────────────────────────────────────

enum ColStrategy<'a> {
    ForceNull,
    FkPool(&'a [String]),
    Configured {
        fc: &'a crate::fieldconfig::types::FieldConfig,
        unique_key: String,
    },
    Generate,
}

fn resolve_strategy<'a>(
    col: &ColumnSchema,
    table: &TableSchema,
    fk_id_pools: &'a HashMap<String, Vec<String>>,
    self_ref_cols: &[String],
    table_config: Option<&'a TableFieldConfig>,
) -> ColStrategy<'a> {
    // 优先级 1：自引用 FK → 始终 NULL，即使有 FieldConfig 也如此
    // （自引用列生成有效 ID 需要已插入的行，超出当前生成范围）
    if self_ref_cols.contains(&col.name) {
        return ColStrategy::ForceNull;
    }

    // 优先级 2：用户显式配置（FieldConfig）
    // ★ 必须在 FK 推断之前：用户配置了 type: username/email 等，
    //   不应被 FK 推断覆盖，否则会输出外键池里的整数。
    if let Some(cfg) = table_config {
        if let Some(fc) = cfg.get(&col.name) {
            let unique_key = format!("{}.{}", table.name, col.name);
            return ColStrategy::Configured { fc, unique_key };
        }
    }

    // 优先级 3：常规 FK 推断（仅在无 FieldConfig 时生效）
    if let Some(fk) = table.foreign_keys.iter().find(|fk| fk.column == col.name) {
        if let Some(pool) = fk_id_pools.get(&fk.referenced_table) {
            if !pool.is_empty() {
                return ColStrategy::FkPool(pool.as_slice());
            }
        }
        if col.is_nullable {
            return ColStrategy::ForceNull;
        }
        // 非空 FK 但池为空 → 落入 Generate，由 DB 报错
    }

    // 优先级 4：schema 驱动默认生成
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
