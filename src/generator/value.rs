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
        random_action, random_address, random_alphanum, random_api_path, random_article,
        random_article_title, random_assignment_status, random_avatar_url, random_bank_card,
        random_branch_name, random_brand, random_certificate_number, random_color,
        random_commit_hash, random_company, random_config_key, random_container_name,
        random_coupon_code, random_course_name, random_course_type, random_cron_expr,
        random_db_url, random_difficulty, random_docker_tag, random_environment, random_error_type,
        random_extension, random_file_hash, random_framework, random_grade_letter,
        random_http_method, random_image_url, random_ip, random_job, random_jti, random_library,
        random_license, random_log_level, random_mime_type, random_moderation_status,
        random_notification_type, random_order_status, random_password_hash, random_payment_method,
        random_port_string, random_post_status, random_post_type, random_product_name,
        random_programming_language, random_question_type, random_report_type, random_role,
        random_sku, random_slug, random_subject, random_tag_name, random_target_type,
        random_task_status, random_token, random_tracking_number, random_url, random_user_agent,
        random_version, random_violation_type, unique_email, unique_phone_number, unique_username,
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

    // ========== 1. 唯一性字段 ==========
    if contains_any(&n, &["email", "e_mail"]) {
        return sql_str(unique_email());
    }
    if contains_any(&n, &["phone", "mobile", "cell", "fax", "telephone"]) {
        return sql_str(unique_phone_number());
    }
    if contains_any(&n, &["name", "username", "login", "nickname", "fullname"]) {
        return sql_str(unique_username());
    }

    // ========== 2. 图片 / 头像 / 封面 ==========
    if contains_any(&n, &["avatar", "avatar_url"]) {
        return sql_str(random_avatar_url());
    }
    if contains_any(&n, &["image", "cover", "cover_url", "photo", "picture"]) {
        return sql_str(random_image_url());
    }
    if n == "icon" || n.ends_with("_icon") || n == "icon_url" {
        return sql_str(random_avatar_url());
    }
    if n == "screenshots" && col.data_type.to_lowercase().contains("json") {
        return sql_str(format!("[\"{}\"]", random_image_url()));
    }

    // ========== 3. URL / URI / 网站 ==========
    if contains_any(
        &n,
        &[
            "url",
            "uri",
            "website",
            "homepage",
            "href",
            "link",
            "homepage_url",
        ],
    ) {
        return sql_str(random_url());
    }
    if n == "script_url" || n == "evidence_url" {
        return sql_str(random_url());
    }

    // ========== 4. UUID / GUID ==========
    if contains_any(&n, &["uuid", "guid"]) {
        return sql_str(Uuid::new_v4().to_string());
    }

    // ========== 5. 密码哈希 ==========
    if contains_any(&n, &["password", "passwd", "pwd", "hash", "secret"]) {
        return sql_str(random_password_hash());
    }

    // ========== 6. IP 地址 ==========
    if n == "ip"
        || n.ends_with("_ip")
        || n.contains("_ip_")
        || n == "upload_ip"
        || n == "operator_ip"
        || n == "reporter_ip"
    {
        return sql_str(random_ip());
    }

    // ========== 7. Slug / Code / Identifier ==========
    if contains_any(&n, &["slug", "code", "identifier", "ident"]) {
        let slug = random_slug();
        let trimmed = if slug.len() > max_len {
            &slug[..max_len]
        } else {
            &slug
        };
        return sql_str(trimmed.to_string());
    }

    // ========== 8. 版本号 ==========
    if n == "version" || n.ends_with("_version") {
        return sql_str(random_version());
    }

    // ========== 9. 颜色 ==========
    if n == "color" || n == "colour" || n.ends_with("_color") || n.ends_with("_colour") {
        return sql_str(random_color());
    }

    // ========== 10. User-Agent ==========
    if contains_any(&n, &["user_agent", "useragent", "agent"]) {
        return sql_str(random_user_agent());
    }

    // ========== 11. 地址（国家/省/市/街道） ==========
    if contains_any(
        &n,
        &[
            "address", "addr", "location", "street", "city", "state", "country",
        ],
    ) {
        return sql_str(random_address());
    }

    // ========== 12. 公司 / 组织 ==========
    if contains_any(&n, &["company", "organization", "org", "corp", "firm"]) {
        return sql_str(random_company());
    }

    // ========== 13. 职位 ==========
    // 注意：去掉 "role"（由社区专用块处理），去掉 "title"（由短标签块处理）
    if contains_any(&n, &["job", "position", "occupation"]) {
        return sql_str(random_job());
    }

    // ========== 14. 银行卡号 ==========
    if contains_any(
        &n,
        &["bank_card", "credit_card", "card_number", "payment_method"],
    ) {
        return sql_str(random_bank_card());
    }

    // ========== 15. 文件相关字段 ==========
    if n == "file_id" || n == "stored_name" {
        return sql_str(random_alphanum(16));
    }
    if n == "original_name" {
        return sql_str(format!("{}.{}", random_alphanum(8), random_extension()));
    }
    if n == "stored_path" {
        return sql_str(format!(
            "/uploads/{}/{}",
            random_alphanum(6),
            random_alphanum(12)
        ));
    }
    if n == "file_type" {
        return sql_str(
            random_mime_type()
                .split('/')
                .next()
                .unwrap_or("application")
                .to_string(),
        );
    }
    if n == "mime_type" {
        return sql_str(random_mime_type());
    }
    if n == "mime_major" {
        return sql_str(
            random_mime_type()
                .split('/')
                .next()
                .unwrap_or("application")
                .to_string(),
        );
    }
    if n == "ext" || n == "extension" {
        return sql_str(random_extension());
    }
    if n == "file_hash" {
        return sql_str(random_file_hash());
    }

    // ========== 16. 令牌 / JTI ==========
    if n == "token" || n == "refresh_token" {
        return sql_str(random_token());
    }
    if n == "jti" {
        return sql_str(random_jti());
    }

    // ========== 17. Cron 表达式 ==========
    if n == "cron_expr" {
        return sql_str(random_cron_expr());
    }

    // ========== 18. 长文本（描述 / 内容 / 正文 / 备注等） ==========
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
            "comment",
            "review",
            "feedback",
            "content_snapshot",
            "event_detail",
            "error_msg",
            "script_code",
            "reason", // 只保留一次
            "appeal_reason",
            "appeal_result",
            "review_note",
            "handle_note",
            "config_schema",
            "config_values",
        ],
    ) {
        let article = random_article();
        let trimmed = if article.len() > max_len {
            &article[..max_len]
        } else {
            &article
        };
        return sql_str(trimmed.to_string());
    }

    // ========== 19. 短标签（标题 / 主题 / 类型 / 状态 / 动作等） ==========
    if contains_any(&n, &["title", "headline", "caption", "label", "category"])
        || matches!(n.as_str(), "v0" | "v1" | "v2" | "v3" | "v4" | "v5")
    {
        let short = random_article_title();
        let trimmed = if short.len() > max_len {
            &short[..max_len]
        } else {
            &short
        };
        return sql_str(trimmed.to_string());
    }
    // 去掉已在专用块处理的: "role", "subject", "tag", "level", "value"

    // ========== 电商专用字段 ==========
    if contains_any(
        &n,
        &["product_name", "product_title", "item_name", "goods_name"],
    ) {
        let name = random_product_name();
        let trimmed = if name.len() > max_len {
            &name[..max_len]
        } else {
            &name
        };
        return sql_str(trimmed.to_string());
    }
    if contains_any(&n, &["sku", "product_sku", "item_sku", "goods_id"]) {
        let sku = random_sku();
        let trimmed = if sku.len() > max_len {
            &sku[..max_len]
        } else {
            &sku
        };
        return sql_str(trimmed.to_string());
    }
    if contains_any(&n, &["brand", "brand_name", "manufacturer"]) {
        return sql_str(random_brand());
    }
    if contains_any(&n, &["order_status", "order_state", "delivery_status"]) {
        return sql_str(random_order_status());
    }
    if contains_any(&n, &["payment_method", "payment_type", "pay_type"]) {
        return sql_str(random_payment_method());
    }
    if contains_any(&n, &["tracking_number", "tracking_no", "shipment_number"]) {
        return sql_str(random_tracking_number());
    }
    if contains_any(&n, &["coupon_code", "promo_code", "discount_code"]) {
        return sql_str(random_coupon_code());
    }
    if contains_any(
        &n,
        &["shipping_address", "delivery_address", "billing_address"],
    ) {
        return sql_str(random_address());
    }
    if contains_any(&n, &["product_description", "specification", "details"]) {
        let article = random_article();
        let trimmed = if article.len() > max_len {
            &article[..max_len]
        } else {
            &article
        };
        return sql_str(trimmed.to_string());
    }
    if contains_any(&n, &["product_tags", "tags"]) && !col.data_type.to_lowercase().contains("json")
    {
        let tags = vec![random_article_title(), random_article_title()].join(",");
        let trimmed = if tags.len() > max_len {
            &tags[..max_len]
        } else {
            &tags
        };
        return sql_str(trimmed.to_string());
    }

    // ========== 社区专用字段 ==========
    if contains_any(&n, &["post_type", "topic_type"]) {
        return sql_str(random_post_type());
    }
    if contains_any(&n, &["post_status", "status"])
        && !contains_any(
            &n,
            &[
                "order_status",
                "payment_status",
                "task_status",
                "job_status",
                "build_status",
                "pipeline_status",
                "moderation_status",
                "review_status",
                "audit_status",
                "assignment_status",
                "submission_status",
                "delivery_status",
            ],
        )
    {
        return sql_str(random_post_status());
    }
    if contains_any(&n, &["moderation_status", "review_status", "audit_status"]) {
        return sql_str(random_moderation_status());
    }
    if contains_any(&n, &["report_type", "violation_type"]) {
        if col.data_type.to_lowercase().contains("varchar")
            || col.data_type.to_lowercase().contains("text")
        {
            return sql_str(random_report_type());
        }
    }
    if n == "action" || n.ends_with("_action") {
        return sql_str(random_action());
    }
    if contains_any(&n, &["target_type", "object_type"]) {
        return sql_str(random_target_type());
    }
    // "role" 统一在此处理（原第 13 节和此处合并）
    if n == "role" {
        return sql_str(random_role());
    }
    if contains_any(&n, &["notification_type", "notify_type"]) {
        return sql_str(random_notification_type());
    }
    if contains_any(&n, &["signature", "personal_note"]) {
        let short = random_article_title();
        let trimmed = if short.len() > max_len {
            &short[..max_len]
        } else {
            &short
        };
        return sql_str(trimmed.to_string());
    }
    if n == "profile_url" || n == "user_url" {
        return sql_str(random_url());
    }
    if contains_any(&n, &["group_name", "team_name", "circle_name"]) {
        let name = random_company();
        let trimmed = if name.len() > max_len {
            &name[..max_len]
        } else {
            &name
        };
        return sql_str(trimmed.to_string());
    }
    if contains_any(&n, &["group_description", "group_bio"]) {
        let desc = random_article();
        let trimmed = if desc.len() > max_len {
            &desc[..max_len]
        } else {
            &desc
        };
        return sql_str(trimmed.to_string());
    }

    // ========== 教育专用字段 ==========
    if contains_any(&n, &["course_name", "course_title", "class_name"]) {
        let name = random_course_name();
        let trimmed = if name.len() > max_len {
            &name[..max_len]
        } else {
            &name
        };
        return sql_str(trimmed.to_string());
    }
    // "subject" 统一在此处理（从第 19 节移出）
    if contains_any(&n, &["subject", "discipline", "major"]) {
        let subj = random_subject();
        let trimmed = if subj.len() > max_len {
            &subj[..max_len]
        } else {
            &subj
        };
        return sql_str(trimmed.to_string());
    }
    // "level" / "difficulty" 统一在此处理（从第 19 节移出）
    if contains_any(&n, &["difficulty", "grade_level"]) || n == "level" {
        return sql_str(random_difficulty());
    }
    if contains_any(&n, &["course_type", "class_type"]) {
        return sql_str(random_course_type());
    }
    if contains_any(&n, &["question_type", "quiz_type"]) {
        return sql_str(random_question_type());
    }
    if contains_any(&n, &["grade_letter", "score_letter"]) {
        return sql_str(random_grade_letter());
    }
    if contains_any(&n, &["certificate_number", "cert_no", "diploma_no"]) {
        let cert = random_certificate_number();
        let trimmed = if cert.len() > max_len {
            &cert[..max_len]
        } else {
            &cert
        };
        return sql_str(trimmed.to_string());
    }
    if contains_any(&n, &["assignment_status", "submission_status"]) {
        return sql_str(random_assignment_status());
    }
    if contains_any(
        &n,
        &["course_description", "syllabus", "learning_objectives"],
    ) {
        let desc = random_article();
        let trimmed = if desc.len() > max_len {
            &desc[..max_len]
        } else {
            &desc
        };
        return sql_str(trimmed.to_string());
    }
    if n == "course_cover" || n == "thumbnail_url" {
        return sql_str(random_image_url());
    }
    if contains_any(&n, &["instructor_bio", "teacher_intro"]) {
        let bio = random_article_title();
        let trimmed = if bio.len() > max_len {
            &bio[..max_len]
        } else {
            &bio
        };
        return sql_str(trimmed.to_string());
    }
    if contains_any(&n, &["student_feedback", "review_comment"]) {
        let feedback = random_article();
        let trimmed = if feedback.len() > max_len {
            &feedback[..max_len]
        } else {
            &feedback
        };
        return sql_str(trimmed.to_string());
    }

    // ========== 开发/技术专用字段 ==========
    if contains_any(&n, &["language", "lang", "programming_language"]) {
        return sql_str(random_programming_language());
    }
    if contains_any(&n, &["framework", "library", "sdk", "toolchain"]) {
        return sql_str(random_framework());
    }
    if contains_any(&n, &["api_path", "endpoint", "route", "url_path", "path"]) {
        return sql_str(random_api_path());
    }
    if contains_any(&n, &["http_method", "method", "request_method"]) {
        return sql_str(random_http_method());
    }
    if contains_any(&n, &["environment", "env", "deploy_env"]) {
        return sql_str(random_environment());
    }
    if contains_any(&n, &["config_key", "setting_key", "property_key"]) {
        return sql_str(random_config_key());
    }
    if contains_any(&n, &["commit_hash", "git_hash", "revision", "short_sha"]) {
        return sql_str(random_commit_hash());
    }
    if contains_any(&n, &["branch", "git_branch"]) {
        return sql_str(random_branch_name());
    }
    // "tag" 统一在此处理（从第 19 节移出），精确匹配避免误伤 "product_tags"
    if contains_any(&n, &["git_tag", "release_tag"]) || n == "tag" {
        return sql_str(random_tag_name());
    }
    // "log_level" / "severity" 统一在此处理（"level" 已由教育块处理，此处用精确词）
    if contains_any(&n, &["log_level", "severity"]) {
        return sql_str(random_log_level());
    }
    if contains_any(&n, &["error_type", "exception_type", "failure_type"]) {
        return sql_str(random_error_type());
    }
    if contains_any(
        &n,
        &[
            "task_status",
            "job_status",
            "build_status",
            "pipeline_status",
        ],
    ) {
        return sql_str(random_task_status());
    }
    if contains_any(&n, &["docker_tag", "image_tag"]) {
        return sql_str(random_docker_tag());
    }
    if contains_any(&n, &["container_name", "pod_name", "instance_name"]) {
        return sql_str(random_container_name());
    }
    if contains_any(&n, &["db_url", "connection_string", "dsn"]) {
        return sql_str(random_db_url());
    }
    if n == "port" || n.ends_with("_port") {
        return sql_str(random_port_string());
    }
    if contains_any(&n, &["license", "license_type", "spdx_id"]) {
        return sql_str(random_license());
    }

    // ========== 20. 默认：随机字母数字字符串 ==========
    let len = rand::thread_rng().gen_range(8..=max_len.min(64)).max(1);
    sql_str(random_alphanum(len))
} // ─────────────────────────────────────────────────────────────────────────────
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
