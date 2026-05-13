//! batch.rs — build batched INSERT SQL statements from schema metadata.
//!
//! Value resolution priority for each column:
//!   1. Self-referencing FK → NULL
//!   2. FK with non-empty pool → random pool entry
//!   3. FK with empty pool + nullable → NULL
//!   4. FieldConfig present → generate_with_config()
//!      • FieldKind::Skip → exclude column from INSERT
//!   5. Fallback → schema-driven generate_value()

use std::collections::HashMap;

use rand::Rng;

use crate::core::schema::{ColumnSchema, TableSchema};
use crate::fieldconfig::generate::generate_with_config;
use crate::fieldconfig::infer::TableFieldConfig;
use crate::generator::value::generate_value;

const BATCH_SIZE: usize = 500;

/// Build all INSERT statements for `table`, generating `row_count` rows.
///
/// - `fk_id_pools`   : table → Vec of SQL literals for known PK values
/// - `self_ref_cols` : column names that self-reference (set to NULL)
/// - `table_config`  : optional per-column FieldConfig overrides
pub fn build_insert_batches(
    table: &TableSchema,
    row_count: usize,
    db_type: &str,
    fk_id_pools: &HashMap<String, Vec<String>>,
    self_ref_cols: &[String],
    table_config: Option<&TableFieldConfig>,
) -> Vec<String> {
    if row_count == 0 {
        return vec![];
    }

    // Columns to INSERT: exclude auto-increment, and also any col marked Skip.
    let cols: Vec<&ColumnSchema> = table
        .columns
        .iter()
        .filter(|c| {
            if c.is_auto_increment { return false; }
            // If config says Skip, exclude from column list entirely.
            if let Some(cfg) = table_config {
                if let Some(fc) = cfg.get(&c.name) {
                    if fc.kind == crate::fieldconfig::types::FieldKind::Skip {
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

    let table_q  = qi(&table.name, db_type);
    let col_list = cols.iter().map(|c| qi(&c.name, db_type)).collect::<Vec<_>>().join(", ");

    // Pre-compute strategy per column (avoids repeated lookups per row).
    let strategies: Vec<ColStrategy> = cols
        .iter()
        .map(|col| resolve_strategy(col, table, fk_id_pools, self_ref_cols, table_config))
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
    /// Always NULL (self-ref or empty nullable FK).
    ForceNull,
    /// Random pick from FK pool.
    FkPool(Vec<String>),
    /// Use FieldConfig (carries unique_key for unique/sequence counters).
    Configured {
        fc: crate::fieldconfig::types::FieldConfig,
        unique_key: String,
    },
    /// Schema-driven fallback.
    Generate,
}

fn resolve_strategy(
    col: &ColumnSchema,
    table: &TableSchema,
    fk_id_pools: &HashMap<String, Vec<String>>,
    self_ref_cols: &[String],
    table_config: Option<&TableFieldConfig>,
) -> ColStrategy {
    // 1. Self-referencing FK → NULL
    if self_ref_cols.contains(&col.name) {
        return ColStrategy::ForceNull;
    }

    // 2. FK columns
    if let Some(fk) = table.foreign_keys.iter().find(|fk| fk.column == col.name) {
        if let Some(pool) = fk_id_pools.get(&fk.referenced_table) {
            if !pool.is_empty() {
                return ColStrategy::FkPool(pool.clone());
            }
        }
        if col.is_nullable {
            return ColStrategy::ForceNull;
        }
        // Non-nullable FK with empty pool: fall through to config/generate;
        // the DB will surface the constraint error.
    }

    // 3. FieldConfig override
    if let Some(cfg) = table_config {
        if let Some(fc) = cfg.get(&col.name) {
            let unique_key = format!("{}.{}", table.name, col.name);
            return ColStrategy::Configured { fc: fc.clone(), unique_key };
        }
    }

    // 4. Schema-driven fallback
    ColStrategy::Generate
}

fn apply_strategy(strat: &ColStrategy, col: &ColumnSchema, db_type: &str) -> String {
    match strat {
        ColStrategy::ForceNull => "NULL".to_string(),

        ColStrategy::FkPool(pool) => {
            let idx = rand::thread_rng().gen_range(0..pool.len());
            pool[idx].clone()
        }

        ColStrategy::Configured { fc, unique_key } => {
            match generate_with_config(col, fc, unique_key, db_type) {
                Some(v) if v == "__SKIP__" => "NULL".to_string(), // shouldn't reach here
                Some(v) => v,
                None    => generate_value(col, db_type), // FieldKind::Default
            }
        }

        ColStrategy::Generate => generate_value(col, db_type),
    }
}

fn make_insert(table_q: &str, col_list: &str, rows: &[String]) -> String {
    format!("INSERT INTO {} ({}) VALUES {}", table_q, col_list, rows.join(", "))
}

fn qi(name: &str, db_type: &str) -> String {
    if db_type == "mysql" || db_type == "mariadb" {
        format!("`{}`", name)
    } else {
        format!("\"{}\"", name)
    }
}