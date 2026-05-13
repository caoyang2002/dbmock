pub mod mysql;
pub mod postgres;
pub mod sqlite;

use std::sync::Arc;
use crate::config::{DatabaseConfig, DbType};
use crate::core::driver::DatabaseDriver;
use crate::db::DbPool;
use crate::errors::Result;

pub use mysql::MySqlDriver;
pub use postgres::PostgresDriver;
pub use sqlite::SqliteDriver;

/// Factory: create the appropriate driver from config
pub async fn create_driver(config: &DatabaseConfig) -> Result<Arc<dyn DatabaseDriver>> {
    use crate::db::connection::create_pool;

    let pool = create_pool(config).await?;
    let driver: Arc<dyn DatabaseDriver> = match pool {
        DbPool::Postgres(p) => Arc::new(PostgresDriver::new(p)),
        DbPool::MySQL(p) => Arc::new(MySqlDriver::new(p, config.database.clone())),
        DbPool::SQLite(p) => Arc::new(SqliteDriver::new(p)),
    };

    driver.ping().await?;
    Ok(driver)
}
