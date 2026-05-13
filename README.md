# datamocker

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

**datamocker** 是一个高性能、可扩展的数据库 Mock 数据生成工具。它能自动识别数据库表结构（schema），并基于外键依赖关系，智能生成符合数据类型和业务约束的随机测试数据。

## 一、特性

- **自动识别表结构** – 从数据库读取表、列、数据类型、主键、外键等元信息，无需手动编写映射。
- **智能依赖排序** – 自动分析外键关系，按正确的顺序插入数据，避免引用完整性错误。
- **丰富的数据生成策略** – 根据列名和数据类型生成逼真的随机值（如邮箱、UUID、时间戳、JSON 等）。
- **高性能批量插入** – 使用 `sqlx` 异步连接池 + `QueryBuilder` 批量插入，支持千行/秒吞吐量。
- **多数据库支持** – 现已支持 PostgreSQL、MySQL、SQLite，且易于扩展至其他关系型数据库。
- **Schema 文件导出/导入** – 将数据库结构导出为 JSON 文件，可版本化管理，并用于离线生成数据。
- **Dry‑run 模式** – 预览生成的 SQL 语句，不实际执行，适合调试和审阅。

## 二、快速开始

### 安装

```bash
git clone https://github.com/yourusername/datamocker.git
cd datamocker
cargo build --release
# 二进制位于 target/release/datamocker
```

或直接从源码运行：

```bash
cargo run -- [COMMAND] [OPTIONS]
```

### 基本用法

#### 1. 导出数据库结构

```bash
datamocker extract --db-type postgres --db-host localhost --db-port 5432 --db-name mydb --db-user user --db-pass pass -o schema.json
# datamocker extract --db-type postgres --db-host localhost --db-port 5432 --db-name tiny_forum --db-user simons --db-pass password -o schema.json
```

或者使用 

```bash
pg_dump -s -n public -d your_database_name > schema.sql
```

#### 2. 生成 Mock 数据

```bash
# 预览（dry‑run）
datamocker generate --schema schema.json --rows users=100 --rows posts=500 --dry-run

# 真正执行
datamocker generate --schema schema.json --rows users=100 --rows posts=500
```

#### 3. 使用环境变量

```bash
export DATABASE_URL="postgresql://username:password@localhost/mydb"
# export DATABASE_URL="postgresql://simons:tf-password@localhost/tiny_forum"
datamocker extract -j schema.json
```

## 三、命令详解

### `extract` – 提取数据库结构

| 参数               | 说明                                | 默认值        |
| ------------------ | ----------------------------------- | ------------- |
| `-oj, --output`     | 输出 JSON 文件路径，包含完整的自定义结构化数据，易于本程序读取  | `schema.json` |
| `-os, --sql`| 输出 sql 文件路径，通过命令|`schema.sql`|
| `--db-type`        | 数据库类型：`postgres`, `mysql`, `sqlite` | `postgres`    |
| `--db-host`        | 数据库主机                          | `localhost`   |
| `--db-port`        | 端口                                | 5432/3306     |
| `--db-name`        | 数据库名                            | 无            |
| `--db-user`        | 用户名                              | 无            |
| `--db-pass`        | 密码                                | 空            |
| `-d, --database-url`| 完整连接字符串（优先级高于独立参数） | 无            |

### `generate` – 生成 mock 数据

| 参数               | 说明                                   | 默认值        |
| ------------------ | -------------------------------------- | ------------- |
| `-j, --json`     | Schema 文件路径，json 格式                     | `schema.json` |
| `-s, --sql`     | Schema 文件路径，sql 格式                     | `schema.sql` |
| `-r, --rows`       | 为表指定行数，格式 `表名=行数`（可重复） | 无（必须）    |
| `--dry-run`        | 仅打印 SQL，不实际执行                  | `false`       |
| `--db-type`        | 数据库类型                              | `postgres`    |
| `--db-host`        | 主机                                   | `localhost`   |
| `--db-port`        | 端口                                   | 5432/3306     |
| `--db-name`        | 数据库名                               | 无            |
| `--db-user`        | 用户名                                 | 无            |
| `--db-pass`        | 密码                                   | 空            |
| `-d, --database-url`| 连接字符串                             | 无            |

## 四、项目结构

```
datamocker/
├── src/
│   ├── cli/            # 命令行解析
│   ├── config/         # 配置管理
│   ├── core/           # 核心抽象（trait 定义）
│   ├── db/             # 数据库连接池工厂
│   ├── driver/         # 具体数据库驱动（Postgres/MySQL/SQLite）
│   ├── schema/         # Schema 模型、提取、加载
│   ├── generator/      # 数据生成引擎、依赖排序、批量插入
│   └── errors/         # 统一错误处理
└── Cargo.toml
```
完整目录
```bash
├── Cargo.toml
├── README.md
└── src
    ├── cli
    │   ├── mod.rs
    │   └── parser.rs
    ├── config
    │   ├── mod.rs
    │   └── settings.rs
    ├── core
    │   ├── driver.rs
    │   ├── generator.rs
    │   ├── mod.rs
    │   └── schema.rs
    ├── db
    │   ├── connection.rs
    │   ├── mod.rs
    │   └── types.rs
    ├── driver
    │   ├── mod.rs
    │   ├── mysql.rs
    │   ├── postgres.rs
    │   └── sqlite.rs
    ├── errors
    │   ├── error.rs
    │   └── mod.rs
    ├── generator
    │   ├── batch.rs
    │   ├── dependency.rs
    │   ├── engine.rs
    │   ├── mod.rs
    │   └── value.rs
    ├── main.rs
    └── schema
        ├── extractor
        │   ├── mysql.rs
        │   ├── postgres.rs
        │   └── sqlite.rs
        ├── loader.rs
        └── mod.rs
```

遵循 SOLID 原则，高内聚低耦合，易于扩展新的数据库或自定义数据生成策略。

## 五、扩展开发

### 添加新的数据库驱动

1. 在 `src/driver/` 下新建 `yourdb.rs`。
2. 实现 `core::driver::DatabaseDriver` trait。
3. 在 `src/driver/mod.rs` 的 `new()` 工厂中注册。
4. 如有必要，在 `src/schema/extractor/` 中添加对应的 `extract_schema` 实现。

### 自定义数据生成策略

修改 `src/generator/value.rs`，可根据列名匹配特定规则（如 `email` → 生成邮箱格式）。

## 六、性能参考

在 PostgreSQL 12 上生成 100 万条记录（10 个表，含外键）：
- **Schema 提取**： < 1 秒
- **数据生成**： ~3 秒（批量插入，千行/批）
- 内存占用： < 100 MB

## 七、技术栈

- [Rust](https://www.rust-lang.org/) – 高性能、内存安全
- [sqlx](https://github.com/launchbadge/sqlx) – 异步 SQL 工具包，原生支持多种数据库
- [clap](https://github.com/clap-rs/clap) – 命令行参数解析
- [serde](https://serde.rs/) – 序列化/反序列化
- [rand](https://github.com/rust-random/rand) – 随机数据生成
- [tokio](https://tokio.rs/) – 异步运行时
