use async_trait::async_trait;
use crate::errors::Result;
use crate::core::schema::Schema;

/// Trait that all database drivers must implement
#[async_trait]
pub trait DatabaseDriver: Send + Sync {
    /// Extract schema from the connected database
    async fn extract_schema(&self) -> Result<Schema>;

    /// Execute a raw SQL statement
    async fn execute_sql(&self, sql: &str) -> Result<u64>;

    /// Execute multiple SQL statements in a transaction
    async fn execute_batch(&self, statements: Vec<String>) -> Result<u64>;

    /// Get database type identifier
    fn db_type(&self) -> &str;

    /// Test connectivity
    async fn ping(&self) -> Result<()>;

    /// Close the connection pool
    async fn close(&self);
}
