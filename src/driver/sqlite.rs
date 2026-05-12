use crate::core::driver::DatabaseDriver;
use crate::core::schema::{ColumnSchema, ForeignKey, Schema, TableSchema};
use crate::errors::Result;
use async_trait::async_trait;
use sqlx::{Pool, Row, Sqlite};

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
            let columns = self.fetch_columns(&table_name).await?;
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
        let result = sqlx::query(sql).execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    async fn execute_batch(&self, statements: Vec<String>) -> Result<u64> {
        let mut tx = self.pool.begin().await?;
        let mut total: u64 = 0;

        for sql in statements {
            let result = sqlx::query(&sql).execute(&mut *tx).await?;
            total += result.rows_affected();
        }

        tx.commit().await?;
        Ok(total)
    }

    fn db_type(&self) -> &str {
        "sqlite"
    }

    async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    async fn close(&self) {
        self.pool.close().await;
    }
}

impl SqliteDriver {
    async fn fetch_tables(&self) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| r.get::<String, _>("name")).collect())
    }

    async fn fetch_columns(&self, table: &str) -> Result<Vec<ColumnSchema>> {
        // SQLite pragma returns: cid, name, type, notnull, dflt_value, pk
        let sql = format!("PRAGMA table_info(\"{}\")", table);
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;

        let cols = rows
            .iter()
            .map(|r| {
                let data_type: String = r.get("type");
                let is_pk: i32 = r.get("pk");
                let notnull: i32 = r.get("notnull");
                let is_auto = data_type.to_uppercase() == "INTEGER" && is_pk == 1;

                ColumnSchema {
                    name: r.get("name"),
                    data_type: data_type.to_lowercase(),
                    is_nullable: notnull == 0,
                    is_primary_key: is_pk > 0,
                    is_auto_increment: is_auto,
                    max_length: None,
                    numeric_precision: None,
                    numeric_scale: None,
                    default_value: r.get("dflt_value"),
                    is_unique: false,
                }
            })
            .collect();

        Ok(cols)
    }

    async fn fetch_foreign_keys(&self, table: &str) -> Result<Vec<ForeignKey>> {
        let sql = format!("PRAGMA foreign_key_list(\"{}\")", table);
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;

        Ok(rows
            .iter()
            .map(|r| ForeignKey {
                column: r.get("from"),
                referenced_table: r.get("table"),
                referenced_column: r.get("to"),
                constraint_name: None,
            })
            .collect())
    }
}
