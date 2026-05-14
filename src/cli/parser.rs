use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "dbmock",
    version = "0.1.0",
    author = "dbmock contributors",
    about = "High-performance database mock data generator",
    long_about = concat!(
        "dbmock reads your database schema and generates realistic mock data\n",
        "that respects foreign-key constraints.\n\n",
        "Typical workflow:\n",
        "  1. dbmock extract  -d <URL> -j schema.json\n",
        "  2. dbmock config   -j schema.json -o mock_config.yml\n",
        "     (edit mock_config.yml to customise column generators)\n",
        "  3. dbmock generate -j schema.json -c mock_config.yml \\\n",
        "                         --rows users=100 --rows posts=500"
    )
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Extract the database schema and save to JSON / SQL files
    Extract(ExtractArgs),

    /// Export an editable YAML config with inferred field strategies
    Config(ConfigArgs),

    /// Generate mock data and insert it into the database
    Generate(GenerateArgs),
}

// ─────────────────────────────────────────────────────────────────────────────
// extract
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct ExtractArgs {
    /// Output JSON schema file path
    #[arg(short = 'j', long = "json", default_value = "schema.json")]
    pub output_json: String,

    /// Output SQL file path (CREATE TABLE statements)
    #[arg(short = 's', long = "sql", default_value = "schema.sql")]
    pub output_sql: String,

    /// Database type: postgres | mysql | sqlite
    #[arg(long, default_value = "postgres")]
    pub db_type: String,

    /// Database host
    #[arg(long, default_value = "localhost")]
    pub db_host: String,

    /// Database port (default: 5432 for postgres, 3306 for mysql)
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

    /// Full connection URL — overrides the individual parameters above
    #[arg(short = 'd', long)]
    pub database_url: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// config
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct ConfigArgs {
    /// Input schema JSON file (produced by `extract`)
    #[arg(short = 'j', long = "json", default_value = "schema.json")]
    pub schema_json: String,

    /// Output YAML config file path
    #[arg(short = 'o', long = "output", default_value = "mock_config.yml")]
    pub output: String,

    /// Only emit config for these tables (comma-separated).
    /// Omit to emit all tables.
    #[arg(long, value_delimiter = ',')]
    pub tables: Vec<String>,

    /// Overwrite the output file if it already exists
    #[arg(long, default_value_t = false)]
    pub force: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// generate
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct GenerateArgs {
    /// Schema JSON file
    #[arg(short = 'j', long = "json")]
    pub schema_json: Option<String>,

    /// Schema SQL file (alternative to JSON)
    #[arg(short = 's', long = "sql")]
    pub schema_sql: Option<String>,

    /// Mock-data config YAML file (produced by `config`, optional).
    /// When omitted, schema-driven defaults are used for every column.
    #[arg(short = 'c', long = "config")]
    pub mock_config: Option<String>,

    /// Row counts per table: TABLE=COUNT  (repeatable)
    #[arg(
        short = 'r',
        long = "rows",
        value_name = "TABLE=COUNT",
        // required = true
    )]
    pub rows: Vec<String>,

    /// Print SQL without executing (no database connection required)
    #[arg(long, default_value_t = false)]
    pub preview: bool,

    /// Database type: postgres | mysql | sqlite
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
