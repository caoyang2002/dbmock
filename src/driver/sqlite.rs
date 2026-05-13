use async_trait::async_trait;
use sqlx::{Pool, Row, Sqlite};

use crate::core::driver::DatabaseDriver;
use crate::core::schema::{ColumnSchema, ForeignKey, Schema, TableSchema};
use crate::errors::Result;

pub struct SqliteDriver {
    pool: Pool<Sqlite>,
}

impl SqliteDriver {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DatabaseDriver for SqliteDriver {
    async fn extract_schema(&self) -> Result<Schema> {
        let tables = self.fetch_tables().await?;
        let mut table_schemas = Vec::new();

        for table_name in tables {
            let columns      = self.fetch_columns(&table_name).await?;
            let foreign_keys = self.fetch_foreign_keys(&table_name).await?;
            let primary_keys: Vec<String> = columns
                .iter()
                .filter(|c| c.is_primary_key)
                .map(|c| c.name.clone())
                .collect();

            table_schemas.push(TableSchema {
                name: table_name,
                columns,
                primary_keys,
                foreign_keys,
                unique_constraints: vec![],
            });
        }

        Ok(Schema {
            tables: table_schemas,
            database_type: "sqlite".to_string(),
        })
    }

    async fn execute_sql(&self, sql: &str) -> Result<u64> {
        let r = sqlx::query(sql).execute(&self.pool).await?;
        Ok(r.rows_affected())
    }

    async fn execute_batch(&self, statements: Vec<String>) -> Result<u64> {
        let mut tx = self.pool.begin().await?;
        let mut total: u64 = 0;
        for sql in statements {
            let r = sqlx::query(&sql).execute(&mut *tx).await?;
            total += r.rows_affected();
        }
        tx.commit().await?;
        Ok(total)
    }

    /// SQLite supports RETURNING since 3.35.0; re-query as fallback.
    async fn execute_batch_returning_ids(
        &self,
        statements: Vec<String>,
        table: &str,
        pk_col: &str,
    ) -> Result<Vec<String>> {
        let mut tx = self.pool.begin().await?;
        for sql in &statements {
            sqlx::query(sql).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        let n = statements.len() * 500;
        self.query_ids(table, pk_col, n).await
    }

    async fn query_ids(
        &self,
        table: &str,
        column: &str,
        limit: usize,
    ) -> Result<Vec<String>> {
        let sql = format!(
            "SELECT \"{}\" FROM \"{}\" WHERE \"{}\" IS NOT NULL ORDER BY \"{}\" LIMIT {}",
            column, table, column, column, limit
        );
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;

        Ok(rows.iter().map(|r| {
            if let Ok(v) = r.try_get::<i64, _>(0) {
                v.to_string()
            } else {
                let v: String = r.get(0);
                format!("'{}'", v.replace('\'', "''"))
            }
        }).collect())
    }

    fn db_type(&self) -> &str { "sqlite" }

    async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    async fn close(&self) { self.pool.close().await; }
}

impl SqliteDriver {
    async fn fetch_tables(&self) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT name FROM sqlite_master \
             WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .fetch_all(&self.pool).await?;
        Ok(rows.iter().map(|r| r.get::<String, _>("name")).collect())
    }

    async fn fetch_columns(&self, table: &str) -> Result<Vec<ColumnSchema>> {
        let sql = format!("PRAGMA table_info(\"{}\")", table);
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;

        Ok(rows.iter().map(|r| {
            let dt: String  = r.get("type");
            let is_pk: i32  = r.get("pk");
            let notnull: i32 = r.get("notnull");
            let is_auto = dt.to_uppercase() == "INTEGER" && is_pk == 1;

            ColumnSchema {
                name:              r.get("name"),
                data_type:         dt.to_lowercase(),
                is_nullable:       notnull == 0,
                is_primary_key:    is_pk > 0,
                is_auto_increment: is_auto,
                max_length:        None,
                numeric_precision: None,
                numeric_scale:     None,
                default_value:     r.get("dflt_value"),
                is_unique:         false,
            }
        }).collect())
    }

    async fn fetch_foreign_keys(&self, table: &str) -> Result<Vec<ForeignKey>> {
        let sql = format!("PRAGMA foreign_key_list(\"{}\")", table);
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;

        Ok(rows.iter().map(|r| ForeignKey {
            column:            r.get("from"),
            referenced_table:  r.get("table"),
            referenced_column: r.get("to"),
            constraint_name:   None,
        }).collect())
    }
}
