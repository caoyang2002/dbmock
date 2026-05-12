use serde::{Deserialize, Serialize};

/// Database connection settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub db_type: DbType,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub database_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DbType {
    Postgres,
    MySQL,
    SQLite,
}

impl std::fmt::Display for DbType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbType::Postgres => write!(f, "postgres"),
            DbType::MySQL => write!(f, "mysql"),
            DbType::SQLite => write!(f, "sqlite"),
        }
    }
}

impl std::str::FromStr for DbType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "postgres" | "postgresql" => Ok(DbType::Postgres),
            "mysql" | "mariadb" => Ok(DbType::MySQL),
            "sqlite" => Ok(DbType::SQLite),
            other => Err(format!("Unknown database type: {}", other)),
        }
    }
}

impl DatabaseConfig {
    pub fn connection_string(&self) -> String {
        if let Some(url) = &self.database_url {
            return url.clone();
        }
        match self.db_type {
            DbType::Postgres => format!(
                "postgresql://{}:{}@{}:{}/{}",
                self.username, self.password, self.host, self.port, self.database
            ),
            DbType::MySQL => format!(
                "mysql://{}:{}@{}:{}/{}",
                self.username, self.password, self.host, self.port, self.database
            ),
            DbType::SQLite => format!("sqlite:{}", self.database),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            db_type: DbType::Postgres,
            host: "localhost".to_string(),
            port: 5432,
            database: String::new(),
            username: String::new(),
            password: String::new(),
            database_url: None,
        }
    }
}
