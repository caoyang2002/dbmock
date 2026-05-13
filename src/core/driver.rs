use async_trait::async_trait;
use crate::errors::Result;
use crate::core::schema::Schema;
/// 数据库驱动
/// Trait that all database drivers must implement
#[async_trait]
pub trait DatabaseDriver: Send + Sync {
    /// Extract schema from the connected database
    async fn extract_schema(&self) -> Result<Schema>;

    /// Execute a raw SQL statement
    async fn execute_sql(&self, sql: &str) -> Result<u64>;

    /// Execute multiple SQL statements in a transaction (single transaction).
    async fn execute_batch(&self, statements: Vec<String>) -> Result<u64>;

    /// Execute INSERT statements and return the generated PK values.
    /// For PostgreSQL: appends RETURNING pk_col.
    /// For others: re-queries MAX/last N rows after insert.
    async fn execute_batch_returning_ids(
        &self,
        statements: Vec<String>,
        table: &str,
        pk_col: &str,
    ) -> Result<Vec<String>>;

    /// Query up to `limit` existing values of `column` from `table`.
    /// Returns SQL literal strings ready for use in INSERT VALUES
    /// (e.g. "42" for integers, "'abc-uuid'" for text/uuid columns).
    async fn query_ids(
        &self,
        table: &str,
        column: &str,
        limit: usize,
    ) -> Result<Vec<String>>;

    /// Get database type identifier
    fn db_type(&self) -> &str;

    /// Test connectivity
    async fn ping(&self) -> Result<()>;

    /// Close the connection pool
    async fn close(&self);
}
