use crate::core::schema::Schema;

/// Generate CREATE TABLE SQL from schema for SQLite
pub fn schema_to_sql(schema: &Schema) -> String {
    let mut lines = Vec::new();

    for table in &schema.tables {
        lines.push(format!("-- Table: {}", table.name));
        lines.push(format!("CREATE TABLE IF NOT EXISTS \"{}\" (", table.name));

        let mut col_defs: Vec<String> = table
            .columns
            .iter()
            .map(|col| {
                let type_str = if col.is_auto_increment {
                    "INTEGER".to_string()
                } else {
                    col.data_type.to_uppercase()
                };

                let mut def = format!("  \"{}\" {}", col.name, type_str);

                if col.is_primary_key && table.primary_keys.len() == 1 {
                    def.push_str(" PRIMARY KEY");
                    if col.is_auto_increment {
                        def.push_str(" AUTOINCREMENT");
                    }
                }
                if !col.is_nullable && !col.is_primary_key {
                    def.push_str(" NOT NULL");
                }
                def
            })
            .collect();

        for fk in &table.foreign_keys {
            col_defs.push(format!(
                "  FOREIGN KEY (\"{}\") REFERENCES \"{}\"(\"{}\")",
                fk.column, fk.referenced_table, fk.referenced_column
            ));
        }

        lines.push(col_defs.join(",\n"));
        lines.push(");\n".to_string());
    }

    lines.join("\n")
}
