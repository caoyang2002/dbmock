use clap::{Args, Parser, Subcommand};

/// 命令行入口参数解析器
#[derive(Parser, Debug)]
#[command(
    name = "datamocker",
    version = "0.1.0",
    author = "datamocker contributors",
    about = "高性能数据库模拟数据生成器",
    long_about = "datamocker 能够自动读取数据库表结构，并生成符合外键约束的、逼真的模拟数据。"
)]
pub struct Cli {
    /// 子命令（extract / generate / config）
    #[command(subcommand)]
    pub command: Commands,
}

/// 所有支持的子命令
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 提取数据库结构并保存为文件（JSON / SQL）
    Extract(ExtractArgs),

    /// 根据结构定义文件生成模拟数据
    Generate(GenerateArgs),

    /// 生成或更新字段级别的配置 YAML 文件
    Config(ConfigArgs),
}

/// `extract` 子命令的参数
#[derive(Args, Debug)]
pub struct ExtractArgs {
    /// 输出的 JSON 文件路径（自定义结构化格式）
    #[arg(short = 'j', long = "json", default_value = "schema.json")]
    pub output_json: String,

    /// 输出的 SQL 文件路径（CREATE TABLE 语句）
    #[arg(short = 's', long = "sql", default_value = "schema.sql")]
    pub output_sql: String,

    /// 数据库类型：postgres, mysql, sqlite
    #[arg(long, default_value = "postgres")]
    pub db_type: String,

    /// 数据库主机地址
    #[arg(long, default_value = "localhost")]
    pub db_host: String,

    /// 数据库端口（默认：PostgreSQL=5432, MySQL=3306, SQLite 忽略）
    #[arg(long)]
    pub db_port: Option<u16>,

    /// 数据库名称
    #[arg(long, default_value = "")]
    pub db_name: String,

    /// 数据库用户名
    #[arg(long, default_value = "")]
    pub db_user: String,

    /// 数据库密码
    #[arg(long, default_value = "")]
    pub db_pass: String,

    /// 完整数据库连接 URL（若提供则覆盖以上独立参数）
    #[arg(short = 'd', long)]
    pub database_url: Option<String>,
}

/// `generate` 子命令的参数
#[derive(Args, Debug)]
pub struct GenerateArgs {
    /// 输入的 JSON 结构文件路径（与 --sql 二选一）
    #[arg(short = 'j', long = "json")]
    pub schema_json: Option<String>,

    /// 输入的 SQL 结构文件路径（与 --json 二选一）
    #[arg(short = 's', long = "sql")]
    pub schema_sql: Option<String>,

    /// 每张表需要生成的行数，格式：表名=行数（可重复使用，例如 --rows users=100 --rows posts=500）
    #[arg(short = 'r', long = "rows", value_name = "TABLE=COUNT", required = true)]
    pub rows: Vec<String>,

    /// 是否为试运行模式：仅打印生成的 SQL，不实际插入数据库
    #[arg(long, default_value_t = false)]
    pub preview: bool,

    /// 字段级配置的 YAML 文件路径（生成或读取）
    #[arg(long, default_value = "config.yaml")]
    pub config: String,

    /// 目标数据库类型（用于生成合适的 SQL 方言）
    #[arg(long, default_value = "postgres")]
    pub db_type: String,

    /// 数据库主机地址
    #[arg(long, default_value = "localhost")]
    pub db_host: String,

    /// 数据库端口
    #[arg(long)]
    pub db_port: Option<u16>,

    /// 数据库名称
    #[arg(long, default_value = "")]
    pub db_name: String,

    /// 数据库用户名
    #[arg(long, default_value = "")]
    pub db_user: String,

    /// 数据库密码
    #[arg(long, default_value = "")]
    pub db_pass: String,

    /// 完整数据库连接 URL（覆盖以上独立参数）
    #[arg(short = 'd', long)]
    pub database_url: Option<String>,
}

/// `config` 子命令的参数（用于生成或更新字段配置）
#[derive(Args, Debug)]
pub struct ConfigArgs {
    /// 输入的 schema JSON 文件路径（从 extract 获得）
    #[arg(short = 's', long = "schema", default_value = "schema.json")]
    pub schema: String,
    /// 输出的 YAML 配置路径
    #[arg(short = 'o', long = "output", default_value = "mock_config.yml")]
    pub output: String,
}