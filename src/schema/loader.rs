use crate::core::schema::{ColumnSchema, ForeignKey, Schema, TableSchema};
use crate::errors::{MockerError, Result};
use std::path::Path;

/// Load schema from a JSON file
pub fn load_json(path: &Path) -> Result<Schema> {
    let content = std::fs::read_to_string(path).map_err(|e| MockerError::Io(e))?;
    let schema: Schema = serde_json::from_str(&content)?;
    Ok(schema)
}

/// Save schema to a JSON file
pub fn save_json(schema: &Schema, path: &Path) -> Result<()> {
    let content = serde_json::to_string_pretty(schema)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Save schema to a SQL file
pub fn save_sql(schema: &Schema, path: &Path) -> Result<()> {
    let sql = match schema.database_type.as_str() {
        "postgres" | "postgresql" => crate::schema::extractor::postgres::schema_to_sql(schema),
        "mysql" | "mariadb" => crate::schema::extractor::mysql::schema_to_sql(schema),
        "sqlite" => crate::schema::extractor::sqlite::schema_to_sql(schema),
        _ => crate::schema::extractor::postgres::schema_to_sql(schema),
    };
    std::fs::write(path, sql)?;
    Ok(())
}

/// Parse a basic SQL CREATE TABLE statement into a Schema.
/// Supports simple column definitions; not a full SQL parser.
pub fn load_sql(path: &Path) -> Result<Schema> {
    let content = std::fs::read_to_string(path)?;
    parse_sql_schema(&content)
}

fn parse_sql_schema(sql: &str) -> Result<Schema> {
    let mut tables = Vec::new();
    let mut db_type = "postgres".to_string();

    // Very simplistic parser for CREATE TABLE statements
    for chunk in sql.split(';') {
        let chunk = chunk.trim();
        if chunk.to_uppercase().contains("CREATE TABLE") {
            if let Some(table) = parse_create_table(chunk) {
                tables.push(table);
            }
        }
    }

    if tables.is_empty() {
        return Err(MockerError::Schema {
            message: "No CREATE TABLE statements found in SQL file".to_string(),
        });
    }

    Ok(Schema {
        tables,
        database_type: db_type,
    })
}

fn parse_create_table(sql: &str) -> Option<TableSchema> {
    let upper = sql.to_uppercase();
    let start = upper.find("CREATE TABLE")?;
    let after = &sql[start + 12..].trim_start();

    // Skip IF NOT EXISTS
    let after = if after.to_uppercase().starts_with("IF NOT EXISTS") {
        after[13..].trim_start()
    } else {
        after
    };

    // Extract table name (handles quoted and unquoted)
    let (table_name, rest) = extract_identifier(after)?;

    let paren_start = rest.find('(')?;
    let paren_end = rest.rfind(')')?;
    let col_block = &rest[paren_start + 1..paren_end];

    let mut columns = Vec::new();
    let mut primary_keys = Vec::new();
    let mut foreign_keys = Vec::new();

    for part in split_top_level_commas(col_block) {
        let part = part.trim();
        let upper_part = part.to_uppercase();

        if upper_part.starts_with("PRIMARY KEY") {
            // PRIMARY KEY (col1, col2)
            if let Some(inner) = extract_paren_content(part) {
                for col in inner.split(',') {
                    let col = col
                        .trim()
                        .trim_matches(|c| c == '"' || c == '`' || c == '\'');
                    primary_keys.push(col.to_string());
                }
            }
        } else if upper_part.starts_with("FOREIGN KEY") {
            // FOREIGN KEY (col) REFERENCES table(col)
            if let Some(fk) = parse_fk(part) {
                foreign_keys.push(fk);
            }
        } else if upper_part.starts_with("UNIQUE")
            || upper_part.starts_with("INDEX")
            || upper_part.starts_with("KEY ")
            || upper_part.starts_with("CONSTRAINT")
        {
            // Skip
        } else {
            // Column definition
            if let Some(col) = parse_column_def(part) {
                if col.is_primary_key {
                    primary_keys.push(col.name.clone());
                }
                columns.push(col);
            }
        }
    }

    // Mark primary key columns
    for col in &mut columns {
        if primary_keys.contains(&col.name) {
            col.is_primary_key = true;
        }
    }

    Some(TableSchema {
        name: table_name,
        columns,
        primary_keys,
        foreign_keys,
        unique_constraints: vec![],
    })
}

fn extract_identifier(s: &str) -> Option<(String, &str)> {
    let s = s.trim();
    if s.starts_with('"') || s.starts_with('`') {
        let quote = s.chars().next().unwrap();
        let end = s[1..].find(quote)? + 1;
        let name = s[1..end].to_string();
        let rest = &s[end + 1..];
        Some((name, rest))
    } else {
        let end = s
            .find(|c: char| c.is_whitespace() || c == '(')
            .unwrap_or(s.len());
        let name = s[..end].to_string();
        let rest = &s[end..];
        Some((name, rest))
    }
}

fn extract_paren_content(s: &str) -> Option<String> {
    let start = s.find('(')?;
    let end = s.rfind(')')?;
    Some(s[start + 1..end].to_string())
}

fn split_top_level_commas(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut start = 0;

    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }

    parts.push(&s[start..]);
    parts
}

fn parse_column_def(s: &str) -> Option<ColumnSchema> {
    let mut tokens = s.split_whitespace();
    let raw_name = tokens.next()?;
    let name = raw_name.trim_matches(|c| c == '"' || c == '`').to_string();
    let data_type = tokens.next().unwrap_or("text").to_lowercase();

    let upper = s.to_uppercase();
    let is_nullable = !upper.contains("NOT NULL");
    let is_auto_increment = upper.contains("AUTO_INCREMENT")
        || upper.contains("AUTOINCREMENT")
        || upper.contains("SERIAL");
    let is_primary_key = upper.contains("PRIMARY KEY");

    Some(ColumnSchema {
        name,
        data_type,
        is_nullable,
        is_primary_key,
        is_auto_increment,
        max_length: None,
        numeric_precision: None,
        numeric_scale: None,
        default_value: None,
        is_unique: upper.contains("UNIQUE"),
    })
}

fn parse_fk(s: &str) -> Option<ForeignKey> {
    let upper = s.to_uppercase();
    let fk_col_start = upper.find("FOREIGN KEY")? + 11;
    let fk_col = extract_paren_content(&s[fk_col_start..])?
        .trim()
        .to_string();
    let fk_col = fk_col.trim_matches(|c| c == '"' || c == '`').to_string();

    let ref_start = upper.find("REFERENCES")? + 10;
    let rest = s[ref_start..].trim();
    let (ref_table, ref_rest) = extract_identifier(rest)?;
    let ref_col = extract_paren_content(ref_rest)?.trim().to_string();
    let ref_col = ref_col.trim_matches(|c| c == '"' || c == '`').to_string();

    Some(ForeignKey {
        column: fk_col,
        referenced_table: ref_table,
        referenced_column: ref_col,
        constraint_name: None,
    })
}
