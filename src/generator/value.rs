//! value.rs — schema-driven SQL literal generator
//!
//! Every decision is derived from `ColumnSchema` fields:
//!   • data_type  (the raw string as stored by the driver, e.g. "timestamp with time zone")
//!   • max_length / numeric_precision / numeric_scale
//!   • is_nullable
//!   • col.name   (semantic hint only — never used to hard-code a fixed value)
//!
//! No static data arrays.  All output is constructed algorithmically.

use chrono::{Duration, Utc};
use rand::Rng;
use uuid::Uuid;

use crate::{
    core::schema::ColumnSchema,
    datapool::{
        random_image_url, unique_email, unique_phone_number, unique_username,
        user::random_avatar_url,
    },
};

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Return a SQL literal (or NULL) for `col`.
pub fn generate_value(col: &ColumnSchema, _db_type: &str) -> String {
    // Nullable: ~15 % NULL
    if col.is_nullable && rand::thread_rng().gen_bool(0.15) {
        return "NULL".to_string();
    }

    // 1. Classify the raw data_type string first — this is the ground truth.
    let kind = TypeKind::from_data_type(&col.data_type);

    // 2. Within each TypeKind, optionally refine using the column name as a
    //    *semantic hint*.  The hint never overrides the type class; it only
    //    chooses a more meaningful generator within the same class.
    kind.generate(col)
}

/// Generate a representative PK value matching the column's storage type.
/// Used by the engine to pre-populate FK pools.
pub fn generate_pk_value(col: &ColumnSchema) -> String {
    match TypeKind::from_data_type(&col.data_type) {
        TypeKind::Uuid => sql_str(Uuid::new_v4().to_string()),
        TypeKind::Integer { .. } => rand::thread_rng().gen_range(1_i64..=2_000_000).to_string(),
        _ => sql_str(gen_alphanum(12)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TypeKind — complete classification of every PostgreSQL / MySQL / SQLite type
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum TypeKind {
    /// INTEGER family: SERIAL, BIGINT, SMALLINT, INT, MEDIUMINT, TINYINT, …
    Integer { max: i64 },
    /// FLOAT / DOUBLE / REAL
    Float,
    /// DECIMAL / NUMERIC / MONEY  — carries scale from schema
    Decimal { scale: usize },
    /// BOOLEAN / TINYINT(1) / BIT
    Boolean,
    /// UUID (native)
    Uuid,
    /// JSON / JSONB
    Json,
    /// TIMESTAMP WITH TIME ZONE / TIMESTAMPTZ
    TimestampTz,
    /// TIMESTAMP WITHOUT TIME ZONE / TIMESTAMP
    Timestamp,
    /// DATE
    Date,
    /// TIME
    Time,
    /// YEAR (MySQL)
    Year,
    /// INTERVAL
    Interval,
    /// CHARACTER VARYING / VARCHAR / CHAR / TEXT / CLOB / CITEXT / NAME / TINYTEXT / MEDIUMTEXT / LONGTEXT
    Text { max_len: usize },
    /// BYTEA / BLOB / BINARY / VARBINARY
    Binary,
    /// ENUM / SET  — valid values unknown; emit NULL
    Enum,
    /// Anything else
    Unknown,
}

impl TypeKind {
    /// Parse the raw `data_type` string (exactly as returned by the DB driver /
    /// information_schema) into a TypeKind.
    ///
    /// We normalise to lowercase and match on substrings so that both short
    /// aliases ("int", "varchar") and full PG names ("character varying",
    /// "timestamp with time zone") are handled.
    fn from_data_type(raw: &str) -> Self {
        let dt = raw.trim().to_lowercase();
        let dt = dt.as_str();

        // ── timestamp (must check BEFORE generic "time" and "char") ──────────
        // "timestamp with time zone" / "timestamptz"
        if dt.contains("timestamp") {
            return if dt.contains("with time zone") || dt == "timestamptz" {
                TypeKind::TimestampTz
            } else {
                // "timestamp without time zone" / bare "timestamp"
                TypeKind::Timestamp
            };
        }

        // ── date ─────────────────────────────────────────────────────────────
        if dt == "date" {
            return TypeKind::Date;
        }

        // ── time (without date component) ────────────────────────────────────
        if dt == "time"
            || dt == "time without time zone"
            || dt == "time with time zone"
            || dt == "timetz"
        {
            return TypeKind::Time;
        }

        // ── year (MySQL only) ────────────────────────────────────────────────
        if dt == "year" {
            return TypeKind::Year;
        }

        // ── interval ─────────────────────────────────────────────────────────
        if dt.contains("interval") {
            return TypeKind::Interval;
        }

        // ── uuid ─────────────────────────────────────────────────────────────
        if dt == "uuid" {
            return TypeKind::Uuid;
        }

        // ── json / jsonb ─────────────────────────────────────────────────────
        if dt == "json" || dt == "jsonb" {
            return TypeKind::Json;
        }

        // ── boolean ──────────────────────────────────────────────────────────
        if dt == "boolean" || dt == "bool" || dt == "bit" {
            return TypeKind::Boolean;
        }

        // ── integer family ───────────────────────────────────────────────────
        // "serial" / "bigserial" / "smallserial" are auto-increment in PG;
        // drivers still report them as a type.  We map them to their int size
        // so FK pools get the right scale.
        let int_max = match dt {
            "tinyint" | "int1" => Some(127_i64),
            "smallint" | "int2" | "smallserial" => Some(32_767),
            "mediumint" | "int3" => Some(8_388_607),
            "int" | "int4" | "integer" | "serial" => Some(2_147_483_647),
            "bigint" | "int8" | "bigserial" | "unsigned bigint" => Some(9_007_199_254_740_991),
            _ => None,
        };
        if let Some(max) = int_max {
            return TypeKind::Integer { max };
        }
        // Catch "int" substrings that weren't in the table above
        // (e.g. DB-specific variants like "integer unsigned")
        if dt.contains("int") && !dt.contains("point") {
            return TypeKind::Integer { max: 2_147_483_647 };
        }

        // ── float / double / real ────────────────────────────────────────────
        if dt == "float"
            || dt == "float4"
            || dt == "float8"
            || dt == "double"
            || dt == "double precision"
            || dt == "real"
        {
            return TypeKind::Float;
        }

        // ── decimal / numeric / money ─────────────────────────────────────────
        if dt.contains("decimal")
            || dt.contains("numeric")
            || dt.contains("money")
            || dt.contains("currency")
        {
            // scale may be embedded in the raw string if the driver does not
            // split it out; fall back to ColumnSchema.numeric_scale later.
            return TypeKind::Decimal { scale: 2 }; // refined in generate()
        }

        // ── text family ───────────────────────────────────────────────────────
        // "character varying" (PG full name), "character", "varchar", "char",
        // "text", "tinytext", "mediumtext", "longtext", "clob", "citext", "name"
        if dt == "character varying"
            || dt == "character"
            || dt.contains("varchar")
            || dt.contains("char")   // catches char, nchar, bpchar …
            || dt.contains("text")
            || dt == "clob"
            || dt == "citext"
            || dt == "name"
        {
            return TypeKind::Text { max_len: 64 }; // refined in generate()
        }

        // ── binary ───────────────────────────────────────────────────────────
        if dt == "bytea" || dt.contains("blob") || dt.contains("binary") || dt.contains("bytes") {
            return TypeKind::Binary;
        }

        // ── enum / set ───────────────────────────────────────────────────────
        if dt.starts_with("enum") || dt.starts_with("set") {
            return TypeKind::Enum;
        }

        TypeKind::Unknown
    }

    /// Generate a SQL literal for this type, optionally refined by the column
    /// name as a semantic hint.
    fn generate(self, col: &ColumnSchema) -> String {
        match self {
            // ── integers ──────────────────────────────────────────────────────
            TypeKind::Integer { max } => {
                // Semantic refinement: counts should be small non-negative
                let n = col.name.to_lowercase();
                let upper = if ends_with_any(
                    &n,
                    &[
                        "_count",
                        "_num",
                        "_number",
                        "_qty",
                        "_quantity",
                        "_total",
                        "_rank",
                        "_score",
                        "_index",
                        "_size",
                        "_priority",
                        "_order",
                        "_seq",
                    ],
                ) || matches!(
                    n.as_str(),
                    "count" | "quantity" | "total" | "score" | "rank" | "age"
                ) {
                    1_000_i64
                } else {
                    max.min(1_000_000)
                };
                rand::thread_rng().gen_range(1..=upper).to_string()
            }

            // ── floats ────────────────────────────────────────────────────────
            TypeKind::Float => {
                let v: f64 = rand::thread_rng().gen_range(0.0_f64..9_999.0);
                format!("{:.4}", v)
            }

            // ── decimal ───────────────────────────────────────────────────────
            TypeKind::Decimal { .. } => {
                let scale = col.numeric_scale.unwrap_or(2).max(0) as usize;
                let prec = col.numeric_precision.unwrap_or(10).max(scale as i32 + 1) as usize;
                let int_digits = prec.saturating_sub(scale).min(8);
                let max_int = 10_u64.pow(int_digits as u32).saturating_sub(1).min(999_999);
                let int_part: u64 = rand::thread_rng().gen_range(0..=max_int);
                if scale == 0 {
                    int_part.to_string()
                } else {
                    let frac_max = 10_u64.pow(scale.min(18) as u32);
                    let frac: u64 = rand::thread_rng().gen_range(0..frac_max);
                    format!("{}.{:0>width$}", int_part, frac, width = scale)
                }
            }

            // ── boolean ───────────────────────────────────────────────────────
            TypeKind::Boolean => gen_bool(),

            // ── uuid ──────────────────────────────────────────────────────────
            TypeKind::Uuid => sql_str(Uuid::new_v4().to_string()),

            // ── json / jsonb ──────────────────────────────────────────────────
            TypeKind::Json => gen_json(),

            // ── timestamp with time zone ──────────────────────────────────────
            TypeKind::TimestampTz => gen_timestamptz(),

            // ── timestamp without time zone ───────────────────────────────────
            TypeKind::Timestamp => gen_timestamp(),

            // ── date ──────────────────────────────────────────────────────────
            TypeKind::Date => gen_date(),

            // ── time ──────────────────────────────────────────────────────────
            TypeKind::Time => gen_time(),

            // ── year ──────────────────────────────────────────────────────────
            TypeKind::Year => rand::thread_rng().gen_range(2000_u16..=2030).to_string(),

            // ── interval ──────────────────────────────────────────────────────
            TypeKind::Interval => {
                let h: u32 = rand::thread_rng().gen_range(0..=8_760);
                let m: u32 = rand::thread_rng().gen_range(0..=59);
                sql_str(format!("{:02}:{:02}:00", h, m))
            }

            // ── text family ───────────────────────────────────────────────────
            TypeKind::Text { .. } => {
                let max = col.max_length.unwrap_or(64).max(1).min(512) as usize;
                gen_text_for_col(col, max)
            }

            // ── binary → NULL (can't guess bytes) ────────────────────────────
            TypeKind::Binary => "NULL".to_string(),

            // ── enum / set → NULL (valid members unknown) ─────────────────────
            TypeKind::Enum => "NULL".to_string(),

            // ── unknown → short alphanum ──────────────────────────────────────
            TypeKind::Unknown => sql_str(gen_alphanum(8)),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Text-column semantic refinement
// Inspects the column *name* to pick a meaningful generator,
// but only within the text type class — never cross-type.
// ─────────────────────────────────────────────────────────────────────────────

fn gen_text_for_col(col: &ColumnSchema, max_len: usize) -> String {
    let n = col.name.to_lowercase();

    // ── email ────────────────────────────────────────────────────────────────
    if contains_any(&n, &["email", "e_mail"]) {
        return sql_str(unique_email());
        // let user = gen_alphanum(rand::thread_rng().gen_range(4..=10));
        // let tag: u32 = rand::thread_rng().gen_range(10..=999);
        // let domain = gen_alphanum(rand::thread_rng().gen_range(4..=9));
        // return sql_str(format!("{}.{}@{}.{}", user, tag, domain, pick_tld()));
    }
    if contains_any(&n, &["avatar"]) {
        return sql_str(random_avatar_url());
    }
    if contains_any(&n, &["image", "cover"]) {
        return sql_str(random_image_url());
    }

    // ── url / website ────────────────────────────────────────────────────────
    if contains_any(&n, &["url", "uri", "website", "homepage", "href"]) {
        let host = gen_alphanum(rand::thread_rng().gen_range(5..=12));
        let path = gen_alphanum(rand::thread_rng().gen_range(3..=8));
        return sql_str(format!("https://www.{}.{}/{}", host, pick_tld(), path));
    }

    // ── uuid stored as text ──────────────────────────────────────────────────
    if contains_any(&n, &["uuid", "guid"]) {
        return sql_str(Uuid::new_v4().to_string());
    }

    // ── phone ────────────────────────────────────────────────────────────────
    if contains_any(&n, &["phone", "mobile", "cell", "fax"]) {
        return sql_str(unique_phone_number());
        // let mut rng = rand::thread_rng();
        // let cc: u16 = rng.gen_range(1..=99);
        // let sub: u64 = rng.gen_range(100_000_000..=9_999_999_999);
        // return sql_str(format!("+{}{}", cc, sub));
    }

    // ── hashed credential  (password / secret / hash — NOT token/jti which are random strings) ──
    if contains_any(&n, &["password", "passwd", "pwd"]) {
        let rounds: u8 = rand::thread_rng().gen_range(10..=13);
        let salt = gen_base64url(22);
        let hash = gen_base64url(31);
        return sql_str(format!("$2b${:02}${}{}", rounds, salt, hash));
    }

    // ── IP address ───────────────────────────────────────────────────────────
    if n == "ip"
        || n.ends_with("_ip")
        || n.contains("_ip_")
        || n == "upload_ip"
        || n == "operator_ip"
        || n == "reporter_ip"
    {
        let mut rng = rand::thread_rng();
        return sql_str(format!(
            "{}.{}.{}.{}",
            rng.gen_range(1_u8..=254),
            rng.gen_range(0_u8..=255),
            rng.gen_range(0_u8..=255),
            rng.gen_range(1_u8..=254),
        ));
    }

    // ── slug / code / identifier ─────────────────────────────────────────────
    if contains_any(&n, &["slug", "code"]) {
        let len = rand::thread_rng().gen_range(4..=12).min(max_len);
        return sql_str(gen_slug(len));
    }

    // ── version string ───────────────────────────────────────────────────────
    if n == "version" || n.ends_with("_version") {
        let mut rng = rand::thread_rng();
        return sql_str(format!(
            "{}.{}.{}",
            rng.gen_range(0..=5),
            rng.gen_range(0..=20),
            rng.gen_range(0..=99)
        ));
    }

    // ── color / colour ───────────────────────────────────────────────────────
    if n == "color" || n == "colour" || n.ends_with("_color") || n.ends_with("_colour") {
        let mut rng = rand::thread_rng();
        return sql_str(format!("#{:06X}", rng.gen_range(0_u32..=0xFF_FF_FF)));
    }

    // ── user-agent string ────────────────────────────────────────────────────
    if contains_any(&n, &["user_agent", "useragent", "agent"]) {
        return sql_str(format!(
            "Mozilla/5.0 (compatible; datamocker/{})",
            gen_alphanum(4)
        ));
    }

    if contains_any(&n, &["name", "username", "login"]) {
        return sql_str(unique_username());
    }

    // ── long prose: description / content / body / bio / summary / note ──────
    if contains_any(
        &n,
        &[
            "description",
            "content",
            "body",
            "bio",
            "biography",
            "summary",
            "detail",
            "note",
            "remark",
            "message",
            "about",
            "before",
            "after",
        ],
    ) {
        let word_count = rand::thread_rng().gen_range(8..=25_usize);
        let words: Vec<String> = (0..word_count)
            .map(|_| gen_word(rand::thread_rng().gen_range(3..=9)))
            .collect();
        let s = words.join(" ");
        let s = if s.len() > max_len {
            s[..max_len].to_string()
        } else {
            s
        };
        return sql_str(s);
    }

    // ── short label: title / subject / name / username / label / tag / role ──
    if contains_any(
        &n,
        &[
            "title",
            "subject",
            "headline",
            "caption",
            "label",
            "tag",
            "category",
            "role",
            "reason",
            "action",
            "type",
            "status",
            "state",
            "target_type",
            "trigger_type",
            "event_type",
            "timeline_type",
            "ptype",
        ],
    ) || matches!(n.as_str(), "v0" | "v1" | "v2" | "v3" | "v4" | "v5")
    {
        let word_count = rand::thread_rng().gen_range(1..=2_usize);
        let words: Vec<String> = (0..word_count)
            .map(|_| gen_capitalized_word(rand::thread_rng().gen_range(3..=8)))
            .collect();
        let s = words.join(" ");
        let s = if s.len() > max_len {
            s[..max_len].to_string()
        } else {
            s
        };
        return sql_str(s);
    }

    // ── token / jti / file_id / stored_name / stored_path / script_url etc. ─
    // These are opaque identifiers — use random alphanum of appropriate length.
    let len = rand::thread_rng().gen_range(8..=max_len.min(64)).max(1);
    sql_str(gen_alphanum(len))
}

// ─────────────────────────────────────────────────────────────────────────────
// Primitive generators — purely algorithmic, no embedded data
// ─────────────────────────────────────────────────────────────────────────────

/// PostgreSQL BOOLEAN literal.
fn gen_bool() -> String {
    if rand::thread_rng().gen_bool(0.5) {
        "true".to_string()
    } else {
        "false".to_string()
    }
}

/// JSON object with 1-4 algorithmically generated key/value pairs.
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

/// TIMESTAMP WITH TIME ZONE — PostgreSQL format with explicit UTC offset.
/// e.g. '2024-03-15 08:42:17+00'
fn gen_timestamptz() -> String {
    let (days, secs) = rand_offset();
    let ts = Utc::now() - Duration::days(days) - Duration::seconds(secs);
    sql_str(ts.format("%Y-%m-%d %H:%M:%S+00").to_string())
}

/// TIMESTAMP WITHOUT TIME ZONE — no offset suffix.
/// e.g. '2024-03-15 08:42:17'
fn gen_timestamp() -> String {
    let (days, secs) = rand_offset();
    let ts = Utc::now() - Duration::days(days) - Duration::seconds(secs);
    sql_str(ts.format("%Y-%m-%d %H:%M:%S").to_string())
}

/// DATE — 'YYYY-MM-DD'
fn gen_date() -> String {
    let days: i64 = rand::thread_rng().gen_range(0..=1_825);
    let d = (Utc::now() - Duration::days(days)).date_naive();
    sql_str(d.format("%Y-%m-%d").to_string())
}

/// TIME — 'HH:MM:SS'
fn gen_time() -> String {
    let mut rng = rand::thread_rng();
    sql_str(format!(
        "{:02}:{:02}:{:02}",
        rng.gen_range(0..=23_u8),
        rng.gen_range(0..=59_u8),
        rng.gen_range(0..=59_u8),
    ))
}

fn rand_offset() -> (i64, i64) {
    let mut rng = rand::thread_rng();
    (rng.gen_range(0..=730), rng.gen_range(0..=86_399))
}

/// Lowercase alphanum: a-z 0-9
fn gen_alphanum(len: usize) -> String {
    const C: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| C[rng.gen_range(0..C.len())] as char)
        .collect()
}

/// URL-safe base64 alphabet
fn gen_base64url(len: usize) -> String {
    const C: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| C[rng.gen_range(0..C.len())] as char)
        .collect()
}

/// Lowercase alpha-only word (a-z)
fn gen_word(len: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| rng.gen_range(b'a'..=b'z') as char)
        .collect()
}

/// Capitalized word (first char A-Z, rest a-z)
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

/// Slug: lowercase words joined with hyphens
fn gen_slug(max_len: usize) -> String {
    let word_count = rand::thread_rng().gen_range(2..=4_usize);
    let words: Vec<String> = (0..word_count)
        .map(|_| gen_word(rand::thread_rng().gen_range(3..=7)))
        .collect();
    let s = words.join("-");
    if s.len() > max_len {
        s[..max_len].to_string()
    } else {
        s
    }
}

/// One of five generic TLDs — chosen by index so no static slice needed at call site.
fn pick_tld() -> &'static str {
    const T: [&str; 5] = ["com", "org", "net", "io", "co"];
    T[rand::thread_rng().gen_range(0..T.len())]
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn sql_str(s: String) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn contains_any(hay: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| hay.contains(n))
}

fn ends_with_any(hay: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| hay.ends_with(n))
}
