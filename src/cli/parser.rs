use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "datamocker",
    version = "0.1.0",
    author = "datamocker contributors",
    about = "High-performance database mock data generator",
    long_about = "datamocker automatically reads your database schema and generates\nrealistic mock data respecting foreign key constraints."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Extract the database schema and save to file(s)
    Extract(ExtractArgs),

    /// Generate mock data from a schema file
    Generate(GenerateArgs),
}

#[derive(Args, Debug)]
pub struct ExtractArgs {
    /// Output JSON file path (custom structured format)
    #[arg(short = 'j', long = "json", default_value = "schema.json")]
    pub output_json: String,

    /// Output SQL file path (CREATE TABLE statements)
    #[arg(short = 's', long = "sql", default_value = "schema.sql")]
    pub output_sql: String,

    /// Database type: postgres, mysql, sqlite
    #[arg(long, default_value = "postgres")]
    pub db_type: String,

    /// Database host
    #[arg(long, default_value = "localhost")]
    pub db_host: String,

    /// Database port (defaults: 5432 for postgres, 3306 for mysql)
    #[arg(long)]
    pub db_port: Option<u16>,

    /// Database name
    #[arg(long, default_value = "")]
    pub db_name: String,

    /// Database user
    #[arg(long, default_value = "")]
    pub db_user: String,

    /// Database password
    #[arg(long, default_value = "")]
    pub db_pass: String,

    /// Full connection URL (overrides individual parameters)
    #[arg(short = 'd', long)]
    pub database_url: Option<String>,
}

#[derive(Args, Debug)]
pub struct GenerateArgs {
    /// Schema JSON file path
    #[arg(short = 'j', long = "json")]
    pub schema_json: Option<String>,

    /// Schema SQL file path
    #[arg(short = 's', long = "sql")]
    pub schema_sql: Option<String>,

    /// Row counts per table, format: table_name=count (repeatable)
    #[arg(short = 'r', long = "rows", value_name = "TABLE=COUNT", required = true)]
    pub rows: Vec<String>,

    /// Dry-run: print SQL without executing
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    /// Database type
    #[arg(long, default_value = "postgres")]
    pub db_type: String,

    /// Database host
    #[arg(long, default_value = "localhost")]
    pub db_host: String,

    /// Database port
    #[arg(long)]
    pub db_port: Option<u16>,

    /// Database name
    #[arg(long, default_value = "")]
    pub db_name: String,

    /// Database user
    #[arg(long, default_value = "")]
    pub db_user: String,

    /// Database password
    #[arg(long, default_value = "")]
    pub db_pass: String,

    /// Full connection URL
    #[arg(short = 'd', long)]
    pub database_url: Option<String>,
}
