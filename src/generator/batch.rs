use std::collections::HashMap;
use crate::core::schema::TableSchema;
use crate::generator::value::generate_value;

const BATCH_SIZE: usize = 1000;

/// Build batched INSERT SQL statements for a table
pub fn build_insert_batches(
    table: &TableSchema,
    row_count: usize,
    db_type: &str,
    fk_id_pools: &HashMap<String, Vec<String>>, // table -> list of existing PK values
) -> Vec<String> {
    let insertable_cols: Vec<&crate::core::schema::ColumnSchema> = table
        .columns
        .iter()
        .filter(|c| !c.is_auto_increment)
        .collect();

    if insertable_cols.is_empty() {
        return vec![];
    }

    let col_names: Vec<String> = insertable_cols
        .iter()
        .map(|c| quote_identifier(&c.name, db_type))
        .collect();

    let col_list = col_names.join(", ");
    let table_quoted = quote_identifier(&table.name, db_type);

    let mut statements = Vec::new();
    let mut batch_rows: Vec<String> = Vec::new();

    for row_idx in 0..row_count {
        let values: Vec<String> = insertable_cols
            .iter()
            .map(|col| {
                // Check if this column is a FK
                if let Some(fk) = table.foreign_keys.iter().find(|fk| fk.column == col.name) {
                    if let Some(pool) = fk_id_pools.get(&fk.referenced_table) {
                        if !pool.is_empty() {
                            let idx = row_idx % pool.len();
                            return pool[idx].clone();
                        }
                    }
                }
                generate_value(col, db_type)
            })
            .collect();

        batch_rows.push(format!("({})", values.join(", ")));

        if batch_rows.len() >= BATCH_SIZE {
            let sql = format!(
                "INSERT INTO {} ({}) VALUES {}",
                table_quoted,
                col_list,
                batch_rows.join(", ")
            );
            statements.push(sql);
            batch_rows.clear();
        }
    }

    if !batch_rows.is_empty() {
        let sql = format!(
            "INSERT INTO {} ({}) VALUES {}",
            table_quoted,
            col_list,
            batch_rows.join(", ")
        );
        statements.push(sql);
    }

    statements
}

fn quote_identifier(name: &str, db_type: &str) -> String {
    match db_type {
        "mysql" | "mariadb" => format!("`{}`", name),
        _ => format!("\"{}\"", name),
    }
}
