use serde::{Deserialize, Serialize};

/// Represents a complete database schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub tables: Vec<TableSchema>,
    pub database_type: String,
}

/// Represents a single table's schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnSchema>,
    pub primary_keys: Vec<String>,
    pub foreign_keys: Vec<ForeignKey>,
    pub unique_constraints: Vec<Vec<String>>,
}

/// Represents a single column's schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub is_primary_key: bool,
    pub is_auto_increment: bool,
    pub max_length: Option<i64>,
    pub numeric_precision: Option<i64>,
    pub numeric_scale: Option<i64>,
    pub default_value: Option<String>,
    pub is_unique: bool,
}

/// Represents a foreign key constraint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKey {
    pub column: String,
    pub referenced_table: String,
    pub referenced_column: String,
    pub constraint_name: Option<String>,
}

impl TableSchema {
    pub fn get_column(&self, name: &str) -> Option<&ColumnSchema> {
        self.columns.iter().find(|c| c.name == name)
    }

    pub fn non_auto_columns(&self) -> Vec<&ColumnSchema> {
        self.columns
            .iter()
            .filter(|c| !c.is_auto_increment)
            .collect()
    }
}

impl Schema {
    pub fn get_table(&self, name: &str) -> Option<&TableSchema> {
        self.tables.iter().find(|t| t.name == name)
    }
}
