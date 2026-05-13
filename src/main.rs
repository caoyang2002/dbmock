mod cli;
mod config;
mod core;
mod db;
mod driver;
mod errors;
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
use generator::MockEngine;

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
        Commands::Generate(args) => handle_generate(args).await,
    }
}

async fn handle_extract(args: cli::ExtractArgs) -> Result<()> {
    let config = build_db_config(
        &args.db_type, &args.db_host, args.db_port,
        &args.db_name, &args.db_user, &args.db_pass,
        args.database_url.as_deref(),
    )?;

    println!("🔌  Connecting to {} database...", config.db_type);
    let drv = driver::create_driver(&config).await?;
    println!("✅  Connected.");

    println!("🔍  Extracting schema...");
    let schema_obj = drv.extract_schema().await?;
    drv.close().await;

    let table_count = schema_obj.tables.len();

    let json_path = Path::new(&args.output_json);
    schema::save_json(&schema_obj, json_path)?;
    println!("📄  Schema JSON saved to: {}", args.output_json);

    let sql_path = Path::new(&args.output_sql);
    schema::save_sql(&schema_obj, sql_path)?;
    println!("📄  Schema SQL saved to: {}", args.output_sql);

    println!("✨  Extracted {} tables.", table_count);
    Ok(())
}

async fn handle_generate(args: cli::GenerateArgs) -> Result<()> {
    // Parse --rows table=count pairs
    let mut row_counts: HashMap<String, usize> = HashMap::new();
    for entry in &args.rows {
        let parts: Vec<&str> = entry.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(MockerError::Config {
                message: format!("Invalid row spec '{}'. Expected: table=count", entry),
            });
        }
        let count: usize = parts[1].parse().map_err(|_| MockerError::Config {
            message: format!("Invalid count '{}' for '{}'", parts[1], parts[0]),
        })?;
        row_counts.insert(parts[0].to_string(), count);
    }

    // Load schema
    let schema_obj = if let Some(ref p) = args.schema_json {
        println!("📂  Loading schema from JSON: {}", p);
        schema::load_json(Path::new(p))?
    } else if let Some(ref p) = args.schema_sql {
        println!("📂  Loading schema from SQL: {}", p);
        schema::load_sql(Path::new(p))?
    } else {
        let default = Path::new("schema.json");
        if default.exists() {
            println!("📂  Loading schema from default schema.json");
            schema::load_json(default)?
        } else {
            return Err(MockerError::Config {
                message: "No schema file specified. Use --json or --sql.".to_string(),
            });
        }
    };

    println!("📊  Tables to generate:");
    let total: usize = row_counts.values().sum();
    for (t, n) in &row_counts {
        println!("     • {} → {} rows", t, n);
    }
    println!("     Total: {} rows\n", total);

    if args.dry_run {
        println!("🔬  Dry-run mode — printing SQL:\n");
        let engine = DryRunEngine { db_type: schema_obj.database_type.clone() };
        engine.generate(&schema_obj, &row_counts, true).await?;
        println!("\n✅  Dry run complete.");
    } else {
        let config = build_db_config(
            &args.db_type, &args.db_host, args.db_port,
            &args.db_name, &args.db_user, &args.db_pass,
            args.database_url.as_deref(),
        )?;

        println!("🔌  Connecting to {} database...", config.db_type);
        let drv = driver::create_driver(&config).await?;
        println!("✅  Connected.\n");

        let engine = MockEngine::new(drv.clone());
        let report = engine.generate(&schema_obj, &row_counts, false).await?;
        drv.close().await;

        println!("\n📊  Generation complete:");
        println!("     Tables processed : {}", report.tables_processed);
        println!("     Rows inserted     : {}", report.total_rows_inserted);
        if !report.errors.is_empty() {
            println!("     Errors            : {}", report.errors.len());
            for e in &report.errors {
                println!("       - {}", e);
            }
        }
    }

    Ok(())
}

// ── dry-run engine (no DB connection needed) ─────────────────────────────────

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
    ) -> Result<core::generator::GenerationReport> {
        use generator::batch::build_insert_batches;
        use generator::dependency::topological_sort;

        let requested: Vec<String> = row_counts.keys().cloned().collect();
        let sorted = topological_sort(schema, &requested)?;

        let mut report = core::generator::GenerationReport::default();
        let mut fk_pools: HashMap<String, Vec<String>> = HashMap::new();

        for tname in &sorted {
            let count = *row_counts.get(tname).unwrap_or(&0);
            if count == 0 { continue; }

            let ts = schema.get_table(tname).unwrap();
            let self_ref_cols: Vec<String> = ts.foreign_keys.iter()
                .filter(|fk| fk.referenced_table == *tname)
                .map(|fk| fk.column.clone())
                .collect();

            let stmts = build_insert_batches(ts, count, &self.db_type, &fk_pools, &self_ref_cols);
            for s in &stmts {
                println!("{};", s);
                report.sql_statements.push(s.clone());
            }

            // Synthetic sequential pool for downstream FK references.
            fk_pools.insert(tname.clone(), (1..=count).map(|i| i.to_string()).collect());
            report.tables_processed += 1;
        }

        Ok(report)
    }
}

// ── config builder ───────────────────────────────────────────────────────────

fn build_db_config(
    db_type: &str, host: &str, port: Option<u16>,
    db_name: &str, user: &str, pass: &str,
    url: Option<&str>,
) -> Result<DatabaseConfig> {
    let database_url = url.map(|s| s.to_string())
        .or_else(|| std::env::var("DATABASE_URL").ok());

    let parsed_type: DbType = db_type.parse().map_err(|e: String| MockerError::Config {
        message: e,
    })?;

    let default_port = match parsed_type {
        DbType::Postgres => 5432,
        DbType::MySQL    => 3306,
        DbType::SQLite   => 0,
    };

    Ok(DatabaseConfig {
        db_type:      parsed_type,
        host:         host.to_string(),
        port:         port.unwrap_or(default_port),
        database:     db_name.to_string(),
        username:     user.to_string(),
        password:     pass.to_string(),
        database_url,
    })
}
