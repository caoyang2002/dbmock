use thiserror::Error;

#[derive(Debug, Error)]
pub enum MockerError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Configuration error: {message}")]
    Config { message: String },

    #[error("Schema error: {message}")]
    Schema { message: String },

    #[error("Generator error: {message}")]
    Generator { message: String },

    #[error("Circular dependency detected among tables: {tables:?}")]
    CircularDependency { tables: Vec<String> },

    #[error("Unsupported database type: {db_type}")]
    UnsupportedDatabase { db_type: String },

    #[error("Table not found: {table}")]
    TableNotFound { table: String },

    #[error("Connection error: {message}")]
    Connection { message: String },

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, MockerError>;
