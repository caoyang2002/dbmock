use sqlx::{Pool, Postgres, MySql, Sqlite};

pub enum DbPool {
    Postgres(Pool<Postgres>),
    MySQL(Pool<MySql>),
    SQLite(Pool<Sqlite>),
}
