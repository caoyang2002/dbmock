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

use std::collections::HashMap;

use rand::Rng;

use crate::core::schema::{ColumnSchema, TableSchema};
use crate::fieldconfig::generate::generate_with_config;
use crate::fieldconfig::infer::TableFieldConfig;
use crate::fieldconfig::types::FieldKind;
use crate::generator::value::generate_value;

/// Build all INSERT statements for `table`.
///
/// - `row_count`     : total rows to generate
/// - `insert_rows`   : rows per INSERT statement (e.g. 1_000)
/// - `fk_id_pools`   : table → pool of SQL literals for FK values
/// - `self_ref_cols` : column names with self-referencing FK (→ NULL)
/// - `table_config`  : optional per-column FieldConfig overrides
pub fn build_insert_batches(
    table: &TableSchema,
    row_count: usize,
    db_type: &str,
    fk_id_pools: &HashMap<String, Vec<String>>,
    self_ref_cols: &[String],
    table_config: Option<&TableFieldConfig>,
    insert_rows: usize,
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

    // ── pre-compute per-column strategy (done once, not per row) ─────────────
    let strategies: Vec<ColStrategy> = cols
        .iter()
        .map(|col| resolve_strategy(col, table, fk_id_pools, self_ref_cols, table_config))
        .collect();

    // ── generate rows ─────────────────────────────────────────────────────────
    // Pre-size the output vector to avoid reallocations.
    let n_stmts = row_count.div_ceil(insert_rows);
    let mut statements: Vec<String> = Vec::with_capacity(n_stmts);

    // Reuse a Vec<String> for values and a String for the row buffer to avoid
    // repeated allocations inside the hot loop.
    let mut batch: Vec<String> = Vec::with_capacity(insert_rows);
    let mut values_buf: Vec<String> = Vec::with_capacity(cols.len());

    for row_idx in 0..row_count {
        values_buf.clear();
        for (strat, col) in strategies.iter().zip(cols.iter()) {
            values_buf.push(apply_strategy(strat, col, db_type));
        }
        batch.push(format!("({})", values_buf.join(", ")));

        if batch.len() >= insert_rows || row_idx == row_count - 1 {
            statements.push(make_insert(&table_q, &col_list, &batch));
            batch.clear();
        }
    }

    statements
}

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

// ─────────────────────────────────────────────────────────────────────────────

fn make_insert(table_q: &str, col_list: &str, rows: &[String]) -> String {
    // Pre-size the output string to avoid reallocs:
    // rough estimate: avg 50 chars per row
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
