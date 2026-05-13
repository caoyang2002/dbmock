use async_trait::async_trait;
use sqlx::{MySql, Pool, Row};

use crate::core::driver::DatabaseDriver;
use crate::core::schema::{ColumnSchema, ForeignKey, Schema, TableSchema};
use crate::errors::Result;

pub struct MySqlDriver {
    pool: Pool<MySql>,
    database: String,
}

impl MySqlDriver {
    pub fn new(pool: Pool<MySql>, database: String) -> Self {
        Self { pool, database }
    }
}

#[async_trait]
impl DatabaseDriver for MySqlDriver {
    async fn extract_schema(&self) -> Result<Schema> {
        let tables = self.fetch_tables().await?;
        let mut table_schemas = Vec::new();

        for table_name in tables {
            let columns            = self.fetch_columns(&table_name).await?;
            let primary_keys       = self.fetch_primary_keys(&table_name).await?;
            let foreign_keys       = self.fetch_foreign_keys(&table_name).await?;
            let unique_constraints = self.fetch_unique_constraints(&table_name).await?;

            let columns = columns
                .into_iter()
                .map(|mut col| {
                    col.is_primary_key = primary_keys.contains(&col.name);
                    col
                })
                .collect();

            table_schemas.push(TableSchema {
                name: table_name,
                columns,
                primary_keys,
                foreign_keys,
                unique_constraints,
            });
        }

        Ok(Schema {
            tables: table_schemas,
            database_type: "mysql".to_string(),
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

    /// MySQL does not support RETURNING; re-query after each batch.
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

        // Re-query the last N inserted rows ordered by PK desc.
        let n = statements.len() * 500; // upper bound
        self.query_ids(table, pk_col, n).await
    }

    async fn query_ids(
        &self,
        table: &str,
        column: &str,
        limit: usize,
    ) -> Result<Vec<String>> {
        let sql = format!(
            "SELECT `{}` FROM `{}` WHERE `{}` IS NOT NULL ORDER BY `{}` LIMIT {}",
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

    fn db_type(&self) -> &str { "mysql" }

    async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    async fn close(&self) { self.pool.close().await; }
}

impl MySqlDriver {
    async fn fetch_tables(&self) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT TABLE_NAME FROM information_schema.TABLES \
             WHERE TABLE_SCHEMA = ? AND TABLE_TYPE = 'BASE TABLE' ORDER BY TABLE_NAME",
        )
        .bind(&self.database)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|r| r.get::<String, _>("TABLE_NAME")).collect())
    }

    async fn fetch_columns(&self, table: &str) -> Result<Vec<ColumnSchema>> {
        let rows = sqlx::query(
            "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE, \
                CHARACTER_MAXIMUM_LENGTH, NUMERIC_PRECISION, NUMERIC_SCALE, \
                COLUMN_DEFAULT, EXTRA \
             FROM information_schema.COLUMNS \
             WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? ORDER BY ORDINAL_POSITION",
        )
        .bind(&self.database)
        .bind(table)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| {
            let extra: String = r.get("EXTRA");
            ColumnSchema {
                name:              r.get("COLUMN_NAME"),
                data_type:         r.get("DATA_TYPE"),
                is_nullable:       r.get::<String, _>("IS_NULLABLE") == "YES",
                is_primary_key:    false,
                is_auto_increment: extra.contains("auto_increment"),
                max_length:        r.get("CHARACTER_MAXIMUM_LENGTH"),
                numeric_precision: r.get::<Option<i64>, _>("NUMERIC_PRECISION"),
                numeric_scale:     r.get::<Option<i64>, _>("NUMERIC_SCALE"),
                default_value:     r.get("COLUMN_DEFAULT"),
                is_unique:         false,
            }
        }).collect())
    }

    async fn fetch_primary_keys(&self, table: &str) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT COLUMN_NAME FROM information_schema.KEY_COLUMN_USAGE \
             WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND CONSTRAINT_NAME = 'PRIMARY' \
             ORDER BY ORDINAL_POSITION",
        )
        .bind(&self.database).bind(table)
        .fetch_all(&self.pool).await?;
        Ok(rows.iter().map(|r| r.get::<String, _>("COLUMN_NAME")).collect())
    }

    async fn fetch_foreign_keys(&self, table: &str) -> Result<Vec<ForeignKey>> {
        let rows = sqlx::query(
            "SELECT kcu.COLUMN_NAME, kcu.REFERENCED_TABLE_NAME, \
                kcu.REFERENCED_COLUMN_NAME, kcu.CONSTRAINT_NAME \
             FROM information_schema.KEY_COLUMN_USAGE kcu \
             JOIN information_schema.REFERENTIAL_CONSTRAINTS rc \
                ON kcu.CONSTRAINT_NAME   = rc.CONSTRAINT_NAME \
                AND kcu.TABLE_SCHEMA     = rc.CONSTRAINT_SCHEMA \
             WHERE kcu.TABLE_SCHEMA = ? AND kcu.TABLE_NAME = ?",
        )
        .bind(&self.database).bind(table)
        .fetch_all(&self.pool).await?;

        Ok(rows.iter().map(|r| ForeignKey {
            column:            r.get("COLUMN_NAME"),
            referenced_table:  r.get("REFERENCED_TABLE_NAME"),
            referenced_column: r.get("REFERENCED_COLUMN_NAME"),
            constraint_name:   r.get("CONSTRAINT_NAME"),
        }).collect())
    }

    async fn fetch_unique_constraints(&self, table: &str) -> Result<Vec<Vec<String>>> {
        let rows = sqlx::query(
            "SELECT CONSTRAINT_NAME, COLUMN_NAME FROM information_schema.KEY_COLUMN_USAGE \
             WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? \
               AND CONSTRAINT_NAME != 'PRIMARY' AND REFERENCED_TABLE_NAME IS NULL \
             ORDER BY CONSTRAINT_NAME, ORDINAL_POSITION",
        )
        .bind(&self.database).bind(table)
        .fetch_all(&self.pool).await?;

        let mut map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for row in &rows {
            let c: String = row.get("CONSTRAINT_NAME");
            let k: String = row.get("COLUMN_NAME");
            map.entry(c).or_default().push(k);
        }
        Ok(map.into_values().collect())
    }
}
