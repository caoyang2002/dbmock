//! generate.rs — value generation driven by FieldConfig.
//!
//! `generate_with_config` is the primary entry point used by batch.rs
//! when a mock_config.yml is present. It covers every FieldKind variant
//! and falls back to the schema-driven generator for `Default`.

use chrono::{Duration, NaiveDate, Utc};
use rand::Rng;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use uuid::Uuid;

use crate::core::schema::ColumnSchema;
use crate::fieldconfig::types::{FieldConfig, FieldKind};
use crate::generator::value::generate_value; // fallback

// ── per-column unique counters ────────────────────────────────────────────────
// Key: "table.column" → counter
static UNIQUE_COUNTERS: std::sync::OnceLock<
    std::sync::Mutex<HashMap<String, u64>>
> = std::sync::OnceLock::new();

fn next_unique(key: &str) -> u64 {
    let mut map = UNIQUE_COUNTERS
        .get_or_init(|| std::sync::Mutex::new(HashMap::new()))
        .lock()
        .unwrap();
    let counter = map.entry(key.to_string()).or_insert(0);
    *counter += 1;
    *counter
}

/// Reset all unique counters (called at the start of each generate run).
pub fn reset_unique_counters() {
    if let Some(lock) = UNIQUE_COUNTERS.get() {
        lock.lock().unwrap().clear();
    }
}

// ── main entry point ──────────────────────────────────────────────────────────

/// Generate a SQL literal for `col` according to `fc`.
///
/// `unique_key` is "table_name.col_name" used to track the unique counter.
/// `db_type`    drives quoting style for booleans.
pub fn generate_with_config(
    col: &ColumnSchema,
    fc: &FieldConfig,
    unique_key: &str,
    db_type: &str,
) -> Option<String> {
    // Honour null_rate override (or keep the 15% default for nullable cols).
    let null_rate = fc.null_rate.unwrap_or(if col.is_nullable { 0.15 } else { 0.0 });
    if null_rate > 0.0 && rand::thread_rng().gen_bool(null_rate.clamp(0.0, 1.0)) {
        return Some("NULL".to_string());
    }

    let val = match &fc.kind {
        // ── meta ─────────────────────────────────────────────────────────────
        FieldKind::Default  => return None, // caller falls back to schema-driven
        FieldKind::Skip     => return Some("__SKIP__".to_string()), // sentinel
        FieldKind::Null     => "NULL".to_string(),
        FieldKind::Constant => {
            sql_str(fc.constant.clone().unwrap_or_default())
        }

        // ── identity ─────────────────────────────────────────────────────────
        FieldKind::Uuid   => sql_str(Uuid::new_v4().to_string()),
        FieldKind::Unique => {
            let n = next_unique(unique_key);
            // Use the column's base type to shape the unique value.
            let dt = col.data_type.to_lowercase();
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

        // ── numbers ──────────────────────────────────────────────────────────
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
            let lo    = fc.min.unwrap_or(0.0);
            let hi    = fc.max.unwrap_or(999_999.0);
            let hi    = if hi <= lo { lo + 1.0 } else { hi };
            let v     = rand::thread_rng().gen_range(lo..hi);
            format!("{:.prec$}", v, prec = scale)
        }

        // ── boolean ──────────────────────────────────────────────────────────
        FieldKind::Bool => {
            let b = rand::thread_rng().gen_bool(0.5);
            let dt = col.data_type.to_lowercase();
            if dt.contains("bool") {
                if b { "true".to_string() } else { "false".to_string() }
            } else {
                if b { "1".to_string() } else { "0".to_string() }
            }
        }

        // ── text ─────────────────────────────────────────────────────────────
        FieldKind::String => {
            let lo  = fc.min_len.unwrap_or(4);
            let hi  = fc.max_len.unwrap_or(32).max(lo + 1);
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
            let s = if s.len() > max { s[..max].to_string() } else { s };
            sql_str(s)
        }
        FieldKind::Label => {
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

        // ── internet ─────────────────────────────────────────────────────────
        FieldKind::Email => {
            let user = gen_alphanum(rand::thread_rng().gen_range(4..=10));
            let tag: u32 = rand::thread_rng().gen_range(10..=999);
            let domain = gen_alphanum(rand::thread_rng().gen_range(4..=9));
            sql_str(format!("{}.{}@{}.{}", user, tag, domain, pick_tld()))
        }
        FieldKind::Url => {
            let host = gen_alphanum(rand::thread_rng().gen_range(5..=12));
            let path = gen_alphanum(rand::thread_rng().gen_range(3..=8));
            sql_str(format!("https://www.{}.{}/{}", host, pick_tld(), path))
        }
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
            sql_str(format!("{}.{}.{}", rng.gen_range(0..=5), rng.gen_range(0..=20), rng.gen_range(0..=99)))
        }

        // ── personal ─────────────────────────────────────────────────────────
        FieldKind::Password => {
            let rounds: u8 = rand::thread_rng().gen_range(10..=13);
            let salt = gen_base64url(22);
            let hash = gen_base64url(31);
            sql_str(format!("$2b${:02}${}{}", rounds, salt, hash))
        }
        FieldKind::Phone => {
            let mut rng = rand::thread_rng();
            let cc: u16  = rng.gen_range(1..=99);
            let sub: u64 = rng.gen_range(100_000_000..=9_999_999_999);
            sql_str(format!("+{}{}", cc, sub))
        }
        FieldKind::Color => {
            sql_str(format!("#{:06X}", rand::thread_rng().gen_range(0_u32..=0xFF_FF_FF)))
        }
        FieldKind::UserAgent => {
            sql_str(format!("Mozilla/5.0 (compatible; datamocker/{})", gen_alphanum(4)))
        }

        // ── date / time ──────────────────────────────────────────────────────
        FieldKind::TimestampTz => {
            let (from, to) = parse_date_range(fc);
            let offset_days = rand::thread_rng().gen_range(0..=(to - from).num_days().max(0));
            let secs: i64 = rand::thread_rng().gen_range(0..=86_399);
            let ts = (from + Duration::days(offset_days)).and_hms_opt(0, 0, 0).unwrap()
                + Duration::seconds(secs);
            sql_str(ts.format("%Y-%m-%d %H:%M:%S+00").to_string())
        }
        FieldKind::Timestamp => {
            let (from, to) = parse_date_range(fc);
            let offset_days = rand::thread_rng().gen_range(0..=(to - from).num_days().max(0));
            let secs: i64 = rand::thread_rng().gen_range(0..=86_399);
            let ts = (from + Duration::days(offset_days)).and_hms_opt(0, 0, 0).unwrap()
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

        // ── structured ───────────────────────────────────────────────────────
        FieldKind::Json => {
            let mut rng = rand::thread_rng();
            let n = rng.gen_range(1_usize..=4);
            let pairs: Vec<String> = (0..n).map(|i| {
                let k = gen_alphanum(rng.gen_range(3..=8));
                let v = match i % 4 {
                    0 => rng.gen_range(0_i64..=9_999).to_string(),
                    1 => format!("\"{}\"", gen_alphanum(rng.gen_range(4..=10))),
                    2 => if rng.gen_bool(0.5) { "true".into() } else { "false".into() },
                    _ => format!("{:.2}", rng.gen_range(0.0_f64..=100.0)),
                };
                format!("\"{}\":{}", k, v)
            }).collect();
            sql_str(format!("{{{}}}", pairs.join(",")))
        }
    };

    Some(val)
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Parse date_from / date_to from FieldConfig, defaulting to a 5-year window.
fn parse_date_range(fc: &FieldConfig) -> (NaiveDate, NaiveDate) {
    let today = Utc::now().date_naive();
    let from = fc.date_from.as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| today - Duration::days(1_825));
    let to = fc.date_to.as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(today);
    let (from, to) = if from <= to { (from, to) } else { (to, from) };
    (from, to)
}

fn sql_str(s: String) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn gen_alphanum(len: usize) -> String {
    const C: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..len.max(1)).map(|_| C[rng.gen_range(0..C.len())] as char).collect()
}

fn gen_base64url(len: usize) -> String {
    const C: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut rng = rand::thread_rng();
    (0..len).map(|_| C[rng.gen_range(0..C.len())] as char).collect()
}

fn gen_word(len: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..len.max(1)).map(|_| rng.gen_range(b'a'..=b'z') as char).collect()
}

fn gen_capitalized_word(len: usize) -> String {
    if len == 0 { return String::new(); }
    let w = gen_word(len);
    let mut c = w.chars();
    match c.next() {
        None    => String::new(),
        Some(h) => h.to_uppercase().to_string() + c.as_str(),
    }
}

fn gen_slug(max_len: usize) -> String {
    let n: usize = rand::thread_rng().gen_range(2..=4);
    let words: Vec<String> = (0..n)
        .map(|_| gen_word(rand::thread_rng().gen_range(3..=7)))
        .collect();
    let s = words.join("-");
    if s.len() > max_len { s[..max_len].to_string() } else { s }
}

fn pick_tld() -> &'static str {
    const T: [&str; 5] = ["com", "org", "net", "io", "co"];
    T[rand::thread_rng().gen_range(0..T.len())]
}

fn is_int_type(dt: &str) -> bool {
    matches!(dt,
        "smallint"|"int2"|"int"|"int4"|"integer"|"bigint"|"int8"|
        "tinyint"|"mediumint"|"serial"|"bigserial"|"smallserial"
    ) || (dt.contains("int") && !dt.contains("point"))
}