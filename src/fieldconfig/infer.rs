//! infer.rs — 根据 ColumnSchema 自动推断 FieldKind。
//!
//! 该模块实现与 value.rs 相同的语义逻辑，但用于生成 FieldKind 赋值，
//! 而非直接生成值。它会生成初始的 `mock_config.yml` 供用户后续编辑。

use crate::core::schema::{ColumnSchema, Schema, TableSchema};
use crate::fieldconfig::types::{FieldConfig, FieldKind};
use std::collections::BTreeMap;

/// 表级别的配置：列名 → FieldConfig
pub type TableFieldConfig = BTreeMap<String, FieldConfig>;

/// 完整的 mock 配置：表名 → TableFieldConfig
pub type MockConfig = BTreeMap<String, TableFieldConfig>;

/// 从 Schema 推断完整的 MockConfig。
/// 每个列都会获得一个带有合理推断 kind 的 FieldConfig。
/// 自增主键会被标记为 Skip（由数据库自己赋值）。
pub fn infer_mock_config(schema: &Schema) -> MockConfig {
    let mut config = MockConfig::new();
    for table in &schema.tables {
        config.insert(table.name.clone(), infer_table_config(table));
    }
    config
}

/// 推断单个表的配置。
pub fn infer_table_config(table: &TableSchema) -> TableFieldConfig {
    let mut map = TableFieldConfig::new();
    for col in &table.columns {
        map.insert(col.name.clone(), infer_col_config(col, table));
    }
    map
}

/// 推断单个列的配置。
fn infer_col_config(col: &ColumnSchema, table: &TableSchema) -> FieldConfig {
    // 自增主键：跳过（由数据库赋值）
    if col.is_auto_increment {
        return FieldConfig { kind: FieldKind::Skip, ..Default::default() };
    }

    // 自引用外键（parent_id → 同一张表）：若可为空则直接生成 null
    let is_self_ref = table.foreign_keys.iter()
        .any(|fk| fk.column == col.name && fk.referenced_table == table.name);
    if is_self_ref && col.is_nullable {
        return FieldConfig { kind: FieldKind::Null, ..Default::default() };
    }

    // 外键列：使用 Default（由引擎处理外键池）
    let is_fk = table.foreign_keys.iter().any(|fk| fk.column == col.name);
    if is_fk {
        return FieldConfig { kind: FieldKind::Default, ..Default::default() };
    }

    // 根据数据类型 + 列名推断 kind
    let kind = infer_kind_from_col(col);

    // 根据推断的 kind 构建带有合理默认值的 FieldConfig
    build_config(kind, col)
}

fn infer_kind_from_col(col: &ColumnSchema) -> FieldKind {
    let dt = col.data_type.to_lowercase();
    let n  = col.name.to_lowercase();

    // ── 优先根据数据类型判断 ─────────────────────────────────────────────────────
    if dt == "uuid"                                         { return FieldKind::Uuid; }
    if dt == "boolean" || dt == "bool" || dt == "bit"      { return FieldKind::Bool; }
    if dt == "json"    || dt == "jsonb"                     { return FieldKind::Json; }
    if dt.contains("timestamp") {
        return if dt.contains("with time zone") || dt == "timestamptz" {
            FieldKind::TimestampTz
        } else {
            FieldKind::Timestamp
        };
    }
    if dt == "date"                                         { return FieldKind::Date; }
    if dt == "time" || dt.starts_with("time ")             { return FieldKind::Time; }
    if is_int_type(&dt)                                     { return infer_int_by_name(&n); }
    if is_float_type(&dt)                                   { return FieldKind::Float; }
    if is_decimal_type(&dt)                                 { return FieldKind::Decimal; }

    // ── 文本类型：根据列名进一步细化 ───────────────────────────────────────────
    if is_text_type(&dt) {
        return infer_text_by_name(&n);
    }

    FieldKind::Default
}

fn infer_int_by_name(n: &str) -> FieldKind {
    if ends_with_any(n, &["_count","_num","_number","_qty","_total","_rank","_score","_size","_order","_seq"]) {
        return FieldKind::Int; // 小范围非负整数
    }
    FieldKind::Int
}

fn infer_text_by_name(n: &str) -> FieldKind {
    if contains_any(n, &["email","e_mail"])                                  { return FieldKind::Email; }
    if contains_any(n, &["url","uri","website","homepage","href","avatar"])  { return FieldKind::Url; }
    if contains_any(n, &["uuid","guid"])                                     { return FieldKind::Uuid; }
    if contains_any(n, &["phone","mobile","cell","fax"])                     { return FieldKind::Phone; }
    if contains_any(n, &["password","passwd","pwd"])                         { return FieldKind::Password; }
    if n == "ip" || n.ends_with("_ip") || contains_any(n, &["_ip_"])        { return FieldKind::Ip; }
    if contains_any(n, &["slug"])                                            { return FieldKind::Slug; }
    if n == "version" || n.ends_with("_version")                            { return FieldKind::Semver; }
    if n == "color" || n == "colour" || n.ends_with("_color")               { return FieldKind::Color; }
    if contains_any(n, &["user_agent","useragent"])                          { return FieldKind::UserAgent; }
    if contains_any(n, &["description","content","body","bio","biography","summary","detail","note","remark","message","about"]) {
        return FieldKind::Text;
    }
    if contains_any(n, &["username","login"])                                { return FieldKind::Unique; }
    if contains_any(n, &["title","subject","headline","name","label","tag","category","role","status","state","type","action","reason"]) {
        return FieldKind::Label;
    }
    FieldKind::String
}

fn build_config(kind: FieldKind, col: &ColumnSchema) -> FieldConfig {
    let null_rate = if col.is_nullable { Some(0.15) } else { None };
    let max_len   = col.max_length.map(|v| v as usize);

    match &kind {
        FieldKind::Int => FieldConfig {
            kind,
            min: Some(0.0),
            max: Some(int_max_for_type(&col.data_type) as f64),
            null_rate,
            ..Default::default()
        },
        FieldKind::Float => FieldConfig {
            kind,
            min: Some(0.0),
            max: Some(9999.99),
            null_rate,
            ..Default::default()
        },
        FieldKind::Decimal => FieldConfig {
            kind,
            min: Some(0.0),
            max: Some(999999.0),
            scale: col.numeric_scale.map(|s| s as usize).or(Some(2)),
            null_rate,
            ..Default::default()
        },
        FieldKind::String => FieldConfig {
            kind,
            min_len: Some(4),
            max_len: max_len.or(Some(32)),
            null_rate,
            ..Default::default()
        },
        FieldKind::Text => FieldConfig {
            kind,
            max_len: max_len.or(Some(500)),
            null_rate,
            ..Default::default()
        },
        FieldKind::Enum => FieldConfig {
            kind,
            values: Some(vec!["value1".to_string(), "value2".to_string(), "value3".to_string()]),
            null_rate,
            ..Default::default()
        },
        FieldKind::TimestampTz | FieldKind::Timestamp | FieldKind::Date => FieldConfig {
            kind,
            date_from: Some("2020-01-01".to_string()),
            date_to:   Some("2025-12-31".to_string()),
            null_rate,
            ..Default::default()
        },
        _ => FieldConfig {
            kind,
            null_rate,
            ..Default::default()
        },
    }
}

// ── 类型判断辅助函数 ─────────────────────────────────────────────────────────

fn is_int_type(dt: &str) -> bool {
    matches!(dt,
        "smallint"|"int2"|"int"|"int4"|"integer"|"bigint"|"int8"|
        "tinyint"|"mediumint"|"serial"|"bigserial"|"smallserial"
    ) || (dt.contains("int") && !dt.contains("point"))
}

fn is_float_type(dt: &str) -> bool {
    matches!(dt, "float"|"float4"|"float8"|"double"|"double precision"|"real")
}

fn is_decimal_type(dt: &str) -> bool {
    dt.contains("decimal") || dt.contains("numeric") || dt.contains("money")
}

fn is_text_type(dt: &str) -> bool {
    dt == "character varying" || dt == "character" ||
        dt.contains("varchar") || dt.contains("char") || dt.contains("text") ||
        matches!(dt, "clob"|"citext"|"name")
}

fn int_max_for_type(dt: &str) -> i64 {
    let dt = dt.to_lowercase();
    match dt.as_str() {
        "tinyint" | "int1"             => 127,
        "smallint" | "int2"            => 32_767,
        "mediumint"                    => 8_388_607,
        "int" | "int4" | "integer"     => 2_147_483_647,
        _                              => 9_007_199_254_740_991,
    }
}

fn contains_any(hay: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| hay.contains(n))
}

fn ends_with_any(hay: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| hay.ends_with(n))
}