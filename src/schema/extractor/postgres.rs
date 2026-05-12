use crate::core::schema::Schema;
use crate::errors::Result;

/// Generate CREATE TABLE SQL from schema for PostgreSQL
pub fn schema_to_sql(schema: &Schema) -> String {
    let mut lines = Vec::new();

    for table in &schema.tables {
        lines.push(format!("-- Table: {}", table.name));
        lines.push(format!("CREATE TABLE IF NOT EXISTS \"{}\" (", table.name));

        let mut col_defs: Vec<String> = table
            .columns
            .iter()
            .map(|col| {
                let mut def = format!("  \"{}\" {}", col.name, col.data_type);
                if col.is_auto_increment {
                    def = format!("  \"{}\" SERIAL", col.name);
                }
                if !col.is_nullable {
                    def.push_str(" NOT NULL");
                }
                if col.is_primary_key && table.primary_keys.len() == 1 {
                    def.push_str(" PRIMARY KEY");
                }
                def
            })
            .collect();

        if table.primary_keys.len() > 1 {
            let pk_cols: Vec<String> = table
                .primary_keys
                .iter()
                .map(|k| format!("\"{}\"", k))
                .collect();
            col_defs.push(format!("  PRIMARY KEY ({})", pk_cols.join(", ")));
        }

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
