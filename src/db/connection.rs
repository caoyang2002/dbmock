use crate::config::{DatabaseConfig, DbType};
use crate::db::types::DbPool;
use crate::errors::{MockerError, Result};
use sqlx::mysql::MySqlPoolOptions;
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::SqlitePoolOptions;

pub async fn create_pool(config: &DatabaseConfig) -> Result<DbPool> {
    let conn_str = config.connection_string();
    match config.db_type {
        DbType::Postgres => {
            let pool = PgPoolOptions::new()
                .max_connections(10)
                .connect(&conn_str)
                .await
                .map_err(|e| MockerError::Connection {
                    message: format!("Failed to connect to PostgreSQL: {}", e),
                })?;
            Ok(DbPool::Postgres(pool))
        }
        DbType::MySQL => {
            let pool = MySqlPoolOptions::new()
                .max_connections(10)
                .connect(&conn_str)
                .await
                .map_err(|e| MockerError::Connection {
                    message: format!("Failed to connect to MySQL: {}", e),
                })?;
            Ok(DbPool::MySQL(pool))
        }
        DbType::SQLite => {
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect(&conn_str)
                .await
                .map_err(|e| MockerError::Connection {
                    message: format!("Failed to connect to SQLite: {}", e),
                })?;
            Ok(DbPool::SQLite(pool))
        }
    }
}
