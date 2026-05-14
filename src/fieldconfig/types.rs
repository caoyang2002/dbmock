//! types.rs — FieldType: the complete user-configurable type system.
//!
//! Every variant maps 1-to-1 to a YAML tag that users write in config.yml.
//! All variants are serde-tagged so YAML round-trips cleanly.
//!
//! Supported types (full reference):
//!
//!   default          — auto-detect from schema (name + DB type)
//!   null             — always emit NULL
//!   bool             — random true/false
//!   int              — integer in [min, max]
//!   float            — float in [min, max] with given decimal places
//!   decimal          — fixed-point with explicit scale
//!   string           — random alphanum string, length in [min_len, max_len]
//!   enum             — pick randomly from a fixed value list
//!   sequence         — monotonically incrementing integer (1, 2, 3, …)
//!   unique           — globally unique alphanum token (per column per run)
//!   uuid             — RFC-4122 UUID v4
//!   email            — user.N@domain.tld
//!   url              — https://www.host.tld/path
//!   phone            — E.164 formatted phone number
//!   name             — capitalized word(s)
//!   username         — lowercase word + digits
//!   password         — bcrypt-shaped hash ($2b$NN$...)
//!   ip               — IPv4 dotted-decimal
//!   ipv6             — IPv6 colon-hex
//!   color            — CSS hex color #RRGGBB
//!   slug             — kebab-case-words
//!   version          — semver X.Y.Z
//!   timestamp        — 'YYYY-MM-DD HH:MM:SS+00' (with tz)
//!   timestamp_ntz    — 'YYYY-MM-DD HH:MM:SS'    (without tz)
//!   date             — 'YYYY-MM-DD'
//!   time             — 'HH:MM:SS'
//!   json             — small JSON object literal
//!   paragraph        — N random words joined by spaces (long text)
//!   sentence         — short capitalized phrase
//!   regex            — string matching a user-supplied regex pattern
//!                      (generation uses simple char-class expansion)
//!   constant         — always emit the literal `value` string as-is

use std::collections::HashSet;
use std::str;
use std::sync::{Arc, Mutex};

use rand::Rng;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// FieldType — user-configurable, YAML-serializable
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldType {
    // ── meta / special ────────────────────────────────────────────────────────
    /// Auto-detect from schema (name + DB type). Always the initial default.
    Default,

    /// Always emit SQL NULL.
    Null,

    /// A fixed SQL literal emitted verbatim (must be a valid SQL expression).
    /// Example: value: "'pending'"  or  value: "42"
    Constant {
        value: String,
    },

    // ── booleans ──────────────────────────────────────────────────────────────
    Bool,

    // ── numerics ──────────────────────────────────────────────────────────────
    Int {
        #[serde(default = "default_int_min")]
        min: i64,
        #[serde(default = "default_int_max")]
        max: i64,
    },

    Float {
        #[serde(default = "default_float_min")]
        min: f64,
        #[serde(default = "default_float_max")]
        max: f64,
        #[serde(default = "default_float_scale")]
        scale: usize,
    },

    Decimal {
        #[serde(default = "default_float_min")]
        min: f64,
        #[serde(default = "default_float_max")]
        max: f64,
        #[serde(default = "default_decimal_scale")]
        scale: usize,
    },

    /// Monotonically incrementing integer starting from `start`.
    Sequence {
        #[serde(default = "default_sequence_start")]
        start: i64,
        #[serde(default = "default_sequence_step")]
        step: i64,
    },

    // ── text / string ─────────────────────────────────────────────────────────
    /// Random alphanum string.
    String {
        #[serde(default = "default_str_min")]
        min_len: usize,
        #[serde(default = "default_str_max")]
        max_len: usize,
    },

    /// Pick randomly from a fixed list (stored as SQL literals or raw values).
    Enum {
        values: Vec<String>,
    },

    /// Globally unique per-column random token.
    Unique {
        #[serde(default = "default_unique_len")]
        len: usize,
    },

    /// Paragraph of random words.
    Paragraph {
        #[serde(default = "default_para_min")]
        min_words: usize,
        #[serde(default = "default_para_max")]
        max_words: usize,
    },

    /// Short capitalized phrase.
    Sentence {
        #[serde(default = "default_sentence_min")]
        min_words: usize,
        #[serde(default = "default_sentence_max")]
        max_words: usize,
    },

    // ── identifiers / semantic ────────────────────────────────────────────────
    Uuid,
    Email,
    Url,
    Phone,
    Name,
    Username,
    Password,
    Ip,
    Ipv6,
    Color,
    Slug,
    Version,
    Json,

    // ── temporal ──────────────────────────────────────────────────────────────
    /// Timestamp with UTC offset: 'YYYY-MM-DD HH:MM:SS+00'
    Timestamp,

    /// Timestamp without timezone: 'YYYY-MM-DD HH:MM:SS'
    TimestampNtz,

    Date,
    Time,

    // ── advanced ──────────────────────────────────────────────────────────────
    /// Generate a string matching a simple regex pattern.
    /// Supported tokens: [abc], [a-z], [A-Z], [0-9], ., \d, \w, \s, +, *, ?, {n}, {n,m}
    Regex {
        pattern: String,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Default parameter values (used by serde)
// ─────────────────────────────────────────────────────────────────────────────

fn default_int_min() -> i64 {
    1
}
fn default_int_max() -> i64 {
    1_000_000
}
fn default_float_min() -> f64 {
    0.0
}
fn default_float_max() -> f64 {
    9_999.0
}
fn default_float_scale() -> usize {
    4
}
fn default_decimal_scale() -> usize {
    2
}
fn default_sequence_start() -> i64 {
    1
}
fn default_sequence_step() -> i64 {
    1
}
fn default_str_min() -> usize {
    4
}
fn default_str_max() -> usize {
    32
}
fn default_unique_len() -> usize {
    16
}
fn default_para_min() -> usize {
    10
}
fn default_para_max() -> usize {
    30
}
fn default_sentence_min() -> usize {
    3
}
fn default_sentence_max() -> usize {
    8
}

// ─────────────────────────────────────────────────────────────────────────────
// Sequence state — per-column counter, shared across batch rows
// ─────────────────────────────────────────────────────────────────────────────

/// Thread-safe counter for Sequence fields.
#[derive(Clone, Debug)]
pub struct SeqCounter(Arc<Mutex<i64>>);

impl SeqCounter {
    pub fn new(start: i64) -> Self {
        Self(Arc::new(Mutex::new(start)))
    }

    pub fn next(&self, step: i64) -> i64 {
        let mut g = self.0.lock().unwrap();
        let v = *g;
        *g += step;
        v
    }
}

/// Per-column uniqueness tracker.
#[derive(Clone, Debug, Default)]
pub struct UniqueSeen(Arc<Mutex<HashSet<String>>>);

impl UniqueSeen {
    pub fn insert(&self, v: &str) -> bool {
        self.0.lock().unwrap().insert(v.to_string())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Generator — produce a SQL literal from a FieldType
// ─────────────────────────────────────────────────────────────────────────────

/// Context passed to the generator so stateful variants (Sequence, Unique)
/// can share state across rows within the same batch session.
pub struct GenCtx {
    pub seq: Option<SeqCounter>,
    pub seen: Option<UniqueSeen>,
}

impl GenCtx {
    pub fn for_field(ft: &FieldType) -> Self {
        match ft {
            FieldType::Sequence { start, .. } => Self {
                seq: Some(SeqCounter::new(*start)),
                seen: None,
            },
            FieldType::Unique { .. } => Self {
                seq: None,
                seen: Some(UniqueSeen::default()),
            },
            _ => Self {
                seq: None,
                seen: None,
            },
        }
    }
}

/// Generate a SQL literal for the given `FieldType`.
pub fn generate_for_type(ft: &FieldType, ctx: &GenCtx) -> String {
    use FieldType::*;
    match ft {
        Default => unreachable!("Default should be resolved before generate_for_type"),
        Null => "NULL".to_string(),
        Constant { value } => value.clone(),
        Bool => gen_bool(),
        Int { min, max } => rand::thread_rng().gen_range(*min..=*max).to_string(),
        Float { min, max, scale } => {
            let v: f64 = rand::thread_rng().gen_range(*min..=*max);
            format!("{:.prec$}", v, prec = *scale)
        }
        Decimal { min, max, scale } => {
            let v: f64 = rand::thread_rng().gen_range(*min..=*max);
            format!("{:.prec$}", v, prec = *scale)
        }
        Sequence { step, .. } => {
            let counter = ctx.seq.as_ref().expect("SeqCounter missing");
            counter.next(*step).to_string()
        }
        String { min_len, max_len } => {
            let len = rand::thread_rng().gen_range(*min_len..=(*max_len).max(*min_len + 1));
            sql_str(gen_alphanum(len))
        }
        Enum { values } => {
            if values.is_empty() {
                return "NULL".to_string();
            }
            let idx = rand::thread_rng().gen_range(0..values.len());
            // Values are stored as raw strings; wrap in SQL quotes.
            sql_str(values[idx].clone())
        }
        Unique { len } => {
            let seen = ctx.seen.as_ref().expect("UniqueSeen missing");
            // Retry until unique (collision rate is negligible for sensible len).
            loop {
                let candidate = gen_alphanum(*len);
                if seen.insert(&candidate) {
                    return sql_str(candidate);
                }
            }
        }
        Paragraph {
            min_words,
            max_words,
        } => {
            let n = rand::thread_rng().gen_range(*min_words..=(*max_words).max(*min_words + 1));
            let words: Vec<_> = (0..n)
                .map(|_| gen_word(rand::thread_rng().gen_range(3..=9)))
                .collect();
            sql_str(words.join(" "))
        }
        Sentence {
            min_words,
            max_words,
        } => {
            let n = rand::thread_rng().gen_range(*min_words..=(*max_words).max(*min_words + 1));
            let words: Vec<_> = (0..n)
                .map(|_| gen_cap_word(rand::thread_rng().gen_range(3..=8)))
                .collect();
            sql_str(words.join(" "))
        }
        Uuid => sql_str(uuid::Uuid::new_v4().to_string()),
        Email => gen_email(),
        Url => gen_url(),
        Phone => gen_phone(),
        Name => gen_name(),
        Username => gen_username(),
        Password => gen_password(),
        Ip => gen_ip(),
        Ipv6 => gen_ipv6(),
        Color => gen_color(),
        Slug => gen_slug(),
        Version => gen_version(),
        Json => gen_json(),
        Timestamp => gen_timestamptz(),
        TimestampNtz => gen_timestamp(),
        Date => gen_date(),
        Time => gen_time(),
        Regex { pattern } => sql_str(gen_from_regex(pattern)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Primitive generators (all pure, no hard-coded data)
// ─────────────────────────────────────────────────────────────────────────────

fn gen_bool() -> String {
    if rand::thread_rng().gen_bool(0.5) {
        "true".to_string()
    } else {
        "false".to_string()
    }
}

fn gen_alphanum(len: usize) -> String {
    const C: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| C[rng.gen_range(0..C.len())] as char)
        .collect()
}

fn gen_word(len: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| rng.gen_range(b'a'..=b'z') as char)
        .collect()
}

fn gen_cap_word(len: usize) -> String {
    let w = gen_word(len);
    let mut c = w.chars();
    match c.next() {
        None => String::new(),
        Some(h) => h.to_uppercase().to_string() + c.as_str(),
    }
}

fn gen_email() -> String {
    let user = gen_alphanum(rand::thread_rng().gen_range(4..=10));
    let tag: u32 = rand::thread_rng().gen_range(10..=999);
    let domain = gen_alphanum(rand::thread_rng().gen_range(4..=9));
    sql_str(format!("{}.{}@{}.{}", user, tag, domain, pick_tld()))
}

fn gen_url() -> String {
    let host = gen_alphanum(rand::thread_rng().gen_range(5..=12));
    let path = gen_alphanum(rand::thread_rng().gen_range(3..=8));
    sql_str(format!("https://www.{}.{}/{}", host, pick_tld(), path))
}

fn gen_phone() -> String {
    let mut rng = rand::thread_rng();
    let cc: u16 = rng.gen_range(1..=99);
    let sub: u64 = rng.gen_range(100_000_000..=9_999_999_999);
    sql_str(format!("+{}{}", cc, sub))
}

fn gen_name() -> String {
    let word_count = rand::thread_rng().gen_range(1..=2_usize);
    let words: Vec<_> = (0..word_count)
        .map(|_| gen_cap_word(rand::thread_rng().gen_range(3..=9)))
        .collect();
    sql_str(words.join(" "))
}

fn gen_username() -> String {
    let base = gen_word(rand::thread_rng().gen_range(4..=8));
    let n: u32 = rand::thread_rng().gen_range(10..=9999);
    sql_str(format!("{}{}", base, n))
}

fn gen_password() -> String {
    let rounds: u8 = rand::thread_rng().gen_range(10..=13);
    let salt = gen_base64url(22);
    let hash = gen_base64url(31);
    sql_str(format!("$2b${:02}${}{}", rounds, salt, hash))
}

fn gen_ip() -> String {
    let mut rng = rand::thread_rng();
    sql_str(format!(
        "{}.{}.{}.{}",
        rng.gen_range(1_u8..=254),
        rng.gen_range(0_u8..=255),
        rng.gen_range(0_u8..=255),
        rng.gen_range(1_u8..=254),
    ))
}

fn gen_ipv6() -> String {
    let mut rng = rand::thread_rng();
    let groups: Vec<String> = (0..8)
        .map(|_| format!("{:04x}", rng.gen_range(0_u16..=0xFFFF)))
        .collect();
    sql_str(groups.join(":"))
}

fn gen_color() -> String {
    sql_str(format!(
        "#{:06X}",
        rand::thread_rng().gen_range(0_u32..=0xFF_FF_FF)
    ))
}

fn gen_slug() -> String {
    let n = rand::thread_rng().gen_range(2..=4_usize);
    let parts: Vec<_> = (0..n)
        .map(|_| gen_word(rand::thread_rng().gen_range(3..=7)))
        .collect();
    sql_str(parts.join("-"))
}

fn gen_version() -> String {
    let mut rng = rand::thread_rng();
    sql_str(format!(
        "{}.{}.{}",
        rng.gen_range(0..=5),
        rng.gen_range(0..=20),
        rng.gen_range(0..=99)
    ))
}

fn gen_json() -> String {
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
                        "true".to_string()
                    } else {
                        "false".to_string()
                    }
                }
                _ => format!("{:.2}", rng.gen_range(0.0_f64..=100.0)),
            };
            format!("\"{}\":{}", k, v)
        })
        .collect();
    sql_str(format!("{{{}}}", pairs.join(",")))
}

fn gen_timestamptz() -> String {
    let (d, s) = rand_offset();
    let ts = chrono::Utc::now() - chrono::Duration::days(d) - chrono::Duration::seconds(s);
    sql_str(ts.format("%Y-%m-%d %H:%M:%S+00").to_string())
}

fn gen_timestamp() -> String {
    let (d, s) = rand_offset();
    let ts = chrono::Utc::now() - chrono::Duration::days(d) - chrono::Duration::seconds(s);
    sql_str(ts.format("%Y-%m-%d %H:%M:%S").to_string())
}

fn gen_date() -> String {
    let days: i64 = rand::thread_rng().gen_range(0..=1_825);
    let d = (chrono::Utc::now() - chrono::Duration::days(days)).date_naive();
    sql_str(d.format("%Y-%m-%d").to_string())
}

fn gen_time() -> String {
    let mut rng = rand::thread_rng();
    sql_str(format!(
        "{:02}:{:02}:{:02}",
        rng.gen_range(0..=23_u8),
        rng.gen_range(0..=59_u8),
        rng.gen_range(0..=59_u8)
    ))
}

fn gen_base64url(len: usize) -> String {
    const C: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| C[rng.gen_range(0..C.len())] as char)
        .collect()
}

fn rand_offset() -> (i64, i64) {
    let mut rng = rand::thread_rng();
    (rng.gen_range(0..=730), rng.gen_range(0..=86_399))
}

fn pick_tld() -> &'static str {
    const T: [&str; 5] = ["com", "org", "net", "io", "co"];
    T[rand::thread_rng().gen_range(0..T.len())]
}

/// Minimal regex-to-string generator.
/// Supports: [abc], [a-z], \d, \w, \s, ., {n}, {n,m}, ?, +, *
/// Anything unrecognised is emitted verbatim (best-effort).
fn gen_from_regex(pattern: &str) -> String {
    let mut rng = rand::thread_rng();
    let mut out = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '\\' if i + 1 < chars.len() => {
                let ch = match chars[i + 1] {
                    'd' => (b'0' + rng.gen_range(0..=9)) as char,
                    'D' => (b'a' + rng.gen_range(0..=25)) as char,
                    'w' => {
                        const W: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_";
                        W[rng.gen_range(0..W.len())] as char
                    }
                    's' => ' ',
                    'n' => '\n',
                    't' => '\t',
                    c => c,
                };
                out.push(ch);
                i += 2;
            }
            '[' => {
                // Collect charset
                i += 1;
                let mut charset: Vec<char> = Vec::new();
                while i < chars.len() && chars[i] != ']' {
                    if i + 2 < chars.len() && chars[i + 1] == '-' {
                        let lo = chars[i] as u8;
                        let hi = chars[i + 2] as u8;
                        if lo <= hi {
                            for b in lo..=hi {
                                charset.push(b as char);
                            }
                        }
                        i += 3;
                    } else {
                        charset.push(chars[i]);
                        i += 1;
                    }
                }
                if chars.get(i) == Some(&']') {
                    i += 1;
                }
                if !charset.is_empty() {
                    out.push(charset[rng.gen_range(0..charset.len())]);
                }
            }
            '.' => {
                const P: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
                out.push(P[rng.gen_range(0..P.len())] as char);
                i += 1;
            }
            '{' => {
                // {n} or {n,m} — repeat the last char
                i += 1;
                let mut num_str = String::new();
                while i < chars.len() && chars[i].is_ascii_digit() {
                    num_str.push(chars[i]);
                    i += 1;
                }
                let n_min: usize = num_str.parse().unwrap_or(1);
                let n_max = if chars.get(i) == Some(&',') {
                    i += 1;
                    let mut s2 = String::new();
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        s2.push(chars[i]);
                        i += 1;
                    }
                    s2.parse().unwrap_or(n_min)
                } else {
                    n_min
                };
                if chars.get(i) == Some(&'}') {
                    i += 1;
                }
                let repeat = rng.gen_range(n_min..=n_max.max(n_min));
                // Repeat last character of out
                if let Some(last) = out.chars().last() {
                    // Remove 1 copy (the one already added) then add repeat copies
                    out.pop();
                    for _ in 0..repeat {
                        out.push(last);
                    }
                }
            }
            '?' => {
                // Make preceding char optional
                if rng.gen_bool(0.5) {
                    // keep it (already in out)
                } else {
                    out.pop();
                }
                i += 1;
            }
            '+' | '*' => {
                // Repeat last char 1-5 or 0-5 times
                let min_r = if chars[i] == '+' { 1 } else { 0 };
                let extra = rng.gen_range(min_r..=4_usize);
                if let Some(last) = out.chars().last() {
                    for _ in 0..extra {
                        out.push(last);
                    }
                }
                i += 1;
            }
            '^' | '$' => {
                i += 1;
            } // anchors — skip
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

fn sql_str(s: String) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// 单个列的生成配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldConfig {
    /// 生成策略。
    #[serde(rename = "type")]
    pub kind: FieldKind,

    /// 用于 `enum` 和 `set` 类型：随机选取的允许值列表。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,

    /// 用于 `int` / `float` / `decimal`：下限（包含）。使用 f64 存储。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,

    /// 用于 `int` / `float` / `decimal`：上限（包含）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,

    /// 用于 `decimal`：小数位数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<usize>,

    /// 用于 `string` / `text`：最小字符长度。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_len: Option<usize>,

    /// 用于 `string` / `text`：最大字符长度。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_len: Option<usize>,

    /// 用于 `regex`：正则表达式模式（尽力展开为字母数字字符串）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,

    /// 用于 `timestamp` / `date`：最早日期（YYYY-MM-DD）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_from: Option<String>,

    /// 用于 `timestamp` / `date`：最晚日期（YYYY-MM-DD）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_to: Option<String>,

    /// 覆盖 NULL 值的概率（0.0–1.0）。对于可空列，默认值为 0.15。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub null_rate: Option<f64>,

    /// 常量字面量——始终精确输出此字符串（会对 SQL 进行转义）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constant: Option<String>,
}

/// 所有支持的生成策略。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    // ── 元策略 ────────────────────────────────────────────────────────────────
    /// 使用基于数据库模式的自动推断（这是每个列的默认行为）。
    Default,
    /// 完全跳过此列（不包含在 INSERT 语句中）。
    Skip,
    /// 始终输出 NULL。
    Null,
    /// 始终输出 `constant` 中指定的字面量。
    Constant,

    // ── 标识符 / 主键 ──────────────────────────────────────────────────────────
    /// 生成全局唯一的 UUID v4 字符串。
    Uuid,
    /// 保证在当前运行中唯一（基于计数器的后缀）。
    Unique,

    // ── 数字类型 ──────────────────────────────────────────────────────────────
    /// 随机整数。支持 `min` / `max` 参数。
    Int,
    /// 随机浮点数。支持 `min` / `max` 参数。
    Float,
    /// 固定小数位数的十进制数。支持 `min` / `max` / `scale` 参数。
    Decimal,
    /// 从 1 开始的自增计数器。如果用于主键，建议配合 `skip` 使用（由数据库自动生成）。
    Sequence,

    // ── 布尔类型 ──────────────────────────────────────────────────────────────
    /// true / false（各 50% 概率，也可以使用 `values: [true]` 固定为 true）。
    Bool,

    // ── 文本类型 ──────────────────────────────────────────────────────────────
    /// 通用的随机字母数字字符串。支持 `min_len` / `max_len` 参数。
    String,
    /// 多词散文段落。支持 `max_len` 参数。
    Text,
    /// 简短的首字母大写的标签（1–2 个词）。
    Label,
    /// 小写连字符分隔的 slug。
    Slug,
    /// 语义枚举：从 `values` 列表中随机选取一个值。
    Enum,

    // ── 互联网相关 ────────────────────────────────────────────────────────────
    /// 邮箱地址 user@domain.tld。
    Email,
    /// URL 地址 https://www.domain.tld/path。
    Url,
    /// IPv4 地址（点分十进制）。
    Ip,
    /// 语义化版本号 major.minor.patch。
    Semver,

    // ── 个人信息 ──────────────────────────────────────────────────────────────
    /// bcrypt 格式的哈希字符串。
    Password,
    /// 电话号码 +CC subscriber-number。
    Phone,
    /// CSS 十六进制颜色 #RRGGBB。
    Color,
    /// 浏览器 User-Agent 字符串。
    UserAgent,

    // ── 日期 / 时间 ──────────────────────────────────────────────────────────
    /// 带时区的时间戳 —— 'YYYY-MM-DD HH:MM:SS+00'。支持 `date_from` / `date_to`。
    TimestampTz,
    /// 不带时区的时间戳。支持 `date_from` / `date_to`。
    Timestamp,
    /// 日期 'YYYY-MM-DD'。支持 `date_from` / `date_to`。
    Date,
    /// 时间 'HH:MM:SS'。
    Time,

    // ── 结构化数据 ───────────────────────────────────────────────────────────
    /// 随机的 JSON 对象字面量。
    Json,
}

impl FieldKind {
    /// 导出配置时显示的、人类可读的描述（会写入 YAML 注释中）。
    pub fn description(&self) -> &'static str {
        match self {
            FieldKind::Default => "基于模式的自动推断（保持默认即可）",
            FieldKind::Skip => "从 INSERT 语句中排除此列",
            FieldKind::Null => "始终为 NULL",
            FieldKind::Constant => "始终输出 `constant:` 中指定的值",
            FieldKind::Uuid => "UUID v4 字符串",
            FieldKind::Unique => "本次运行中唯一的值（通过计数器后缀保证唯一性）",
            FieldKind::Int => "随机整数 [min, max]",
            FieldKind::Float => "随机浮点数 [min, max]",
            FieldKind::Decimal => "固定小数位的十进制数 [min, max]，小数位数由 `scale:` 指定",
            FieldKind::Sequence => "从 1 开始单调递增的计数器",
            FieldKind::Bool => "true 或 false",
            FieldKind::String => "随机字母数字字符串 [min_len, max_len]",
            FieldKind::Text => "随机散文段落，最大长度受 `max_len` 限制",
            FieldKind::Label => "简短的首字母大写标签（1–2 个词）",
            FieldKind::Slug => "小写连字符分隔的 slug",
            FieldKind::Enum => "从 `values:` 列表中随机选取",
            FieldKind::Email => "user@domain.tld 邮箱地址",
            FieldKind::Url => "https://www.domain.tld/path 网址",
            FieldKind::Ip => "IPv4 地址",
            FieldKind::Semver => "语义化版本号 major.minor.patch",
            FieldKind::Password => "bcrypt 格式的哈希字符串",
            FieldKind::Phone => "+CC 国家码 + 用户号码",
            FieldKind::Color => "CSS 十六进制颜色 #RRGGBB",
            FieldKind::UserAgent => "浏览器 User-Agent 字符串",
            FieldKind::TimestampTz => "带时区的时间戳 [date_from, date_to]",
            FieldKind::Timestamp => "不带时区的时间戳 [date_from, date_to]",
            FieldKind::Date => "日期 YYYY-MM-DD [date_from, date_to]",
            FieldKind::Time => "时间 HH:MM:SS",
            FieldKind::Json => "随机 JSON 对象",
        }
    }
}

impl Default for FieldConfig {
    fn default() -> Self {
        Self {
            kind: FieldKind::Default,
            values: None,
            min: None,
            max: None,
            scale: None,
            min_len: None,
            max_len: None,
            pattern: None,
            date_from: None,
            date_to: None,
            null_rate: None,
            constant: None,
        }
    }
}
