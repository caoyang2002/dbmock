# dbmock

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

**dbmock** 是一个高性能、可扩展的数据库 Mock 数据生成工具。它能自动识别数据库表结构（schema），并基于外键依赖关系，自动生成符合数据类型和业务约束的随机测试数据。

## 必要说明

> 本项目是 TinyForum 项目的附属产品，旨在快速使用 mock 数据填充数据库，便于进行 TinyForum 的逻辑检验和性能测试，因而本项目会有 TinyForum 的数据，目前正在逐步清理，如有问题请提 Issues，感谢～

### 1. 已知问题

当数据库存在 unique 限制时，重复插入数据大概率会失败。因为目前的生成策略会撞已有数据。

### 2. 适配情况

目前仅对 psql 进行了适配与测试，其他数据库可能存在一些严重问题。

### 3. 限制 

为了确保安全，暂不支持清空数据库，请自行操作，例如清空 tiny_forum 数据库：

#### 方法一：清空表

```sql
DO $$
DECLARE
    r RECORD;
BEGIN
    FOR r IN (SELECT tablename FROM pg_tables WHERE schemaname = 'public') LOOP
        EXECUTE 'TRUNCATE TABLE ' || quote_ident(r.tablename) || ' CASCADE;';
    END LOOP;
END $$;
```

#### 方法二：删除并重建（数据结构丢失）

```sql
\c Postgres
DROP DATABASE IF EXISTS tiny_forum; CREATE DATABASE tiny_forum;
```

## 一、特性

- **自动识别表结构** – 从数据库读取表、列、数据类型、主键、外键等元信息，无需手动编写映射。
- **自动依赖排序** – 自动分析外键关系，按正确的顺序插入数据，避免引用完整性错误。
- **丰富的数据生成策略** – 根据列名和数据类型生成逼真的随机值（如邮箱、UUID、时间戳、用户名等）。
- **高性能批量插入** – 使用 `sqlx` 异步连接池 + `QueryBuilder` 批量插入，支持千行/秒吞吐量。
- **多数据库支持** – 现已支持 PostgreSQL、MySQL、SQLite，且易于扩展至其他关系型数据库。
- **Schema 文件导出/导入** – 将数据库结构导出为 JSON/SQL 文件，可版本化管理，并用于离线生成数据。
- **Preview 模式** – 预览生成的 SQL 语句，不实际执行，适合调试和审阅。

## 二、快速开始

### 1. 安装

手动构建

```bash
git clone https://github.com/caoyang2002/dbmock.git
cd dbmock
cargo build --release
# 二进制位于 target/release/dbmock
./target/release/dbmock --help
```

make 自动构建

```bash
make build
# 二进制会自动复制到根目录
./dbmock --help
```

或直接从源码运行：

```bash
cargo run -- --help
```

### 2. 基本使用

本项目使用 make 工具进行快速启动，你可以使用 `make help` 进行查看相应的命令，如果你只是想要快速填充数据库，并且数据库没有复杂的逻辑，可以尝试运行

```bash
make run # 自动填充 10 条数据（暂不可设置填充数量）
```

## 三、详细用法

### 1. 导出数据库结构

```bash
./dbmock extract --db-type postgres --db-host localhost --db-port 5432 --db-name your_db_name --db-user user_name --db-pass user_password -j schema.json -s schema.sql
# dbmock extract --db-type postgres --db-host localhost --db-port 5432 --db-name tiny_forum --db-user simons --db-pass password -j schema.json
```
> 注意：默认会创建 `schema.sql` 和 `schema.json` 文件，`-j` 和 `-s` 仅仅是命名，而非决定是否创建对应文件。

也可以使用环境变量配置数据库 URL：

```bash
export DB_URL="postgresql://username:password@localhost/mydb"
# export DB_URL="postgresql://tinyforum:tf-password@localhost:5678/tiny_forum"
./dbmock extract
```

如果生成的文件是空的，请检查用户是否有权限，`make test-db`（如果你是 psql），授权方式可以参考（以用户 tinyforum 为例）：

```sql
\c your_database_name
-- 授予 public schema 下所有表的权限
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO tinyforum;

-- 授予所有序列的权限
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO tinyforum;

-- 设置默认权限，确保将来新建的表也自动授权
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL PRIVILEGES ON TABLES TO tinyforum;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL PRIVILEGES ON SEQUENCES TO tinyforum;
```

> **重要提示**
>
> 由于数据库可能有多种字段策略（类型），例如字符串，数字、指定的枚举类型、不可重复、外键关联等，无论是 pg_dump 程序还是 extract 子命令都无法导出，因而使用 config 命令生成 `config.yml` 配置文件 ，随后根据 `config.yml` 文件中相关的注释进行配置。

可以使用以下命令快速创建对应的配置文件（该命令会强制覆盖 `config.yml`，如果有重要修改，请勿使用）

```bash
make init
```

### 2. 生成配置文件

该命令会读取上一个步骤生成的 json 文件，默认读取 `schema.json`，如果有重命名，请使用 `-j filename.json` 设置文件名

```bash
./dbmock config
```

如果配置文件已经存在，可以使用 `--force` 命令进行强制覆盖

```bash
./dbmock config --force
```

### 3. 自动生成 Mock 数据

该命令会全量创建数据库，默认 10 条数据，默认读取 `schema.json` 文件，如果文件名有修改，请使用 `-j` 命令配置对应的文件。

```bash
./dbmock generate
```

自定义创建数量

```bash
./dbmock generate -j schema.json --rows table_name_a=100 --rows table_name_b=500
```

预览模式，不实际执行

```bash
./dbmock generate -j schema.json --rows table_name_a=100 --rows table_name_b=500 --previwe
```

调试输出

```bash
./dbmock generate -j schema.json --rows table_name_a=100 --rows table_name_b=5000 --debug
```

## 三、命令详解

### `extract` – 提取数据库结构

| 参数               | 说明                                | 默认值        |
| ------------------ | ----------------------------------- | ------------- |
| `-j, --json`     | 输出 JSON 文件路径，包含完整的自定义结构化数据，易于本程序读取  | `schema.json` |
| `-s, --sql`| 输出 sql 文件路径，通过命令|`schema.sql`|
| `--db-type`        | 数据库类型：`postgres`, `mysql`, `sqlite` | `postgres`    |
| `--db-host`        | 数据库主机                          | `localhost`   |
| `--db-port`        | 端口                                | 5432/3306     |
| `--db-name`        | 数据库名                            | 无            |
| `--db-user`        | 用户名                              | 无            |
| `--db-pass`        | 密码                                | 空            |
| `-d, --db-url`| 完整连接字符串（优先级高于独立参数） | 无            |

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
| `-d, --db-url`| 连接字符串                             | 无            |

## 四、项目结构

```
dbmock/
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
- **数据生成**： < 30 秒（批量插入，千行/批）
- 内存占用： < 100 MB

## 七、技术栈

- [Rust](https://www.rust-lang.org/) – 高性能、内存安全
- [sqlx](https://github.com/launchbadge/sqlx) – 异步 SQL 工具包，原生支持多种数据库
- [clap](https://github.com/clap-rs/clap) – 命令行参数解析
- [serde](https://serde.rs/) – 序列化/反序列化
- [rand](https://github.com/rust-random/rand) – 随机数据生成
- [tokio](https://tokio.rs/) – 异步运行时
