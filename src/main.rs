mod cli;
mod config;
mod core;
mod datapool;
mod db;
mod driver;
mod errors;
mod fieldconfig;
mod generator;
mod schema;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use clap::Parser;

use cli::{Cli, Commands};
use config::{DatabaseConfig, DbType};
use core::generator::DataGenerator;
use errors::{MockerError, Result};
use fieldconfig::infer::MockConfig;
use generator::MockEngine;

// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("❌  Error: {}", e);
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Extract(args) => handle_extract(args).await,
        Commands::Config(args) => handle_config(args).await,
        Commands::Generate(args) => handle_generate(args).await,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// extract
// ─────────────────────────────────────────────────────────────────────────────

async fn handle_extract(args: cli::ExtractArgs) -> Result<()> {
    let cfg = build_db_config(
        &args.db_type,
        &args.db_host,
        args.db_port,
        &args.db_name,
        &args.db_user,
        &args.db_pass,
        args.database_url.as_deref(),
    )?;

    println!("🔌  Connecting to {} database...", cfg.db_type);
    let drv = driver::create_driver(&cfg).await?;
    println!("✅  Connected.");

    println!("🔍  Extracting schema...");
    let schema_obj = drv.extract_schema().await?;
    drv.close().await;

    let table_count = schema_obj.tables.len();

    schema::save_json(&schema_obj, Path::new(&args.output_json))?;
    println!("📄  Schema JSON saved → {}", args.output_json);

    schema::save_sql(&schema_obj, Path::new(&args.output_sql))?;
    println!("📄  Schema SQL  saved → {}", args.output_sql);

    println!("✨  Extracted {} tables.", table_count);
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// config
// ─────────────────────────────────────────────────────────────────────────────

async fn handle_config(args: cli::ConfigArgs) -> Result<()> {
    // Load schema
    let schema_path = Path::new(&args.schema_json);
    if !schema_path.exists() {
        return Err(MockerError::Config {
            message: format!(
                "Schema file '{}' not found. Run `datamocker extract` first.",
                args.schema_json
            ),
        });
    }
    let schema_obj = schema::load_json(schema_path)?;

    // Check output file
    let out_path = Path::new(&args.output);
    if out_path.exists() && !args.force {
        return Err(MockerError::Config {
            message: format!(
                "Config file '{}' already exists. Use --force to overwrite.",
                args.output
            ),
        });
    }

    // If --tables was given, filter schema down to requested tables only.
    let filtered_schema = if args.tables.is_empty() {
        schema_obj
    } else {
        let tables: Vec<_> = schema_obj
            .tables
            .into_iter()
            .filter(|t| args.tables.contains(&t.name))
            .collect();

        let missing: Vec<_> = args
            .tables
            .iter()
            .filter(|name| !tables.iter().any(|t| &t.name == *name))
            .collect();

        if !missing.is_empty() {
            eprintln!(
                "⚠️  Warning: table(s) not found in schema and will be skipped: {}",
                missing
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        core::schema::Schema {
            tables,
            database_type: schema_obj.database_type,
        }
    };

    // Export config YAML
    fieldconfig::serialize::export_config(&filtered_schema, out_path)?;

    println!("✅  Config exported → {}", args.output);
    println!(
        "   {} table(s), {} column(s) total",
        filtered_schema.tables.len(),
        filtered_schema
            .tables
            .iter()
            .map(|t| t.columns.len())
            .sum::<usize>()
    );
    println!();
    println!("   Edit the file to customise field generators, then run:");
    println!(
        "   datamocker generate -j {} -c {} --rows <table>=<count>",
        args.schema_json, args.output
    );

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// generate
// ─────────────────────────────────────────────────────────────────────────────

async fn handle_generate(args: cli::GenerateArgs) -> Result<()> {
    // ── parse --rows ─────────────────────────────────────────────────────────
    let mut row_counts: HashMap<String, usize> = HashMap::new();
    for entry in &args.rows {
        let parts: Vec<&str> = entry.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(MockerError::Config {
                message: format!(
                    "Invalid --rows value '{}'. Expected format: table=count",
                    entry
                ),
            });
        }
        let count: usize = parts[1].parse().map_err(|_| MockerError::Config {
            message: format!("Invalid row count '{}' for table '{}'", parts[1], parts[0]),
        })?;
        row_counts.insert(parts[0].to_string(), count);
    }

    // ── load schema ──────────────────────────────────────────────────────────
    let schema_obj = load_schema(args.schema_json.as_deref(), args.schema_sql.as_deref())?;

    // ── load mock config (optional) ──────────────────────────────────────────
    let mock_config: Option<MockConfig> = if let Some(ref cfg_path) = args.mock_config {
        let path = Path::new(cfg_path);
        if !path.exists() {
            return Err(MockerError::Config {
                message: format!("Mock config file '{}' not found.", cfg_path),
            });
        }
        println!("⚙️   Loading mock config: {}", cfg_path);
        let mc = fieldconfig::serialize::load_config(path)?;
        Some(mc)
    } else {
        None
    };

    // ── summary ──────────────────────────────────────────────────────────────
    println!("📊  Tables to generate:");
    let total: usize = row_counts.values().sum();
    let mut sorted_display: Vec<(&String, &usize)> = row_counts.iter().collect();
    sorted_display.sort_by_key(|(k, _)| k.as_str());
    for (t, n) in &sorted_display {
        println!("     • {} → {} rows", t, n);
    }
    println!("     Total: {} rows", total);

    if mock_config.is_some() {
        println!(
            "     Config: {} (active)",
            args.mock_config.as_deref().unwrap_or("")
        );
    }
    println!();

    // ── dry run ──────────────────────────────────────────────────────────────
    if args.dry_run {
        println!("🔬  Dry-run mode — SQL preview:\n");
        let engine = DryRunEngine {
            db_type: schema_obj.database_type.clone(),
        };
        engine
            .generate(&schema_obj, &row_counts, true, mock_config.as_ref())
            .await?;
        println!("\n✅  Dry run complete.");
        return Ok(());
    }

    // ── real run ─────────────────────────────────────────────────────────────
    let db_cfg = build_db_config(
        &args.db_type,
        &args.db_host,
        args.db_port,
        &args.db_name,
        &args.db_user,
        &args.db_pass,
        args.database_url.as_deref(),
    )?;

    println!("🔌  Connecting to {} database...", db_cfg.db_type);
    let drv = driver::create_driver(&db_cfg).await?;
    println!("✅  Connected.\n");

    let engine = MockEngine::new(drv.clone());
    let report = engine
        .generate(&schema_obj, &row_counts, false, mock_config.as_ref())
        .await?;

    drv.close().await;

    // ── report ───────────────────────────────────────────────────────────────
    println!("\n📊  Generation complete:");
    println!("     Tables processed : {}", report.tables_processed);
    println!("     Rows inserted     : {}", report.total_rows_inserted);
    if report.errors.is_empty() {
        println!("     Errors            : none");
    } else {
        println!("     Errors            : {}", report.errors.len());
        for e in &report.errors {
            println!("       ⚠  {}", e);
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// DryRunEngine — runs generate without a DB connection
// ─────────────────────────────────────────────────────────────────────────────

struct DryRunEngine {
    db_type: String,
}

#[async_trait::async_trait]
impl DataGenerator for DryRunEngine {
    async fn generate(
        &self,
        schema: &core::schema::Schema,
        row_counts: &HashMap<String, usize>,
        _dry_run: bool,
        mock_config: Option<&MockConfig>,
    ) -> Result<core::generator::GenerationReport> {
        use fieldconfig::generate::reset_unique_counters;
        use generator::batch::build_insert_batches;
        use generator::dependency::topological_sort;

        reset_unique_counters();

        let requested: Vec<String> = row_counts.keys().cloned().collect();
        let sorted = topological_sort(schema, &requested)?;

        let mut report = core::generator::GenerationReport::default();
        let mut fk_pools: HashMap<String, Vec<String>> = HashMap::new();

        for tname in &sorted {
            let count = *row_counts.get(tname).unwrap_or(&0);
            if count == 0 {
                continue;
            }

            let ts = schema.get_table(tname).unwrap();

            let self_ref_cols: Vec<String> = ts
                .foreign_keys
                .iter()
                .filter(|fk| fk.referenced_table == *tname)
                .map(|fk| fk.column.clone())
                .collect();

            let table_cfg = mock_config.and_then(|mc| mc.get(tname));

            let stmts = build_insert_batches(
                ts,
                count,
                &self.db_type,
                &fk_pools,
                &self_ref_cols,
                table_cfg,
            );

            for s in &stmts {
                println!("{};", s);
                report.sql_statements.push(s.clone());
            }

            // Synthetic pool for downstream FK columns
            fk_pools.insert(tname.clone(), (1..=count).map(|i| i.to_string()).collect());
            report.tables_processed += 1;
        }

        Ok(report)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Utilities
// ─────────────────────────────────────────────────────────────────────────────

/// Load schema from JSON or SQL path, falling back to default schema.json.
fn load_schema(json_path: Option<&str>, sql_path: Option<&str>) -> Result<core::schema::Schema> {
    if let Some(p) = json_path {
        println!("📂  Loading schema from JSON: {}", p);
        return schema::load_json(Path::new(p));
    }
    if let Some(p) = sql_path {
        println!("📂  Loading schema from SQL: {}", p);
        return schema::load_sql(Path::new(p));
    }
    let default = Path::new("schema.json");
    if default.exists() {
        println!("📂  Loading schema from default: schema.json");
        return schema::load_json(default);
    }
    Err(MockerError::Config {
        message: "No schema file specified. Use --json or --sql, or run `extract` first.".into(),
    })
}

fn build_db_config(
    db_type: &str,
    host: &str,
    port: Option<u16>,
    db_name: &str,
    user: &str,
    pass: &str,
    url: Option<&str>,
) -> Result<DatabaseConfig> {
    let database_url = url
        .map(|s| s.to_string())
        .or_else(|| std::env::var("DATABASE_URL").ok());

    let parsed_type: DbType = db_type
        .parse()
        .map_err(|e: String| MockerError::Config { message: e })?;

    let default_port = match parsed_type {
        DbType::Postgres => 5432,
        DbType::MySQL => 3306,
        DbType::SQLite => 0,
    };

    Ok(DatabaseConfig {
        db_type: parsed_type,
        host: host.to_string(),
        port: port.unwrap_or(default_port),
        database: db_name.to_string(),
        username: user.to_string(),
        password: pass.to_string(),
        database_url,
    })
}
