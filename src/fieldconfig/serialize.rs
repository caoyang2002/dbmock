//! serialize.rs — YAML serialization with rich comments.
//!
//! serde_yaml produces clean YAML but has no comment support.
//! We post-process the output to inject:
//!   - A file-level header explaining all type options
//!   - Per-column comments showing the inferred description

use crate::core::schema::Schema;
use crate::errors::Result;
use crate::fieldconfig::infer::{infer_mock_config, MockConfig};
use std::path::Path;

/// Generate a mock_config.yml from a Schema and write it to `path`.
pub fn export_config(schema: &Schema, path: &Path) -> Result<()> {
    let config = infer_mock_config(schema);
    let yaml = render_config_yaml(&config, schema);
    std::fs::write(path, yaml)?;
    Ok(())
}

/// Load a MockConfig from a YAML file.
pub fn load_config(path: &Path) -> Result<MockConfig> {
    let content = std::fs::read_to_string(path)?;
    let config: MockConfig =
        serde_yaml::from_str(&content).map_err(|e| crate::errors::MockerError::Config {
            message: format!("Failed to parse mock config YAML: {}", e),
        })?;
    Ok(config)
}

/// Render a human-friendly YAML string with a header and per-column comments.
fn render_config_yaml(config: &MockConfig, schema: &Schema) -> String {
    let mut out = String::new();

    // ── file header ───────────────────────────────────────────────────────────
    out.push_str(FILE_HEADER);

    // ── per-table sections ────────────────────────────────────────────────────
    for (table_name, fields) in config {
        out.push_str(&format!("\n# ──────────────────────────────────────────\n"));
        out.push_str(&format!("# Table: {}\n", table_name));
        out.push_str(&format!("# ──────────────────────────────────────────\n"));
        out.push_str(&format!("{}:\n", table_name));

        // Get column order from schema so we emit columns in definition order.
        let ordered_cols: Vec<&str> = schema
            .get_table(table_name)
            .map(|ts| ts.columns.iter().map(|c| c.name.as_str()).collect())
            .unwrap_or_default();

        let emit_col = |out: &mut String, col_name: &str| {
            if let Some(fc) = fields.get(col_name) {
                let desc = fc.kind.description();
                out.push_str(&format!("  # {}\n", desc));
                out.push_str(&format!("  {}:\n", col_name));
                // Serialize the FieldConfig using serde_yaml, then indent it.
                let field_yaml = serde_yaml::to_string(fc).unwrap_or_default();
                // serde_yaml puts a leading "---\n"; strip it.
                let field_yaml = field_yaml.trim_start_matches("---\n");
                for line in field_yaml.lines() {
                    out.push_str(&format!("    {}\n", line));
                }
            }
        };

        // Emit columns in schema order first, then any extras not in schema.
        let mut emitted = std::collections::HashSet::new();
        for col_name in &ordered_cols {
            emit_col(&mut out, col_name);
            emitted.insert(*col_name);
        }
        // Extras (shouldn't happen normally, but be safe).
        for col_name in fields.keys() {
            if !emitted.contains(col_name.as_str()) {
                emit_col(&mut out, col_name);
            }
        }
    }

    out
}

// ── file header constant ─────────────────────────────────────────────────────

const FILE_HEADER: &str = r#"# =============================================================================
# dbmock — Mock Data Configuration
# =============================================================================
#
# This file controls how each column's mock data is generated.
# Edit the `type:` field for any column to change its strategy.
#
# AVAILABLE TYPES:
#
#   default      — automatic schema-driven inference (safe to leave as-is)
#   skip         — exclude this column from INSERT
#   null         — always emit NULL
#   constant     — always emit the value in `constant:`
#
#   uuid         — UUID v4 string
#   unique       — value unique within this run (counter-suffixed)
#   int          — random integer  [min:, max:]
#   float        — random float    [min:, max:]
#   decimal      — fixed-scale decimal [min:, max:, scale:]
#   sequence     — monotonically increasing integer from 1
#   bool         — true / false
#
#   string       — random alphanum string [min_len:, max_len:]
#   text         — random prose paragraph [max_len:]
#   label        — short 1-2 word capitalized label
#   slug         — lowercase hyphen-separated slug
#   enum         — random pick from values: [a, b, c]
#
#   email        — user@domain.tld
#   url          — https://www.domain.tld/path
#   ip           — IPv4 address
#   semver       — major.minor.patch
#   password     — bcrypt-shaped hash
#   phone        — +CC subscriber
#   color        — CSS hex color #RRGGBB
#   user_agent   — browser User-Agent string
#
#   timestamp_tz — TIMESTAMP WITH TIME ZONE [date_from:, date_to:]
#   timestamp    — TIMESTAMP WITHOUT TIME ZONE [date_from:, date_to:]
#   date         — DATE YYYY-MM-DD [date_from:, date_to:]
#   time         — TIME HH:MM:SS
#
#   json         — random JSON object
#
# OPTIONAL PARAMETERS (only those relevant to the chosen type are used):
#
#   values:    [a, b, c]      # for enum
#   min:       0              # for int / float / decimal
#   max:       1000           # for int / float / decimal
#   scale:     2              # for decimal (decimal places)
#   min_len:   4              # for string
#   max_len:   64             # for string / text
#   date_from: "2020-01-01"   # for timestamp / date
#   date_to:   "2025-12-31"   # for timestamp / date
#   null_rate: 0.15           # override NULL probability (0.0–1.0)
#   constant:  "fixed value"  # for constant type
#
# =============================================================================

"#;
