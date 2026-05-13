//! types.rs — FieldKind: the complete set of user-configurable generation strategies.
//!
//! Every variant maps directly to a YAML `type:` value the user can write.
//! The serialization names are lowercase strings matching the YAML.

use serde::{Deserialize, Serialize};

/// A single column's generation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldConfig {
    /// The generation strategy.
    #[serde(rename = "type")]
    pub kind: FieldKind,

    /// For `enum` and `set`: the allowed values to pick from randomly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,

    /// For `int` / `float` / `decimal`: lower bound (inclusive). Stored as f64.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,

    /// For `int` / `float` / `decimal`: upper bound (inclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,

    /// For `decimal`: number of decimal places.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<usize>,

    /// For `string` / `text`: minimum character length.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_len: Option<usize>,

    /// For `string` / `text`: maximum character length.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_len: Option<usize>,

    /// For `regex`: the pattern to generate from (best-effort alphanum expansion).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,

    /// For `timestamp` / `date`: earliest date (YYYY-MM-DD).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_from: Option<String>,

    /// For `timestamp` / `date`: latest date (YYYY-MM-DD).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_to: Option<String>,

    /// Override the NULL probability (0.0–1.0). Defaults to 0.15 for nullable cols.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub null_rate: Option<f64>,

    /// A constant literal value — always emits exactly this string (SQL-escaped).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constant: Option<String>,
}

/// All supported generation strategies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    // ── meta ────────────────────────────────────────────────────────────────
    /// Use the automatic schema-driven inference (the default for every column).
    Default,
    /// Skip this column entirely (don't include it in INSERT).
    Skip,
    /// Always emit NULL.
    Null,
    /// Always emit the literal in `constant`.
    Constant,

    // ── identity / keys ──────────────────────────────────────────────────────
    /// Generate a globally-unique UUID v4 string.
    Uuid,
    /// Guarantee uniqueness within this run (counter-based suffix).
    Unique,

    // ── numbers ──────────────────────────────────────────────────────────────
    /// Random integer. Respects `min` / `max`.
    Int,
    /// Random float. Respects `min` / `max`.
    Float,
    /// Fixed-scale decimal. Respects `min` / `max` / `scale`.
    Decimal,
    /// Auto-increment counter starting at 1. Not stored in DB (skip if PK).
    Sequence,

    // ── booleans ─────────────────────────────────────────────────────────────
    /// true / false (50 % each, or use `values: [true]` to pin).
    Bool,

    // ── text ─────────────────────────────────────────────────────────────────
    /// Generic random alphanumeric string. Respects `min_len` / `max_len`.
    String,
    /// Multi-word prose paragraph. Respects `max_len`.
    Text,
    /// Short capitalized label (1–2 words).
    Label,
    /// Lowercase hyphen-separated slug.
    Slug,
    /// Semantic enum: pick a random value from `values`.
    Enum,

    // ── internet ─────────────────────────────────────────────────────────────
    /// user@domain.tld
    Email,
    /// https://www.domain.tld/path
    Url,
    /// IPv4 address (dotted decimal).
    Ip,
    /// Semantic version  major.minor.patch.
    Semver,

    // ── personal ─────────────────────────────────────────────────────────────
    /// bcrypt-shaped hash string.
    Password,
    /// +CC subscriber number.
    Phone,
    /// CSS hex color #RRGGBB.
    Color,
    /// User-agent header string.
    UserAgent,

    // ── date / time ──────────────────────────────────────────────────────────
    /// TIMESTAMP WITH TIME ZONE — 'YYYY-MM-DD HH:MM:SS+00'. Respects `date_from`/`date_to`.
    TimestampTz,
    /// TIMESTAMP WITHOUT TIME ZONE. Respects `date_from`/`date_to`.
    Timestamp,
    /// DATE 'YYYY-MM-DD'. Respects `date_from`/`date_to`.
    Date,
    /// TIME 'HH:MM:SS'.
    Time,

    // ── structured ───────────────────────────────────────────────────────────
    /// Random JSON object literal.
    Json,
}

impl FieldKind {
    /// Human-readable description shown in the exported config comment.
    pub fn description(&self) -> &'static str {
        match self {
            FieldKind::Default    => "schema-driven automatic inference (safe to leave as-is)",
            FieldKind::Skip       => "exclude column from INSERT",
            FieldKind::Null       => "always NULL",
            FieldKind::Constant   => "always emit the value in `constant:`",
            FieldKind::Uuid       => "UUID v4 string",
            FieldKind::Unique     => "unique value (counter-suffixed to guarantee uniqueness)",
            FieldKind::Int        => "random integer [min, max]",
            FieldKind::Float      => "random float [min, max]",
            FieldKind::Decimal    => "fixed-scale decimal [min, max], digits set by `scale:`",
            FieldKind::Sequence   => "monotonically increasing counter from 1",
            FieldKind::Bool       => "true or false",
            FieldKind::String     => "random alphanumeric string [min_len, max_len]",
            FieldKind::Text       => "random prose paragraph, up to max_len chars",
            FieldKind::Label      => "short 1-2 word capitalized label",
            FieldKind::Slug       => "lowercase hyphen-separated slug",
            FieldKind::Enum       => "random pick from `values:` list",
            FieldKind::Email      => "user@domain.tld",
            FieldKind::Url        => "https://www.domain.tld/path",
            FieldKind::Ip         => "IPv4 address",
            FieldKind::Semver     => "semantic version major.minor.patch",
            FieldKind::Password   => "bcrypt-shaped hash string",
            FieldKind::Phone      => "+CC subscriber-number",
            FieldKind::Color      => "CSS hex color #RRGGBB",
            FieldKind::UserAgent  => "browser User-Agent string",
            FieldKind::TimestampTz => "timestamp with timezone [date_from, date_to]",
            FieldKind::Timestamp  => "timestamp without timezone [date_from, date_to]",
            FieldKind::Date       => "date YYYY-MM-DD [date_from, date_to]",
            FieldKind::Time       => "time HH:MM:SS",
            FieldKind::Json       => "random JSON object",
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