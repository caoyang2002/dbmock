use sqlx::{MySql, Pool, Postgres, Sqlite};

pub enum DbPool {
    Postgres(Pool<Postgres>),
    MySQL(Pool<MySql>),
    SQLite(Pool<Sqlite>),
}
