//! generate.rs — 根据 FieldConfig 生成值。
//!
//! `generate_with_config` 是 batch.rs 在存在 mock_config.yml 时使用的主入口。
//! 它覆盖了所有 FieldKind 变体，并为 `Default` 回退到基于 schema 的生成器。

use chrono::{Duration, NaiveDate, Utc};
use rand::Rng;
use serde_json::{Map, Number, Value};
use std::collections::HashMap;
use uuid::Uuid;

use crate::core::schema::ColumnSchema;
use crate::datapool::{random_avatar_url, random_password_hash, unique_email, unique_username};
use crate::fieldconfig::types::{FieldConfig, FieldKind, JsonFieldDef};

// ── 每列独立计数器 ────────────────────────────────────────────────────────────────
// Key: "表名.列名" → 计数器
static UNIQUE_COUNTERS: std::sync::OnceLock<std::sync::Mutex<HashMap<String, u64>>> =
    std::sync::OnceLock::new();

/// 获取指定 key 的下一个唯一值（从 1 开始递增）。
/// 用于 `Unique` 和 `Sequence` 类型。
fn next_unique(key: &str) -> u64 {
    let mut map = UNIQUE_COUNTERS
        .get_or_init(|| std::sync::Mutex::new(HashMap::new()))
        .lock()
        .unwrap();
    let counter = map.entry(key.to_string()).or_insert(0);
    *counter += 1;
    *counter
}

/// 重置所有唯一计数器（在每次生成运行开始时调用）。
pub fn reset_unique_counters() {
    if let Some(lock) = UNIQUE_COUNTERS.get() {
        lock.lock().unwrap().clear();
    }
}

// ── 主入口 ──────────────────────────────────────────────────────────────────────────

/// 根据 `fc` 为 `col` 生成一个 SQL 字面量。
///
/// - `unique_key`: "表名.列名"，用于追踪唯一计数器。
/// - `db_type`: 影响布尔值的引号风格（目前暂未使用，但保留以备后续扩展）。
///
/// 返回 `None` 表示调用方应回退到 schema 驱动的生成器（`FieldKind::Default`）。
/// 返回 `Some("__SKIP__")` 是一个哨兵值，表示该列应被跳过（不输出）。
pub fn generate_with_config(
    col: &ColumnSchema,
    fc: &FieldConfig,
    unique_key: &str,
    db_type: &str,
) -> Option<String> {
    // 处理 null_rate 覆盖：若列为可空且未显式指定，默认 null 率为 15%。
    let null_rate = fc
        .null_rate
        .unwrap_or(if col.is_nullable { 0.15 } else { 0.0 });
    if null_rate > 0.0 && rand::thread_rng().gen_bool(null_rate.clamp(0.0, 1.0)) {
        return Some("NULL".to_string());
    }

    let val = match &fc.kind {
        // ── 元类型 ─────────────────────────────────────────────────────────────
        FieldKind::Default => return None, // 调用方回退到 schema 驱动
        FieldKind::Skip => return Some("__SKIP__".to_string()), // 哨兵：跳过该列
        FieldKind::Null => "NULL".to_string(),
        FieldKind::Constant => sql_str(fc.constant.clone().unwrap_or_default()),

        // ── 标识 / 唯一性 ─────────────────────────────────────────────────────────
        FieldKind::Uuid => sql_str(Uuid::new_v4().to_string()),
        FieldKind::Unique => {
            let n = next_unique(unique_key);
            let dt = col.data_type.to_lowercase();
            // 整数列直接输出数字，否则输出 "字母+数字" 形式以保持唯一性且可读。
            if is_int_type(&dt) {
                n.to_string()
            } else {
                let base = gen_alphanum(rand::thread_rng().gen_range(4..=8));
                sql_str(format!("{}{}", base, n))
            }
        }
        FieldKind::Sequence => {
            let n = next_unique(unique_key);
            n.to_string()
        }

        // ── 数值类型 ──────────────────────────────────────────────────────────
        FieldKind::Int => {
            let lo = fc.min.unwrap_or(0.0) as i64;
            let hi = fc.max.unwrap_or(2_147_483_647.0) as i64;
            let hi = hi.max(lo + 1);
            rand::thread_rng().gen_range(lo..=hi).to_string()
        }
        FieldKind::Float => {
            let lo = fc.min.unwrap_or(0.0);
            let hi = fc.max.unwrap_or(9_999.99);
            let hi = if hi <= lo { lo + 1.0 } else { hi };
            format!("{:.4}", rand::thread_rng().gen_range(lo..hi))
        }
        FieldKind::Decimal => {
            let scale = fc.scale.unwrap_or(2);
            let lo = fc.min.unwrap_or(0.0);
            let hi = fc.max.unwrap_or(999_999.0);
            let hi = if hi <= lo { lo + 1.0 } else { hi };
            let v = rand::thread_rng().gen_range(lo..hi);
            format!("{:.prec$}", v, prec = scale)
        }

        // ── 布尔类型 ──────────────────────────────────────────────────────────
        FieldKind::Bool => {
            let b = rand::thread_rng().gen_bool(0.5);
            let dt = col.data_type.to_lowercase();
            // 根据数据库类型决定输出 true/false 还是 1/0
            if dt.contains("bool") {
                if b {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            } else {
                if b {
                    "1".to_string()
                } else {
                    "0".to_string()
                }
            }
        }

        // ── 文本类型 ─────────────────────────────────────────────────────────
        FieldKind::String => {
            let lo = fc.min_len.unwrap_or(4);
            let hi = fc.max_len.unwrap_or(32).max(lo + 1);
            let len = rand::thread_rng().gen_range(lo..=hi);
            sql_str(gen_alphanum(len))
        }
        FieldKind::Text => {
            let max = fc.max_len.unwrap_or(500);
            let word_count = rand::thread_rng().gen_range(8..=25_usize);
            let words: Vec<String> = (0..word_count)
                .map(|_| gen_word(rand::thread_rng().gen_range(3..=9)))
                .collect();
            let s = words.join(" ");
            let s = if s.len() > max {
                s[..max].to_string()
            } else {
                s
            };
            sql_str(s)
        }
        FieldKind::Label => {
            // 生成 1~2 个首字母大写的单词，适用于标签、名称等
            let words: Vec<String> = (0..rand::thread_rng().gen_range(1..=2_usize))
                .map(|_| gen_capitalized_word(rand::thread_rng().gen_range(3..=8)))
                .collect();
            sql_str(words.join(" "))
        }
        FieldKind::Slug => {
            let max = fc.max_len.unwrap_or(32);
            sql_str(gen_slug(max))
        }
        FieldKind::Enum => {
            let values = fc.values.as_deref().unwrap_or(&[]);
            if values.is_empty() {
                "NULL".to_string()
            } else {
                let i = rand::thread_rng().gen_range(0..values.len());
                sql_str(values[i].clone())
            }
        }

        // ── 网络相关 ─────────────────────────────────────────────────────────
        //
        // ★ 修复：Email 和 Username 都通过全局 HashSet 保证唯一性，
        //   与 users 表的 UNIQUE 索引约束保持一致。
        //   之前 Email 分支将 unique_email() 注释掉，改用内联 gen_alphanum 拼接，
        //   导致不经过 USED_EMAILS HashSet，无法保证唯一。
        FieldKind::Email => sql_str(unique_email()),
        FieldKind::Url => {
            let mut rng = rand::thread_rng();
            let host = gen_alphanum(rng.gen_range(5..=12));
            let path = gen_alphanum(rng.gen_range(3..=8));
            sql_str(format!("https://www.{}.{}/{}", host, pick_tld(), path))
        }
        FieldKind::UrlAvatar => sql_str(random_avatar_url()),
        FieldKind::Ip => {
            let mut rng = rand::thread_rng();
            sql_str(format!(
                "{}.{}.{}.{}",
                rng.gen_range(1_u8..=254),
                rng.gen_range(0_u8..=255),
                rng.gen_range(0_u8..=255),
                rng.gen_range(1_u8..=254),
            ))
        }
        FieldKind::Semver => {
            let mut rng = rand::thread_rng();
            sql_str(format!(
                "{}.{}.{}",
                rng.gen_range(0..=5),
                rng.gen_range(0..=20),
                rng.gen_range(0..=99)
            ))
        }

        // ── 个人信息 ─────────────────────────────────────────────────────────
        FieldKind::Password => sql_str(random_password_hash()),
        FieldKind::Phone => {
            let mut rng = rand::thread_rng();
            let cc: u16 = rng.gen_range(1..=99);
            let sub: u64 = rng.gen_range(100_000_000..=9_999_999_999);
            sql_str(format!("+{}{}", cc, sub))
        }
        FieldKind::Color => sql_str(format!(
            "#{:06X}",
            rand::thread_rng().gen_range(0_u32..=0xFF_FF_FF)
        )),
        FieldKind::UserAgent => sql_str(format!(
            "Mozilla/5.0 (compatible; dbmock/{})",
            gen_alphanum(4)
        )),
        // ★ 修复：Username 调用 unique_username()，
        //   unique_username() 现在使用 fake::faker::internet::en::Username
        //   生成 ASCII 用户名（原来错用 zh_tw::Username 生成繁体中文用户名）。
        FieldKind::Username => sql_str(unique_username()),

        // ── 日期 / 时间 ──────────────────────────────────────────────────────
        FieldKind::TimestampTz => {
            let (from, to) = parse_date_range(fc);
            let offset_days = rand::thread_rng().gen_range(0..=(to - from).num_days().max(0));
            let secs: i64 = rand::thread_rng().gen_range(0..=86_399);
            let ts = (from + Duration::days(offset_days))
                .and_hms_opt(0, 0, 0)
                .unwrap()
                + Duration::seconds(secs);
            sql_str(ts.format("%Y-%m-%d %H:%M:%S+00").to_string())
        }
        FieldKind::Timestamp => {
            let (from, to) = parse_date_range(fc);
            let offset_days = rand::thread_rng().gen_range(0..=(to - from).num_days().max(0));
            let secs: i64 = rand::thread_rng().gen_range(0..=86_399);
            let ts = (from + Duration::days(offset_days))
                .and_hms_opt(0, 0, 0)
                .unwrap()
                + Duration::seconds(secs);
            sql_str(ts.format("%Y-%m-%d %H:%M:%S").to_string())
        }
        FieldKind::Date => {
            let (from, to) = parse_date_range(fc);
            let offset_days = rand::thread_rng().gen_range(0..=(to - from).num_days().max(0));
            let d = from + Duration::days(offset_days);
            sql_str(d.format("%Y-%m-%d").to_string())
        }
        FieldKind::Time => {
            let mut rng = rand::thread_rng();
            sql_str(format!(
                "{:02}:{:02}:{:02}",
                rng.gen_range(0..=23_u8),
                rng.gen_range(0..=59_u8),
                rng.gen_range(0..=59_u8),
            ))
        }

        // ── 结构化数据 ───────────────────────────────────────────────────────
        FieldKind::Json => {
            if let Some(schema) = &fc.json_schema {
                let json_value = generate_json_value(schema);
                let json_string = serde_json::to_string(&json_value).unwrap();
                sql_str(json_string)
            } else {
                // 生成一个简单的 JSON 对象，包含 1~4 个键值对
                let mut rng = rand::thread_rng();
                let n = rng.gen_range(1_usize..=4);
                let pairs: Vec<String> = (0..n)
                    .map(|i| {
                        let k = gen_alphanum(rng.gen_range(3..=8));
                        let v = match i % 4 {
                            0 => rng.gen_range(0_i64..=9_999).to_string(),
                            1 => format!("\"{}\"", gen_alphanum(rng.gen_range(4..=10))),
                            2 => {
                                if rng.gen_bool(0.5) {
                                    "true".into()
                                } else {
                                    "false".into()
                                }
                            }
                            _ => format!("{:.2}", rng.gen_range(0.0_f64..=100.0)),
                        };
                        format!("\"{}\":{}", k, v)
                    })
                    .collect();
                sql_str(format!("{{{}}}", pairs.join(",")))
            }
        }
    };

    Some(val)
}

// ── 辅助函数 ───────────────────────────────────────────────────────────────────

/// 从 FieldConfig 中解析 date_from 和 date_to。
fn parse_date_range(fc: &FieldConfig) -> (NaiveDate, NaiveDate) {
    let today = Utc::now().date_naive();
    let from = fc
        .date_from
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| today - Duration::days(1_825));
    let to = fc
        .date_to
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(today);
    let (from, to) = if from <= to { (from, to) } else { (to, from) };
    (from, to)
}

/// 将字符串包装为 SQL 字面量，并对单引号进行转义。
fn sql_str(s: String) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// 生成指定长度的字母数字随机字符串（小写字母 + 数字）。
fn gen_alphanum(len: usize) -> String {
    const C: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..len.max(1))
        .map(|_| C[rng.gen_range(0..C.len())] as char)
        .collect()
}

/// 生成指定长度的 Base64url 字符集随机字符串（用于模拟密码盐值和哈希）。
fn gen_base64url(len: usize) -> String {
    const C: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| C[rng.gen_range(0..C.len())] as char)
        .collect()
}

/// 生成指定长度的小写单词（纯字母）。
fn gen_word(len: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..len.max(1))
        .map(|_| rng.gen_range(b'a'..=b'z') as char)
        .collect()
}

/// 生成首字母大写、其余小写的单词。
fn gen_capitalized_word(len: usize) -> String {
    if len == 0 {
        return String::new();
    }
    let w = gen_word(len);
    let mut c = w.chars();
    match c.next() {
        None => String::new(),
        Some(h) => h.to_uppercase().to_string() + c.as_str(),
    }
}

/// 生成一个 slug（小写单词用短横线连接），并截断到最大长度。
fn gen_slug(max_len: usize) -> String {
    let n: usize = rand::thread_rng().gen_range(2..=4);
    let words: Vec<String> = (0..n)
        .map(|_| gen_word(rand::thread_rng().gen_range(3..=7)))
        .collect();
    let s = words.join("-");
    if s.len() > max_len {
        s[..max_len].to_string()
    } else {
        s
    }
}

/// 随机选择一个常见的顶级域名（TLD）。
fn pick_tld() -> &'static str {
    const T: [&str; 5] = ["com", "org", "net", "io", "co"];
    T[rand::thread_rng().gen_range(0..T.len())]
}

/// 判断给定的数据类型名称是否为整数类型。
fn is_int_type(dt: &str) -> bool {
    matches!(
        dt,
        "smallint"
            | "int2"
            | "int"
            | "int4"
            | "integer"
            | "bigint"
            | "int8"
            | "tinyint"
            | "mediumint"
            | "serial"
            | "bigserial"
            | "smallserial"
    ) || (dt.contains("int") && !dt.contains("point"))
}

/// 根据一组 `JsonFieldDef` 生成一个 JSON 对象值。
fn generate_json_value(defs: &[JsonFieldDef]) -> Value {
    const MAX_DEPTH: usize = 10;
    generate_json_value_with_depth(defs, 0, MAX_DEPTH)
}

/// 带深度控制的递归生成器，防止栈溢出。
fn generate_json_value_with_depth(defs: &[JsonFieldDef], depth: usize, max_depth: usize) -> Value {
    if depth > max_depth {
        return Value::Null;
    }
    let mut map = Map::new();
    let mut rng = rand::thread_rng();

    for def in defs {
        let skip_field = def.required == Some(false) && rng.gen_bool(0.2);
        if skip_field {
            continue;
        }
        let value = generate_value_from_json_def(def, depth, max_depth);
        map.insert(def.name.clone(), value);
    }
    Value::Object(map)
}

/// 根据单个 `JsonFieldDef` 生成对应的 JSON 值。
fn generate_value_from_json_def(def: &JsonFieldDef, depth: usize, max_depth: usize) -> Value {
    if let Some(properties) = &def.properties {
        return generate_json_value_with_depth(properties, depth + 1, max_depth);
    }
    if let Some(item_def) = &def.array_items {
        let mut rng = rand::thread_rng();
        let len = rng.gen_range(1..=5);
        let arr: Vec<Value> = (0..len)
            .map(|_| generate_value_from_json_def(item_def, depth + 1, max_depth))
            .collect();
        return Value::Array(arr);
    }
    match &def.kind {
        FieldKind::Null => Value::Null,
        FieldKind::Constant => Value::String(String::new()),
        FieldKind::Uuid => Value::String(Uuid::new_v4().to_string()),
        FieldKind::Unique | FieldKind::Sequence => {
            let n: u64 = rand::thread_rng().gen_range(1..=999999);
            Value::Number(Number::from(n))
        }
        FieldKind::Int => {
            let n: i64 = rand::thread_rng().gen_range(0..=1000);
            Value::Number(Number::from(n))
        }
        FieldKind::Float | FieldKind::Decimal => {
            let f: f64 = rand::thread_rng().gen_range(0.0..=1000.0);
            Number::from_f64(f)
                .map(Value::Number)
                .unwrap_or(Value::Null)
        }
        FieldKind::Bool => Value::Bool(rand::thread_rng().gen_bool(0.5)),
        FieldKind::String => {
            let len = rand::thread_rng().gen_range(4..=16);
            Value::String(gen_alphanum(len))
        }
        FieldKind::Text => {
            let word_count = rand::thread_rng().gen_range(5..=15);
            let words: Vec<String> = (0..word_count)
                .map(|_| gen_word(rand::thread_rng().gen_range(3..=9)))
                .collect();
            Value::String(words.join(" "))
        }
        FieldKind::Label => {
            let words: Vec<String> = (0..rand::thread_rng().gen_range(1..=2))
                .map(|_| gen_capitalized_word(rand::thread_rng().gen_range(3..=8)))
                .collect();
            Value::String(words.join(" "))
        }
        FieldKind::Slug => Value::String(gen_slug(64)),
        FieldKind::Enum => {
            // JsonFieldDef 中的 Enum 需要在调用方通过 values 配置指定，
            // 此处无法访问 values，回退为空字符串。
            Value::String(String::new())
        }
        FieldKind::Email => {
            // JSON 内部的邮箱不需要全局唯一，使用内联生成即可
            let mut rng = rand::thread_rng();
            let user = gen_alphanum(rng.gen_range(4..=10));
            let tag: u32 = rng.gen_range(10..=999);
            let domain = gen_alphanum(rng.gen_range(4..=9));
            Value::String(format!("{}.{}@{}.{}", user, tag, domain, pick_tld()))
        }
        FieldKind::Url => {
            let mut rng = rand::thread_rng();
            let host = gen_alphanum(rng.gen_range(5..=12));
            let path = gen_alphanum(rng.gen_range(3..=8));
            Value::String(format!("https://www.{}.{}/{}", host, pick_tld(), path))
        }
        FieldKind::Ip => {
            let mut rng = rand::thread_rng();
            Value::String(format!(
                "{}.{}.{}.{}",
                rng.gen_range(1_u8..=254),
                rng.gen_range(0_u8..=255),
                rng.gen_range(0_u8..=255),
                rng.gen_range(1_u8..=254),
            ))
        }
        FieldKind::Semver => {
            let mut rng = rand::thread_rng();
            Value::String(format!(
                "{}.{}.{}",
                rng.gen_range(0..=5),
                rng.gen_range(0..=20),
                rng.gen_range(0..=99)
            ))
        }
        FieldKind::Password => {
            let rounds: u8 = rand::thread_rng().gen_range(10..=13);
            let salt = gen_base64url(22);
            let hash = gen_base64url(31);
            Value::String(format!("$2b${:02}${}{}", rounds, salt, hash))
        }
        FieldKind::Phone => {
            let mut rng = rand::thread_rng();
            let cc: u16 = rng.gen_range(1..=99);
            let sub: u64 = rng.gen_range(100_000_000..=9_999_999_999);
            Value::String(format!("+{}{}", cc, sub))
        }
        FieldKind::Color => Value::String(format!(
            "#{:06X}",
            rand::thread_rng().gen_range(0_u32..=0xFF_FF_FF)
        )),
        FieldKind::UserAgent => Value::String(format!(
            "Mozilla/5.0 (compatible; dbmock/{})",
            gen_alphanum(4)
        )),
        // JSON 内部的 Username 无需全局唯一，使用随机 ASCII 字符串
        FieldKind::Username => {
            let mut rng = rand::thread_rng();
            let base = gen_word(rng.gen_range(4..=8));
            let n: u32 = rng.gen_range(10..=9999);
            Value::String(format!("{}{}", base, n))
        }
        FieldKind::TimestampTz | FieldKind::Timestamp | FieldKind::Date | FieldKind::Time => {
            let now = Utc::now();
            Value::String(now.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        }
        FieldKind::Json => Value::Object(Map::new()),
        _ => Value::Null,
    }
}
