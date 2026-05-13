// 模块声明：分别对应命令行、配置、核心抽象、数据库连接、驱动、错误处理、生成器、结构处理、数据池、字段配置
mod cli;
mod config;
mod core;
mod db;
mod driver;
mod errors;
mod generator;
mod schema;

mod datapool;
mod fieldconfig;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use clap::Parser;

use cli::{Cli, Commands};
use config::{DatabaseConfig, DbType};
use core::generator::DataGenerator;
use errors::{MockerError, Result};
use generator::MockEngine;

/// 程序入口：异步运行时（tokio）
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("❌  Error: {}", e);
        std::process::exit(1);
    }
}

/// 命令分发函数：根据子命令调用不同的处理函数
async fn run() -> Result<()> {
    let cli = Cli::parse();                     // 解析命令行参数
    match cli.command {
        Commands::Extract(args) => handle_extract(args).await,   // 提取数据库结构
        Commands::Generate(args) => handle_generate(args).await, // 生成模拟数据
        Commands::Config(args) => handle_config(args).await,           // 处理配置（当前未完整实现）
    }
}

/// 处理 `extract` 子命令：连接数据库，提取表结构，保存为 JSON 和 SQL 文件
async fn handle_extract(args: cli::ExtractArgs) -> Result<()> {
    // 根据命令行参数或 URL 构建数据库配置
    let config = build_db_config(
        &args.db_type, &args.db_host, args.db_port,
        &args.db_name, &args.db_user, &args.db_pass,
        args.database_url.as_deref(),
    )?;

    println!("🔌  Connecting to {} database...", config.db_type);
    let drv = driver::create_driver(&config).await?;   // 创建对应数据库的驱动
    println!("✅  Connected.");

    println!("🔍  Extracting schema...");
    let schema_obj = drv.extract_schema().await?;      // 提取完整结构
    drv.close().await;                                 // 关闭连接

    let table_count = schema_obj.tables.len();

    // 保存为自定义 JSON 格式
    let json_path = Path::new(&args.output_json);
    schema::save_json(&schema_obj, json_path)?;
    println!("📄  Schema JSON saved to: {}", args.output_json);

    // 保存为 SQL DDL（CREATE TABLE）
    let sql_path = Path::new(&args.output_sql);
    schema::save_sql(&schema_obj, sql_path)?;
    println!("📄  Schema SQL saved to: {}", args.output_sql);

    println!("✨  Extracted {} tables.", table_count);
    Ok(())
}

/// 处理 `config` 子命令：目前为占位实现，预期用于生成或更新字段级配置 YAML
/// 生成字段级配置 YAML 文件
/// 处理 `config` 子命令：根据 schema 生成字段级配置 YAML 文件
/// 处理 `config` 子命令：根据 schema 生成字段级配置 YAML 文件
async fn handle_config(args: cli::ConfigArgs) -> Result<()> {
    let schema_path = Path::new(&args.schema);
    if !schema_path.exists() {
        return Err(MockerError::Config {
            message: format!("Schema file not found: {}", args.schema),
        });
    }
    let schema = schema::load_json(schema_path)?;
    let out_path = Path::new(&args.output);
    fieldconfig::serialize::export_config(&schema, out_path)?;
    println!("✅  Mock config written to: {}", out_path.display());
    Ok(())
}

/// 处理 `generate` 子命令：加载结构文件，根据行数要求生成并插入模拟数据
// async fn handle_generate(args: cli::GenerateArgs) -> Result<()> {
//     // 1. 解析 --rows 参数，例如 users=100 posts=500
//     let mut row_counts: HashMap<String, usize> = HashMap::new();
//     for entry in &args.rows {
//         let parts: Vec<&str> = entry.splitn(2, '=').collect();
//         if parts.len() != 2 {
//             return Err(MockerError::Config {
//                 message: format!("Invalid row spec '{}'. Expected: table=count", entry),
//             });
//         }
//         let count: usize = parts[1].parse().map_err(|_| MockerError::Config {
//             message: format!("Invalid count '{}' for '{}'", parts[1], parts[0]),
//         })?;
//         row_counts.insert(parts[0].to_string(), count);
//     }
//
//     // 2. 加载结构（JSON 或 SQL 文件）
//     let schema_obj = if let Some(ref p) = args.schema_json {
//         println!("📂  Loading schema from JSON: {}", p);
//         schema::load_json(Path::new(p))?
//     } else if let Some(ref p) = args.schema_sql {
//         println!("📂  Loading schema from SQL: {}", p);
//         schema::load_sql(Path::new(p))?
//     } else {
//         let default = Path::new("schema.json");
//         if default.exists() {
//             println!("📂  Loading schema from default schema.json");
//             schema::load_json(default)?
//         } else {
//             return Err(MockerError::Config {
//                 message: "No schema file specified. Use --json or --sql.".to_string(),
//             });
//         }
//     };
//
//     // 3. 打印待生成的表及行数
//     println!("📊  Tables to generate:");
//     let total: usize = row_counts.values().sum();
//     for (t, n) in &row_counts {
//         println!("     • {} → {} rows", t, n);
//     }
//     println!("     Total: {} rows\n", total);
//
//     // 4. 根据是否 preview（试运行）选择执行路径
//     if args.preview {
//         println!("🔬  Dry-run mode — printing SQL:\n");
//         let engine = DryRunEngine { db_type: schema_obj.database_type.clone() };
//         engine.generate(&schema_obj, &row_counts, true).await?;
//         println!("\n✅  Dry run complete.");
//     } else {
//         // 实际连接数据库并执行插入
//         let config = build_db_config(
//             &args.db_type, &args.db_host, args.db_port,
//             &args.db_name, &args.db_user, &args.db_pass,
//             args.database_url.as_deref(),
//         )?;
//
//         println!("🔌  Connecting to {} database...", config.db_type);
//         let drv = driver::create_driver(&config).await?;
//         println!("✅  Connected.\n");
//
//         let engine = MockEngine::new(drv.clone());
//         let report = engine.generate(&schema_obj, &row_counts, false).await?;
//         drv.close().await;
//
//         // 输出生成报告
//         println!("\n📊  Generation complete:");
//         println!("     Tables processed : {}", report.tables_processed);
//         println!("     Rows inserted     : {}", report.total_rows_inserted);
//         if !report.errors.is_empty() {
//             println!("     Errors            : {}", report.errors.len());
//             for e in &report.errors {
//                 println!("       - {}", e);
//             }
//         }
//     }
//
//     Ok(())
// }


/// 处理 `generate` 子命令：加载结构文件，根据行数要求生成并插入模拟数据
/// 支持可选的字段级 YAML 配置（--config）
async fn handle_generate(args: cli::GenerateArgs) -> Result<()> {
    // 1. 解析 --rows 参数，例如 users=100 posts=500
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

    // 2. 加载数据库结构（JSON 或 SQL 文件）
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

    // 3. 打印待生成的表及行数
    println!("📊  Tables to generate:");
    let total: usize = row_counts.values().sum();
    for (t, n) in &row_counts {
        println!("     • {} → {} rows", t, n);
    }
    println!("     Total: {} rows\n", total);

    // 4. 加载字段级配置 YAML（如果提供了 --config 参数）
    let mock_config = if !args.config.is_empty() && Path::new(&args.config).exists() {
        println!("📄  Loading field config from: {}", args.config);
        Some(fieldconfig::serialize::load_config(Path::new(&args.config))?)
    } else if !args.config.is_empty() {
        // 配置文件存在但内容为空或不存在？我们当作未提供
        println!("⚠️  Config file '{}' not found, using default schema-driven generation", args.config);
        None
    } else {
        None
    };

    // 5. 根据 preview 模式选择执行路径
    if args.preview {
        println!("🔬  Dry-run mode — printing SQL:\n");
        let engine = DryRunEngine {
            db_type: schema_obj.database_type.clone(),
            mock_config,   // 移动所有权
        };
        engine.generate(&schema_obj, &row_counts, true, None).await?;
        println!("\n✅  Dry run complete.");
    } else {
        // 实际连接数据库并执行插入
        let db_config = build_db_config(
            &args.db_type, &args.db_host, args.db_port,
            &args.db_name, &args.db_user, &args.db_pass,
            args.database_url.as_deref(),
        )?;

        println!("🔌  Connecting to {} database...", db_config.db_type);
        let drv = driver::create_driver(&db_config).await?;
        println!("✅  Connected.\n");

        // MockEngine::new 需增加 mock_config 参数
        let engine = MockEngine::new(drv.clone(), mock_config);
        // let report = engine.generate(&schema_obj, &row_counts, false).await?;
        let report = engine.generate(&schema_obj, &row_counts, false, None).await?;
        drv.close().await;

        // 输出生成报告
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

// ── 试运行引擎：仅生成 SQL 语句，不实际连接数据库 ─────────────────────────────────

/// 试运行引擎结构体，不需要数据库连接，只用于输出 SQL
struct DryRunEngine {
    db_type: String,   // 数据库类型（影响 SQL 方言）
    mock_config: Option<fieldconfig::MockConfig>,
}

#[async_trait::async_trait]
impl DataGenerator for DryRunEngine {
    /// 生成模拟数据的 SQL 语句（不执行）
    async fn generate(
        &self,
        schema: &core::schema::Schema,
        row_counts: &HashMap<String, usize>,
        _preview: bool,
        _mock_config: Option<&fieldconfig::MockConfig>,
    ) -> Result<core::generator::GenerationReport> {
        use generator::batch::build_insert_batches;
        use generator::dependency::topological_sort;

        // 对表进行拓扑排序，保证外键依赖顺序
        let requested: Vec<String> = row_counts.keys().cloned().collect();
        let sorted = topological_sort(schema, &requested)?;

        let mut report = core::generator::GenerationReport::default();
        let mut fk_pools: HashMap<String, Vec<String>> = HashMap::new(); // 记录已生成的主键池

        for tname in &sorted {
            let count = *row_counts.get(tname).unwrap_or(&0);
            if count == 0 { continue; }

            let ts = schema.get_table(tname).unwrap();

            // 找出自引用外键列（比如 parent_id 指向同一张表）
            let self_ref_cols: Vec<String> = ts.foreign_keys.iter()
                .filter(|fk| fk.referenced_table == *tname)
                .map(|fk| fk.column.clone())
                .collect();

            // 生成批量插入语句
            let stmts = build_insert_batches(ts, count, &self.db_type, &fk_pools, &self_ref_cols,  None);
            for s in &stmts {
                println!("{};", s);
                report.sql_statements.push(s.clone());
            }

            // 为下游外键提供模拟的主键池（1..=count）
            fk_pools.insert(tname.clone(), (1..=count).map(|i| i.to_string()).collect());
            report.tables_processed += 1;
        }

        Ok(report)
    }
}


// ── 辅助函数：构建字段配置、数据库配置 ───────────────────────────────────────────

/// 构建字段配置（当前为占位实现，未完成）
// fn build_field_config(
//     field: &core::schema::Field,
//     field_config: &Option<core::schema::FieldConfig>,
// ) -> core::schema::FieldConfig {
//     let mut config = field_config.clone().unwrap_or_default();
//     if config.is_empty() {
//         config.min = field.min.clone();
//     }
//     // TODO: 根据字段类型和约束完善配置
//     config
// }

/// 根据独立参数或 URL 构建统一的数据库配置 DatabaseConfig
fn build_db_config(
    db_type: &str, host: &str, port: Option<u16>,
    db_name: &str, user: &str, pass: &str,
    url: Option<&str>,
) -> Result<DatabaseConfig> {
    // 优先使用显式传入的 URL，否则读取环境变量 DATABASE_URL
    let database_url = url.map(|s| s.to_string())
        .or_else(|| std::env::var("DATABASE_URL").ok());

    // 解析数据库类型字符串为枚举
    let parsed_type: DbType = db_type.parse().map_err(|e: String| MockerError::Config {
        message: e,
    })?;

    // 根据数据库类型确定默认端口
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