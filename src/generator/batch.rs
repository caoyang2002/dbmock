//! batch.rs — build batched INSERT SQL statements from schema metadata.
//!
//! Value resolution priority for each column:
//!   1. Self-referencing FK column (parent_id → same table) → NULL
//!      (only if column is nullable; non-nullable self-refs are skipped with a warning)
//!   2. Foreign-key column with a non-empty pool → random entry from pool
//!   3. Foreign-key column with an EMPTY pool and column is nullable → NULL
//!   4. Foreign-key column with an EMPTY pool and column is NOT nullable → error
//!      is surfaced to the caller via the DB constraint violation
//!   5. Regular column → generate_value()

use std::collections::HashMap;

use rand::Rng;

use crate::core::schema::{ColumnSchema, TableSchema};
use crate::generator::value::generate_value;

const BATCH_SIZE: usize = 500;

/// Build all INSERT statements for `table`, generating `row_count` rows.
///
/// - `fk_id_pools`  : table_name → Vec of SQL literals for known PK values
/// - `self_ref_cols`: column names that reference the same table (set to NULL)
pub fn build_insert_batches(
    table: &TableSchema,
    row_count: usize,
    db_type: &str,
    fk_id_pools: &HashMap<String, Vec<String>>,
    self_ref_cols: &[String],
) -> Vec<String> {
    // Columns we actually INSERT (exclude auto-increment).
    let cols: Vec<&ColumnSchema> = table
        .columns
        .iter()
        .filter(|c| !c.is_auto_increment)
        .collect();

    if cols.is_empty() || row_count == 0 {
        return vec![];
    }

    let table_q  = qi(&table.name, db_type);
    let col_list = cols.iter().map(|c| qi(&c.name, db_type)).collect::<Vec<_>>().join(", ");

    // Pre-compute per-column resolution strategy so we don't redo it per row.
    let strategies: Vec<ColStrategy> = cols
        .iter()
        .map(|col| resolve_strategy(col, table, fk_id_pools, self_ref_cols))
        .collect();

    let mut statements = Vec::new();
    let mut batch: Vec<String> = Vec::with_capacity(BATCH_SIZE);

    for _ in 0..row_count {
        let values: Vec<String> = strategies
            .iter()
            .zip(cols.iter())
            .map(|(strat, col)| apply_strategy(strat, col, db_type))
            .collect();

        batch.push(format!("({})", values.join(", ")));

        if batch.len() >= BATCH_SIZE {
            statements.push(make_insert(&table_q, &col_list, &batch));
            batch.clear();
        }
    }
    if !batch.is_empty() {
        statements.push(make_insert(&table_q, &col_list, &batch));
    }

    statements
}

// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
enum ColStrategy {
    /// Always emit NULL (self-ref FK or nullable FK with empty pool).
    ForceNull,
    /// Pick a random entry from this pool.
    FkPool(Vec<String>),
    /// Generate a value based on column schema.
    Generate,
}

fn resolve_strategy(
    col: &ColumnSchema,
    table: &TableSchema,
    fk_id_pools: &HashMap<String, Vec<String>>,
    self_ref_cols: &[String],
) -> ColStrategy {
    // ── self-referencing FK ───────────────────────────────────────────────────
    if self_ref_cols.contains(&col.name) {
        // Always NULL for self-refs — makes every row a root node.
        // The column must be nullable in practice (e.g. parent_id).
        return ColStrategy::ForceNull;
    }

    // ── regular FK ───────────────────────────────────────────────────────────
    if let Some(fk) = table.foreign_keys.iter().find(|fk| fk.column == col.name) {
        if let Some(pool) = fk_id_pools.get(&fk.referenced_table) {
            if !pool.is_empty() {
                return ColStrategy::FkPool(pool.clone());
            }
        }
        // No pool available.
        if col.is_nullable {
            return ColStrategy::ForceNull;
        }
        // Non-nullable FK with empty pool: fall through to Generate.
        // The DB will raise a FK constraint error which surfaces to the user.
    }

    ColStrategy::Generate
}

fn apply_strategy(strat: &ColStrategy, col: &ColumnSchema, db_type: &str) -> String {
    match strat {
        ColStrategy::ForceNull => "NULL".to_string(),
        ColStrategy::FkPool(pool) => {
            let idx = rand::thread_rng().gen_range(0..pool.len());
            pool[idx].clone()
        }
        ColStrategy::Generate => generate_value(col, db_type),
    }
}

fn make_insert(table_q: &str, col_list: &str, rows: &[String]) -> String {
    format!("INSERT INTO {} ({}) VALUES {}", table_q, col_list, rows.join(", "))
}

/// Quote an identifier: backtick for MySQL, double-quote for everything else.
fn qi(name: &str, db_type: &str) -> String {
    if db_type == "mysql" || db_type == "mariadb" {
        format!("`{}`", name)
    } else {
        format!("\"{}\"", name)
    }
}
