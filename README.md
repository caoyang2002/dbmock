# dbmock

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

**dbmock** 是一个数据库 Mock 数据生成工具。它能自动识别数据库表结构（schema），并基于外键依赖关系，自动生成符合数据类型和业务约束的随机测试数据。

## 必要说明

> 本项目是 TinyForum 项目的附属产品，旨在快速使用 mock 数据填充数据库，便于进行 TinyForum 的逻辑检验和性能测试，因而本项目会有 TinyForum 的数据，目前正在逐步清理，如有问题请提 Issues，感谢～

### 1. 已知问题

当数据库存在 unique 限制时，重复插入数据大概率会失败。因为目前的生成策略会撞已有数据。

### 2. 适配情况

目前仅对 psql 进行了适配与测试，其他数据库可能存在一些严重问题。

### 3. 限制 

为了确保安全，暂不支持清空数据库，请自行操作，例如清空 tiny_forum 数据库：

#### 方法一：清空表（保留表结构）

```sql
\c tiny_forum
SELECT 'TRUNCATE TABLE ' || quote_ident(tablename) || ' CASCADE;' FROM pg_tables WHERE schemaname = 'public' \gexec
```

#### 方法二：删除并重建（表结构丢失）

```sql
\c postgres
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
# export DB_URL="postgresql://tinyforum:tf-password@localhost/tiny_forum"
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

该命令会全量创建数据库，默认会在每个表中创建 1000 条数据，默认读取 `schema.json` 文件，如果文件名有修改，请使用 `-j` 命令配置对应的文件。

```bash
./dbmock generate
```

在所有表中自定义创建数量

```bash
./dbmock generate --count 100000
```

在指定的表中，自定义创建数量

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

### `config` - 生成配置文件


## 四、配置文件

默认生成的配置文件为 `mock_config.yml`

该文件控制每个字段（列）如何生成模拟数据。你可以修改任意字段的 `type:` 来改变其生成策略。

### 1. 可用生成类型（`type:`）

#### 1.1 基础控制类

| 类型 | 说明 | 参数 |
|------|------|------|
| `default` | 自动根据数据库列类型生成（最安全，推荐默认使用） | 无 |
| `skip`   | 完全跳过该列，不会出现在 INSERT 语句中 | 无 |
| `null`   | 始终输出 SQL 的 `NULL` | 无 |
| `constant` | 每次都输出相同的固定值 | `constant: "固定值"` |

#### 1.2 标识与唯一性
| 类型 | 说明 | 参数 |
|------|------|------|
| `uuid` | 生成 UUID v4 字符串 | 无 |
| `unique` | 在本轮生成中保证唯一。若列类型为整数，直接输出数字；否则输出「字母前缀+数字」（如 `abc123`） | 无（自动计数） |
| `sequence` | 从 1 开始单调递增的整数（每行 +1） | 无 |

#### 1.3 数值类型
| 类型 | 说明 | 可选参数 |
|------|------|----------|
| `int` | 随机整数 | `min`, `max`（默认 0 ~ 2,147,483,647） |
| `float` | 随机浮点数，保留 4 位小数 | `min`, `max`（默认 0.0 ~ 9999.99） |
| `decimal` | 固定精度的十进制数 | `min`, `max`, `scale`（小数位数，默认 2） |
| `bool` | 随机 true/false，会根据数据库类型自动输出 `true`/`false` 或 `1`/`0` | 无 |

#### 1.4 文本类型
| 类型 | 说明 | 可选参数 |
|------|------|----------|
| `string` | 随机字母数字串（小写+数字） | `min_len`, `max_len`（默认 4~32） |
| `text` | 随机英文短句（若干单词拼接） | `max_len`（默认 500，超长截断） |
| `label` | 1~2 个首字母大写的单词（适合名称/标签） | 无（单词长度随机 3~8） |
| `slug` | 小写英文单词用短横线连接（如 `my-new-post`） | `max_len`（默认 32，超长截断） |
| `enum` | 从给定的列表中随机选取一项 | `values: [a, b, c]` |

#### 1.5 网络与标识
| 类型 | 说明 | 可选参数 |
|------|------|----------|
| `email` | 形如 `user.123@domain.com` 的邮箱 | 无 |
| `url`   | 形如 `https://www.host.com/path` 的网址 | 无 |
| `ip`    | IPv4 地址（1～254 段合理） | 无 |
| `semver`| 语义化版本号 `major.minor.patch`（0~5 / 0~20 / 0~99） | 无 |
| `password` | 模拟 bcrypt 格式的哈希（`$2b$10$...`），仅供格式参考 | 无 |
| `phone` | 国际格式手机号 `+CCsubscriber`（国家码 1~99，号码 100_000_000 ~ 9_999_999_999） | 无 |
| `color` | CSS 十六进制颜色 `#RRGGBB` | 无 |
| `user_agent` | 简单的浏览器 User-Agent 字符串 | 无 |

#### 1.6 日期时间
| 类型 | 说明 | 可选参数 |
|------|------|----------|
| `timestamp_tz` | 带时区的日期时间，输出格式 `%Y-%m-%d %H:%M:%S+00` | `date_from`, `date_to`（默认近 5 年） |
| `timestamp` | 不带时区的日期时间，格式 `%Y-%m-%d %H:%M:%S` | 同上 |
| `date`       | 日期 `%Y-%m-%d` | 同上 |
| `time`       | 时间 `HH:MM:SS`（00:00:00 ~ 23:59:59） | 无 |

#### 1.7 结构化数据
| 类型 | 说明 | 可选参数 |
|------|------|----------|
| `json` | 生成 JSON 对象。若不提供 `json_schema`，则自动生成 1~4 个随机键值对（数字/字符串/布尔/浮点混合）；若提供 `json_schema`，则按 schema 递归生成精确结构 | `json_schema`（见下文详细说明） |

---

### 2. 全局可选参数（适用于所有类型）

| 参数名 | 类型 | 说明 | 默认值 |
|--------|------|------|--------|
| `null_rate` | 浮点数 (0.0~1.0) | 覆盖该列的 NULL 生成概率。对于可空列默认为 0.15（15%），非空列默认为 0 | 取决于是否可空 |

---

### 3. JSON Schema 配置详解（`json_schema`）

当 `type: json` 且需要精确控制 JSON 结构时，可以配置 `json_schema` 字段。它是一个列表，每个元素描述 JSON 对象中的一个字段。

#### 3.1 字段属性

| 属性 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | 字符串 | 是 | JSON 中的键名 |
| `kind` | 字符串 | 是 | 该字段的生成类型，支持所有基础类型以及 `json_object`、`json_array` |
| `required` | 布尔 | 否 | 是否必须出现。若为 `false`，约有 20% 概率会省略该字段（模拟可选字段） |
| `properties` | 列表 | 否 | 当 `kind` 为 `json_object` 时，用于定义内部子字段 |
| `array_items` | 对象 | 否 | 当 `kind` 为 `json_array` 时，定义数组中每个元素的类型（使用相同结构） |

#### 3.2 示例

```yaml
json_schema:
  - name: "user_id"
    kind: "int"
    required: true
  - name: "profile"
    kind: "json_object"
    required: true
    properties:
      - name: "nickname"
        kind: "string"
      - name: "age"
        kind: "int"
        required: false
  - name: "tags"
    kind: "json_array"
    required: false
    array_items:
      kind: "string"
```

该配置会生成类似如下的 JSON：

```json
{
  "user_id": 423,
  "profile": {
    "nickname": "abc123",
    "age": 28
  },
  "tags": ["rust", "database"]
}
```

> 注意：`json_array` 的 `array_items` 所指向的类型可以是任意基础类型，也可以是 `json_object`（实现对象数组）。目前数组长度随机为 1~5 个元素。

---

### 4. 完整配置示例

```yaml
columns:
  - name: "id"
    type: "sequence"           # 自增主键
  - name: "username"
    type: "string"
    min_len: 5
    max_len: 20
  - name: "score"
    type: "decimal"
    min: 0.0
    max: 100.0
    scale: 2
  - name: "is_active"
    type: "bool"
  - name: "created_at"
    type: "timestamp"
    date_from: "2023-01-01"
    date_to: "2024-12-31"
  - name: "metadata"
    type: "json"
    null_rate: 0.1
    json_schema:
      - name: "version"
        kind: "semver"
      - name: "extra"
        kind: "json_object"
        properties:
          - name: "flag"
            kind: "bool"
```

---

#### 4.1 注意事项

1. **`unique` 与 `sequence` 的区别**  
   - `sequence` 只输出纯数字（从 1 开始递增），适合整数主键。  
   - `unique` 在非整数列上会添加字母前缀，保证字符串列的唯一性。

2. **NULL 生成优先级**  
   - 列级 `null_rate` > 列的可空属性默认值（15% 或 0）。  
   - 如果 `null_rate = 0.0`，永远不会生成 NULL（除非显式使用 `type: null`）。

3. **JSON 内部字段不支持自定义 `null_rate`**  
   目前 JSON 内部的字段 NULL 概率由 `required: false` 控制（约 20% 跳过整个字段），暂不支持细粒度的 NULL 百分比。

4. **日期范围**  
   当 `date_from` 晚于 `date_to` 时，会自动交换二者。解析失败时会回退到默认的近 5 年窗口。

5. **SQL 注入防护**  
   所有字符串都会被正确转义（单引号 `'` 变为 `''`），JSON 字符串通过 `serde_json` 序列化也会自动转义双引号和反斜杠。


---

### ⚙️ 可选参数（根据所选类型使用）

| 参数名 | 用途 | 适用类型 |
|--------|------|----------|
| `values` | 枚举列表 | `enum` |
| `min` | 最小值 | `int`, `float`, `decimal` |
| `max` | 最大值 | `int`, `float`, `decimal` |
| `scale` | 小数位数 | `decimal` |
| `min_len` | 最小长度 | `string` |
| `max_len` | 最大长度 | `string`, `text` |
| `date_from` | 起始日期 | `timestamp_tz`, `timestamp`, `date` |
| `date_to` | 结束日期 | `timestamp_tz`, `timestamp`, `date` |
| `null_rate` | NULL 概率（0.0–1.0） | 任意类型（覆盖默认） |
| `constant` | 固定值 | `constant` |

---

### 📝 示例

```yaml
# 某一列的配置
columns:
  - name: user_id
    type: int
    min: 1
    max: 100000
  - name: status
    type: enum
    values: ["active", "inactive", "banned"]
  - name: created_at
    type: timestamp_tz
    date_from: "2020-01-01"
    date_to: "2025-12-31"
  - name: description
    type: text
    max_len: 500
```

## 五、项目结构

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

### 1. 顶层入口

- **`main.rs`**  
  程序主入口，负责初始化、解析命令行参数、加载配置、协调各模块运行。

---

### 模块详解

#### `cli/` – 命令行接口
- **`parser.rs`** – 解析用户传入的命令行参数（如数据库连接、生成策略、输出格式等）。
- **`mod.rs`** – 模块入口，暴露公共接口。

#### `config/` – 配置管理
- **`settings.rs`** – 通用配置项（数据库连接、生成记录数、并发数等）。
- **`tuning.rs`** – 性能调优参数（批处理大小、缓存设置、超时等）。
- **`mod.rs`** – 整合配置，提供统一的加载与访问接口。

#### `core/` – 核心逻辑
- **`driver.rs`** – 数据库驱动层的抽象接口，定义统一的操作（连接、插入、查询等）。
- **`generator.rs`** – 生成器的主控逻辑，协调字段生成、依赖关系、批处理等。
- **`schema.rs`** – 内部表示数据表结构、字段类型、约束等。
- **`mod.rs`** – 核心模块入口。

#### `datapool/` – 数据池（数据源）
- **`user.rs`** – 用户相关的数据生成规则（如用户名、邮箱、地址等）。
- **`unique.rs`** – 保证生成值唯一性的机制（如自增ID、唯一令牌）。
- **`mock_generators.rs`** – 各类模拟数据生成器（姓名、日期、数字、文本等）。
- **`mod.rs`** – 数据池统一接口。

#### `db/` – 数据库抽象层
- **`connection.rs`** – 连接管理（建立、关闭、事务）。
- **`types.rs`** – 数据库类型与 Rust 类型的映射。
- **`mod.rs`** – 模块入口。

#### `driver/` – 具体数据库驱动实现
- **`mysql.rs`** – MySQL 适配器，实现 `core::driver` 中定义的接口。
- **`postgres.rs`** – PostgreSQL 适配器。
- **`sqlite.rs`** – SQLite 适配器。
- **`mod.rs`** – 动态选择并导出对应的驱动。

#### `errors/` – 错误处理
- **`error.rs`** – 自定义错误类型（支持上下文、链式错误）。
- **`mod.rs`** – 模块入口，通常提供 `Result` 类型别名。

#### `fieldconfig/` – 字段级配置
- **`types.rs`** – 定义字段的配置数据结构（如类型、范围、默认值、依赖关系）。
- **`generate.rs`** – 根据字段配置生成具体值。
- **`infer.rs`** – 从数据库表结构或用户输入推断字段配置。
- **`serialize.rs`** – 配置的序列化/反序列化（如保存为 YAML/JSON）。
- **`mod.rs`** – 模块入口。

#### `generator/` – 生成引擎
- **`engine.rs`** – 生成器的核心引擎，管理生成过程、并发、进度等。
- **`batch.rs`** – 批量生成逻辑（分页、缓冲区）。
- **`dependency.rs`** – 处理字段间依赖关系（例如 `total = price * quantity`）。
- **`value.rs`** – 底层值生成函数（随机、顺序、自定义规则）。
- **`mod.rs`** – 模块入口。

#### `logger/` – 日志与输出
- **`applog.rs`** – 应用运行日志（信息、警告、错误）。
- **`table.rs`** – 表格化输出生成的数据（供预览或导出）。
- **`mod.rs`** – 模块入口。

#### `schema/` – 模式处理
- **`loader.rs`** – 加载表结构定义（从文件、数据库或交互式输入）。
- **`extractor/`** – 从已有数据库提取模式（反向工程）。
  - **`mysql.rs`** – MySQL 模式提取器。
  - **`postgres.rs`** – PostgreSQL 模式提取器。
  - **`sqlite.rs`** – SQLite 模式提取器。
  - **`mod.rs`** – 统一提取接口。
- **`mod.rs`** – 模块入口。

---

### 2. 模块协作关系（工作流）

```text
[CLI] → [Config] → [Logger] 
   ↓
[Core] → [Schema] ←→ [FieldConfig]
   ↓         ↓
[Driver] ← [DB] ← [Datapool]
   ↓
[Generator] → [FieldConfig] → [Datapool]
   ↓
[Logger/Table] → 输出结果
```

1. **解析输入**：`cli` 读取命令，`config` 加载配置文件。
2. **加载/提取模式**：`schema::loader` 或 `extractor` 获得目标表结构。
3. **准备字段配置**：`fieldconfig::infer` 推断每列的生成规则，可选由用户覆盖。
4. **初始化数据库驱动**：`driver` 根据配置连接 MySQL/Postgres/SQLite。
5. **生成数据**：`generator::engine` 调用 `batch`、`dependency`、`value` 产生数据行。
   - 使用 `datapool` 提供的模拟数据源（如用户列表、唯一值池）。
6. **写入数据库**：通过 `core::driver` 抽象层将数据批量插入。
7. **日志与输出**：`logger` 记录过程，`table` 可预览生成的数据。


遵循 SOLID 原则，高内聚低耦合，易于扩展新的数据库或自定义数据生成策略。

## 六、扩展开发

### 1. 添加新的数据库驱动

1. 在 `src/driver/` 下新建 `yourdb.rs`。
2. 实现 `core::driver::DatabaseDriver` trait。
3. 在 `src/driver/mod.rs` 的 `new()` 工厂中注册。
4. 如有必要，在 `src/schema/extractor/` 中添加对应的 `extract_schema` 实现。

### 2. 自定义数据生成策略

修改 `src/generator/value.rs`，可根据列名匹配特定规则（如 `email` → 生成邮箱格式）。

## 七、性能参考

可以使用工具测试

```bash
make perf # 默认 usrs 表
```

在 PostgreSQL 18.3 上生成 100 万条记录：
平台：
```bash
Model Name: Mac mini
Model Identifier: Mac16,10
Model Number: MU9D3CH/A
Chip: Apple M4
Total Number of Cores: 10 (4 Performance and 6 Efficiency)
Memory: 16 GB
System Firmware Version: 13822.81.10
OS Loader Version: 13822.81.10
```

```log
📊 性能报告（表格）
+--------------------------------+----------------------------------+
| 指标                           | 值                               |
+--------------------------------+----------------------------------+
| ✅ 总耗时                      | 31.79 秒                        |
| 📊 目标行数                    | 1000000 行/表 × 1 表 = 1000000 行 |
| 📊 实际插入行数                | 1000000                          |
| 📊 错误数                      | none                             |
| 📊 平均吞吐率                  | 31456 行/秒                    |
+--------------------------------+----------------------------------+
| 资源占用                       |                                  |
+--------------------------------+----------------------------------+
| User time (用户态)              | 8.96 秒                         |
| System time (内核态)            | 0.96 秒                         |
| CPU 占用率                      | 31%                              |
| Maximum resident set size       | 1329392                          |
| Swaps                           | 0                                |
| Socket messages sent            | 3176                             |
| Socket messages received        | 2541                             |
| Major page faults               | 3                                |
| Minor page faults               | 264313                           |
| Voluntary context switches      | 3515                             |
| Involuntary context switches    | 12211                            |
+--------------------------------+----------------------------------+
```

## 七、技术栈

| 类别 | 技术/库 | 版本 | 用途说明 |
|------|---------|------|----------|
| **编程语言** | Rust | Edition 2021 | 核心语言 |
| **异步运行时** | tokio | 1.x (full) | 异步 I/O、任务调度、多线程执行 |
| **数据库驱动** | sqlx | 0.7 | 异步数据库连接池与查询，支持 PostgreSQL、MySQL、SQLite，以及 `json`、`chrono`、`uuid` 特性 |
| **命令行解析** | clap | 4.x (derive) | 定义和解析命令行参数，支持子命令和自动生成帮助信息 |
| **序列化** | serde / serde_json / serde_yaml | 1.x / 0.9 | 数据序列化（JSON/YAML），用于配置文件、输出结果等 |
| **随机生成** | rand | 0.8 | 基础随机数生成，用于 mock 数据 |
| **日期时间** | chrono | 0.4 | 处理日期、时间、时区，支持 serde 序列化 |
| **UUID** | uuid | 1.x | 生成和解析 UUID（v4 版本），支持 serde |
| **错误处理** | thiserror / anyhow | 1.x | thiserror 定义自定义错误类型，anyhow 简化错误传播 |
| **异步 trait** | async-trait | 0.1 | 支持在 trait 中定义异步方法 |
| **模拟数据生成** | fake | 2.9 | 丰富多样的假数据生成器（姓名、地址、互联网等），与 chrono、uuid 集成 |
| **进度条** | indicatif | 0.17 | 命令行进度条和状态指示 |
| **全局初始化** | once_cell | 1.21 | 懒静态变量，用于全局配置或资源 |
| **系统信息** | num_cpus / sys-info | 1.16 / 0.9 | 获取 CPU 核心数、内存等系统资源，用于调优 |
| **编译优化** | 编译器配置 | release profile | `opt-level=3`, `lto=true`, `codegen-units=1` 进行全链路优化，提升运行时性能 |
