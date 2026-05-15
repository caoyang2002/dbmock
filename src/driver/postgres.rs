use async_trait::async_trait;
use sqlx::{Pool, Postgres, Row};

use crate::core::driver::DatabaseDriver;
use crate::core::schema::{ColumnSchema, ForeignKey, Schema, TableSchema};
use crate::errors::{MockerError, Result};
use std::collections::HashMap;
pub struct PostgresDriver {
    pool: Pool<Postgres>,
}

impl PostgresDriver {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DatabaseDriver for PostgresDriver {
    // ── schema extraction ────────────────────────────────────────────────────

    async fn extract_schema(&self) -> Result<Schema> {
        let tables = self.fetch_tables().await?;
        let mut table_schemas = Vec::new();

        for table_name in tables {
            let columns = self.fetch_columns(&table_name).await?;
            let primary_keys = self.fetch_primary_keys(&table_name).await?;
            let foreign_keys = self.fetch_foreign_keys(&table_name).await?;
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
            database_type: "postgres".to_string(),
        })
    }

    // ── basic execution ──────────────────────────────────────────────────────

    async fn execute_sql(&self, sql: &str) -> Result<u64> {
        let r = sqlx::query(sql).execute(&self.pool).await?;
        Ok(r.rows_affected())
    }

    async fn execute_batch(&self, statements: Vec<String>) -> Result<u64> {
        let mut tx = self.pool.begin().await?;
        let mut total: u64 = 0;
        for sql in statements {
            let r = sqlx::query(&sql).execute(&mut *tx).await?;
            sqlx::query("SET LOCAL statement_timeout = '30s'")
                .execute(&mut *tx)
                .await?;
            total += r.rows_affected();
        }
        tx.commit().await?;
        Ok(total)
    }

    // ── INSERT … RETURNING ───────────────────────────────────────────────────

    async fn execute_batch_returning_ids(
        &self,
        statements: Vec<String>,
        _table: &str,
        pk_col: &str,
    ) -> Result<Vec<String>> {
        let mut tx = self.pool.begin().await?;
        let mut ids: Vec<String> = Vec::new();

        for sql in statements {
            // Append RETURNING <pk_col> to each INSERT.
            let returning_sql = format!("{} RETURNING \"{}\"", sql, pk_col);
            let rows = sqlx::query(&returning_sql).fetch_all(&mut *tx).await?;

            for row in rows {
                // Try integer first (SERIAL / BIGSERIAL), else string.
                let id_str: String = if let Ok(v) = row.try_get::<i64, _>(0) {
                    v.to_string()
                } else if let Ok(v) = row.try_get::<i32, _>(0) {
                    v.to_string()
                } else {
                    let v: String = row.get(0);
                    format!("'{}'", v.replace('\'', "''"))
                };
                ids.push(id_str);
            }
        }

        tx.commit().await?;
        Ok(ids)
    }

    // ── query existing IDs ───────────────────────────────────────────────────

    async fn query_ids(&self, table: &str, column: &str, limit: usize) -> Result<Vec<String>> {
        let sql = format!(
            "SELECT \"{}\" FROM \"{}\" WHERE \"{}\" IS NOT NULL ORDER BY \"{}\" LIMIT {}",
            column, table, column, column, limit
        );
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;

        let ids = rows
            .iter()
            .map(|r| {
                // Try i64, then i32, then string.
                if let Ok(v) = r.try_get::<i64, _>(0) {
                    v.to_string()
                } else if let Ok(v) = r.try_get::<i32, _>(0) {
                    v.to_string()
                } else {
                    let v: String = r.get(0);
                    format!("'{}'", v.replace('\'', "''"))
                }
            })
            .collect();

        Ok(ids)
    }

    fn db_type(&self) -> &str {
        "postgres"
    }

    async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    async fn close(&self) {
        self.pool.close().await;
    }
}

// ── private helpers ──────────────────────────────────────────────────────────

impl PostgresDriver {
    async fn fetch_tables(&self) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT table_name FROM information_schema.tables \
             WHERE table_schema = 'public' AND table_type = 'BASE TABLE' \
             ORDER BY table_name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| r.get::<String, _>("table_name"))
            .collect())
    }

    async fn fetch_columns(&self, table: &str) -> Result<Vec<ColumnSchema>> {
        let rows = sqlx::query(
            "SELECT \
                c.column_name, \
                c.data_type, \
                c.is_nullable, \
                c.character_maximum_length, \
                c.numeric_precision, \
                c.numeric_scale, \
                c.column_default, \
                CASE WHEN c.column_default LIKE 'nextval%' THEN true ELSE false END AS is_auto_increment \
             FROM information_schema.columns c \
             WHERE c.table_schema = 'public' AND c.table_name = $1 \
             ORDER BY c.ordinal_position",
        )
        .bind(table)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| ColumnSchema {
                name: r.get("column_name"),
                data_type: r.get("data_type"),
                is_nullable: r.get::<String, _>("is_nullable") == "YES",
                is_primary_key: false,
                is_auto_increment: r.get("is_auto_increment"),
                max_length: r.get::<Option<i32>, _>("character_maximum_length"),
                numeric_precision: r.get::<Option<i32>, _>("numeric_precision"),
                numeric_scale: r.get::<Option<i32>, _>("numeric_scale"),
                default_value: r.get("column_default"),
                is_unique: false,
            })
            .collect())
    }

    async fn fetch_primary_keys(&self, table: &str) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT kcu.column_name \
             FROM information_schema.table_constraints tc \
             JOIN information_schema.key_column_usage kcu \
                ON tc.constraint_name = kcu.constraint_name \
                AND tc.table_schema   = kcu.table_schema \
             WHERE tc.constraint_type = 'PRIMARY KEY' \
               AND tc.table_schema = 'public' \
               AND tc.table_name   = $1 \
             ORDER BY kcu.ordinal_position",
        )
        .bind(table)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| r.get::<String, _>("column_name"))
            .collect())
    }

    async fn fetch_foreign_keys(&self, table: &str) -> Result<Vec<ForeignKey>> {
        let rows = sqlx::query(
            r#"
            SELECT
                a.attname AS column_name,
                r.relname AS referenced_table,
                f.attname AS referenced_column,
                con.conname AS constraint_name
            FROM pg_constraint con
            JOIN pg_class t ON t.oid = con.conrelid
            JOIN pg_namespace n ON n.oid = t.relnamespace
            JOIN pg_attribute a ON a.attrelid = con.conrelid AND a.attnum = ANY(con.conkey)
            JOIN pg_class r ON r.oid = con.confrelid
            JOIN pg_attribute f ON f.attrelid = con.confrelid AND f.attnum = ANY(con.confkey)
            WHERE t.relname = $1
              AND n.nspname = 'public'
              AND con.contype = 'f'
            "#,
        )
        .bind(table)
        .fetch_all(&self.pool)
        .await?;

        let mut fks = Vec::new();
        for row in rows {
            let column: String = row.get("column_name");
            let referenced_table: String = row.get("referenced_table");
            let referenced_column: String = row.get("referenced_column");
            let constraint_name: Option<String> = row.try_get("constraint_name").ok();

            let normalize_ident = |s: &str| -> String {
                s.trim_matches('"')
                    .split('.')
                    .last()
                    .unwrap_or(s)
                    .to_string()
            };

            fks.push(ForeignKey {
                column: normalize_ident(&column),
                referenced_table: normalize_ident(&referenced_table),
                referenced_column: normalize_ident(&referenced_column),
                constraint_name,
            });
        }
        Ok(fks)
    }
    // 唯一约束和索引
    async fn fetch_unique_constraints(&self, table: &str) -> Result<Vec<Vec<String>>> {
        use std::collections::HashMap;

        let rows = sqlx::query(
            r#"
            -- 显式 UNIQUE 约束
            SELECT
                kcu.constraint_name,
                kcu.column_name,
                kcu.ordinal_position::bigint AS ordinal_position
            FROM information_schema.table_constraints tc
            JOIN information_schema.key_column_usage kcu
                ON tc.constraint_name = kcu.constraint_name
                AND tc.table_schema = kcu.table_schema
            WHERE tc.constraint_type = 'UNIQUE'
              AND tc.table_schema = 'public'
              AND tc.table_name = $1

            UNION ALL

            -- 唯一索引（排除已被显式约束覆盖的，并且排除主键索引）
            SELECT
                i.relname AS constraint_name,
                a.attname AS column_name,
                u.ord::bigint AS ordinal_position
            FROM pg_index idx
            JOIN pg_class i ON i.oid = idx.indexrelid
            JOIN pg_class t ON t.oid = idx.indrelid
            CROSS JOIN LATERAL unnest(idx.indkey) WITH ORDINALITY AS u(attnum, ord)
            JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = u.attnum
            WHERE t.relname = $1
              AND t.relnamespace = (SELECT oid FROM pg_namespace WHERE nspname = 'public')
              AND idx.indisunique = true
              AND NOT EXISTS (
                  SELECT 1 FROM information_schema.table_constraints tc
                  WHERE tc.constraint_name = i.relname
                    AND tc.constraint_type = 'UNIQUE'
              )
              -- 排除主键索引（主键索引名称通常包含 _pkey）
              AND i.relname NOT LIKE '%_pkey'
            "#,
        )
        .bind(table)
        .fetch_all(&self.pool)
        .await?;

        let mut map: HashMap<String, Vec<(String, i64)>> = HashMap::new();
        for row in rows {
            let name: String = row.get(0);
            let col: String = row.get(1);
            let pos: i64 = row.get(2);
            map.entry(name).or_default().push((col, pos));
        }

        let mut result = Vec::new();
        for (_, mut cols) in map {
            cols.sort_by_key(|(_, pos)| *pos);
            result.push(cols.into_iter().map(|(name, _)| name).collect());
        }
        Ok(result)
    }
    //
}
