//! 数据生成器实现（唯一值 + 随机值）
//!
//! 唯一生成器内部使用 `HashSet` + 随机字符串保证不重复，碰撞时使用 UUID 回退。
//! 随机生成器直接返回随机值（允许重复）。
//!
//! # 线程安全
//! 所有全局状态均通过 `Mutex` 保护，无竞态条件。

use fake::faker::address::zh_cn::{CityName, CountryName, StateName, StreetName};
use fake::faker::company::zh_cn::CompanyName;
use fake::faker::creditcard::zh_cn::CreditCardNumber;
use fake::faker::internet::en::SafeEmail;
use fake::faker::internet::zh_tw::Username;
use fake::faker::job::zh_cn::{Seniority, Title};
use fake::faker::name::zh_cn::Name;
use fake::faker::phone_number::zh_cn::PhoneNumber;
use fake::{Fake, Faker};
use once_cell::sync::Lazy;
use rand::{random, Rng};
use std::collections::HashSet;
use std::sync::Mutex;
use uuid::Uuid;

use super::unique::UniqueGenerator;

// -----------------------------------------------------------------------------
// 全局唯一生成器
// -----------------------------------------------------------------------------

static UNIQUE_PHONE_GEN: Lazy<Mutex<UniqueGenerator<String>>> =
    Lazy::new(|| Mutex::new(UniqueGenerator::new()));

static USED_USERNAMES: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));
static USED_EMAILS: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

// -----------------------------------------------------------------------------
// 内部辅助函数
// -----------------------------------------------------------------------------

/// 生成随机字母数字字符串（小写）
fn random_string(len: usize) -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}

/// 生成随机 Base64URL 字符串（模拟 bcrypt salt/hash）
fn random_base64url(len: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}

// -----------------------------------------------------------------------------
// 唯一生成器（对外保证不重复）
// -----------------------------------------------------------------------------

/// 生成唯一的用户名
///
/// 使用 `fake` 库生成候选值，碰撞时附加 UUID 短串直到唯一。
pub fn unique_username() -> String {
    let mut used = USED_USERNAMES.lock().unwrap();
    // 先尝试 fake 库生成的用户名
    let candidate: String = Username().fake();
    if used.insert(candidate.clone()) {
        return candidate;
    }
    // 碰撞：将 fake 用户名 + UUID 前6位拼接，保证唯一
    loop {
        let suffix = &Uuid::new_v4().simple().to_string()[..6];
        let fallback = format!("{}_{}", candidate, suffix);
        if used.insert(fallback.clone()) {
            return fallback;
        }
        // 极端情况下继续重试（概率可忽略）
    }
}

/// 生成唯一的邮箱（12位随机字符 + @example.com，碰撞时使用 UUID）
pub fn unique_email() -> String {
    let mut used = USED_EMAILS.lock().unwrap();
    let local = random_string(12);
    let candidate = format!("{}@example.com", local);
    if used.insert(candidate.clone()) {
        return candidate;
    }
    // 碰撞：使用完整 UUID hex 构造邮箱，几乎不可能再碰撞
    loop {
        let local = format!("u{}", &Uuid::new_v4().simple().to_string()[..12]);
        let fallback = format!("{}@example.com", local);
        if used.insert(fallback.clone()) {
            return fallback;
        }
    }
}

/// 生成唯一的手机号
pub fn unique_phone_number() -> String {
    let mut gen = UNIQUE_PHONE_GEN.lock().unwrap();
    gen.generate(|| PhoneNumber().fake())
}

/// 随机生成 bcrypt 密码哈希（模拟格式 `$2b$NN$<salt><hash>`）
pub fn random_password_hash() -> String {
    let rounds: u8 = rand::thread_rng().gen_range(10..=13);
    let salt = random_base64url(22);
    let hash = random_base64url(31);
    format!("$2b${:02}${}{}", rounds, salt, hash)
}

// -----------------------------------------------------------------------------
// 随机生成器（可能重复，速度更快）
// -----------------------------------------------------------------------------

/// 随机头像链接（dicebear API）
pub fn random_avatar_url() -> String {
    format!(
        "https://api.dicebear.com/8.x/lorelei/svg?seed={}",
        random::<u32>()
    )
}

/// 随机图片链接（picsum）
pub fn random_image_url() -> String {
    let random_num = rand::thread_rng().gen_range(100_000..=999_999);
    format!("https://picsum.photos/1920/1440?random={}", random_num)
}

/// 随机地址（国家 + 省 + 市 + 街道）
pub fn random_address() -> String {
    format!(
        "{} {} {} {}",
        CountryName().fake::<String>(),
        StateName().fake::<String>(),
        CityName().fake::<String>(),
        StreetName().fake::<String>()
    )
}

/// 随机职位（资历 + 职位名称）
pub fn random_job() -> String {
    format!(
        "{}{}",
        Seniority().fake::<String>(),
        Title().fake::<String>()
    )
}

/// 随机公司名称
pub fn random_company() -> String {
    CompanyName().fake()
}

/// 随机银行卡号
pub fn random_bank_card() -> String {
    CreditCardNumber().fake()
}

/// 随机文章正文（2~10段）
pub fn random_article() -> String {
    fake::faker::lorem::zh_cn::Paragraph(2..10).fake()
}

/// 随机文章标题（1~2句话）
pub fn random_article_title() -> String {
    fake::faker::lorem::zh_cn::Sentence(1..2).fake()
}

/// 随机字母数字字符串（指定长度）
pub fn random_alphanum(len: usize) -> String {
    random_string(len)
}

/// 随机 URL
pub fn random_url() -> String {
    let path_len = rand::thread_rng().gen_range(5..12);
    format!("https://example.com/{}", random_string(path_len))
}

// ======================== 通用生成器 ========================

/// 随机 slug（例如 "my-awesome-post"）
pub fn random_slug() -> String {
    let mut rng = rand::thread_rng();
    let word_count = rng.gen_range(2..=4);
    (0..word_count)
        .map(|_| random_string(rng.gen_range(3..=8)))
        .collect::<Vec<_>>()
        .join("-")
}

/// 随机语义化版本号，例如 "1.2.3"
pub fn random_version() -> String {
    let mut rng = rand::thread_rng();
    format!(
        "{}.{}.{}",
        rng.gen_range(0..=5),
        rng.gen_range(0..=20),
        rng.gen_range(0..=99)
    )
}

/// 随机十六进制颜色，例如 "#A3F2C1"
pub fn random_color() -> String {
    let color: u32 = rand::thread_rng().gen_range(0..=0xFF_FF_FF);
    format!("#{:06X}", color)
}

/// 随机 User-Agent 字符串
pub fn random_user_agent() -> String {
    format!("Mozilla/5.0 (compatible; dbmock/{})", random_string(4))
}

/// 随机 IPv4 地址（排除广播/保留地址首尾字节为 0/255）
pub fn random_ip() -> String {
    let mut rng = rand::thread_rng();
    format!(
        "{}.{}.{}.{}",
        rng.gen_range(1_u8..=254),
        rng.gen_range(0_u8..=255),
        rng.gen_range(0_u8..=255),
        rng.gen_range(1_u8..=254),
    )
}

/// 随机文件哈希（模拟 SHA-256 十六进制，64位）
pub fn random_file_hash() -> String {
    let mut rng = rand::thread_rng();
    (0..64)
        .map(|_| format!("{:x}", rng.gen_range(0_u8..16)))
        .collect()
}

/// 随机 MIME 类型
pub fn random_mime_type() -> String {
    const MIMES: [&str; 10] = [
        "image/jpeg",
        "image/png",
        "image/gif",
        "image/webp",
        "video/mp4",
        "audio/mpeg",
        "application/pdf",
        "text/plain",
        "application/json",
        "application/octet-stream",
    ];
    let idx = rand::thread_rng().gen_range(0..MIMES.len());
    MIMES[idx].to_string()
}

/// 随机文件扩展名（不带点）
pub fn random_extension() -> String {
    const EXTS: [&str; 12] = [
        "jpg", "png", "gif", "webp", "mp4", "mp3", "pdf", "txt", "json", "xml", "zip", "tar",
    ];
    let idx = rand::thread_rng().gen_range(0..EXTS.len());
    EXTS[idx].to_string()
}

/// 随机令牌（字母数字，长度 32）
pub fn random_token() -> String {
    random_string(32)
}

/// 随机 JTI（UUID v4 格式）
pub fn random_jti() -> String {
    Uuid::new_v4().to_string()
}

/// 随机 cron 表达式（分 + 时，每天执行一次）
pub fn random_cron_expr() -> String {
    let mut rng = rand::thread_rng();
    format!("{} {} * * *", rng.gen_range(0..=59), rng.gen_range(0..=23))
}

// ======================== 电商生成器 ========================

/// 随机商品名称（中文）
pub fn random_product_name() -> String {
    const PRODUCTS: [&str; 20] = [
        "智能手机",
        "无线蓝牙耳机",
        "4K 运动相机",
        "智能手表",
        "游戏鼠标",
        "机械键盘",
        "USB-C 快充头",
        "移动电源",
        "笔记本电脑支架",
        "降噪耳机",
        "曲面显示器",
        "SSD 固态硬盘",
        "智能手环",
        "空气净化器",
        "咖啡机",
        "电动牙刷",
        "扫地机器人",
        "投影仪",
        "无人机",
        "智能音箱",
    ];
    let idx = rand::thread_rng().gen_range(0..PRODUCTS.len());
    format!("{} Pro", PRODUCTS[idx])
}

/// 随机 SKU（例如 ABCD-1234-EFG）
pub fn random_sku() -> String {
    let mut rng = rand::thread_rng();
    let prefix = random_string(4).to_uppercase();
    let num = rng.gen_range(1000..=9999);
    let suffix = random_string(3).to_uppercase();
    format!("{}-{}-{}", prefix, num, suffix)
}

/// 随机品牌名称
pub fn random_brand() -> String {
    const BRANDS: [&str; 15] = [
        "Apple", "Samsung", "Xiaomi", "Huawei", "Sony", "LG", "Dell", "HP", "Lenovo", "Asus",
        "Nike", "Adidas", "Uniqlo", "Zara", "Nestle",
    ];
    let idx = rand::thread_rng().gen_range(0..BRANDS.len());
    BRANDS[idx].to_string()
}

/// 随机订单状态
pub fn random_order_status() -> String {
    const STATUS: [&str; 7] = [
        "pending",
        "paid",
        "shipped",
        "delivered",
        "cancelled",
        "refunded",
        "completed",
    ];
    let idx = rand::thread_rng().gen_range(0..STATUS.len());
    STATUS[idx].to_string()
}

/// 随机支付方式
pub fn random_payment_method() -> String {
    const METHODS: [&str; 5] = ["alipay", "wechat", "credit_card", "paypal", "bank_transfer"];
    let idx = rand::thread_rng().gen_range(0..METHODS.len());
    METHODS[idx].to_string()
}

/// 随机物流单号（模拟顺丰格式）
pub fn random_tracking_number() -> String {
    format!("SF{}", random_string(12).to_uppercase())
}

/// 随机优惠券码（8位大写字母数字）
pub fn random_coupon_code() -> String {
    random_string(8).to_uppercase()
}

// ======================== 社区生成器 ========================

/// 随机帖子类型
pub fn random_post_type() -> String {
    const TYPES: [&str; 5] = ["post", "question", "article", "poll", "event"];
    let idx = rand::thread_rng().gen_range(0..TYPES.len());
    TYPES[idx].to_string()
}

/// 随机帖子状态（草稿/发布/置顶等）
pub fn random_post_status() -> String {
    const STATUS: [&str; 5] = ["draft", "published", "pinned", "archived", "deleted"];
    let idx = rand::thread_rng().gen_range(0..STATUS.len());
    STATUS[idx].to_string()
}

/// 随机审核状态
pub fn random_moderation_status() -> String {
    const STATUS: [&str; 4] = ["pending", "approved", "rejected", "flagged"];
    let idx = rand::thread_rng().gen_range(0..STATUS.len());
    STATUS[idx].to_string()
}

/// 随机举报类型
pub fn random_report_type() -> String {
    const TYPES: [&str; 6] = [
        "spam",
        "abuse",
        "harassment",
        "hate_speech",
        "violence",
        "other",
    ];
    let idx = rand::thread_rng().gen_range(0..TYPES.len());
    TYPES[idx].to_string()
}

/// 随机违规类型（1–10）
pub fn random_violation_type() -> i16 {
    rand::thread_rng().gen_range(1..=10)
}

/// 随机操作行为（用于 audit_logs / moderator_logs）
pub fn random_action() -> String {
    const ACTIONS: [&str; 10] = [
        "create", "update", "delete", "ban", "unban", "approve", "reject", "pin", "unpin", "lock",
    ];
    let idx = rand::thread_rng().gen_range(0..ACTIONS.len());
    ACTIONS[idx].to_string()
}

/// 随机目标类型
pub fn random_target_type() -> String {
    const TYPES: [&str; 6] = ["post", "comment", "user", "board", "tag", "topic"];
    let idx = rand::thread_rng().gen_range(0..TYPES.len());
    TYPES[idx].to_string()
}

/// 随机角色
pub fn random_role() -> String {
    const ROLES: [&str; 3] = ["user", "moderator", "admin"];
    let idx = rand::thread_rng().gen_range(0..ROLES.len());
    ROLES[idx].to_string()
}

/// 随机通知类型
pub fn random_notification_type() -> String {
    const TYPES: [&str; 6] = [
        "like",
        "comment",
        "follow",
        "mention",
        "system",
        "moderator_action",
    ];
    let idx = rand::thread_rng().gen_range(0..TYPES.len());
    TYPES[idx].to_string()
}

// ======================== 教育生成器 ========================

/// 随机课程名称（中文）
pub fn random_course_name() -> String {
    const COURSES: [&str; 15] = [
        "高等数学",
        "大学英语",
        "计算机科学导论",
        "数据结构与算法",
        "操作系统",
        "数据库系统",
        "软件工程",
        "计算机网络",
        "机器学习基础",
        "数字电路",
        "线性代数",
        "概率论与数理统计",
        "大学物理",
        "经济学原理",
        "会计学基础",
    ];
    let idx = rand::thread_rng().gen_range(0..COURSES.len());
    COURSES[idx].to_string()
}

/// 随机学科分类
pub fn random_subject() -> String {
    const SUBJECTS: [&str; 8] = [
        "Mathematics",
        "Computer Science",
        "Physics",
        "Chemistry",
        "Biology",
        "Economics",
        "Literature",
        "History",
    ];
    let idx = rand::thread_rng().gen_range(0..SUBJECTS.len());
    SUBJECTS[idx].to_string()
}

/// 随机难度等级
pub fn random_difficulty() -> String {
    const LEVELS: [&str; 3] = ["beginner", "intermediate", "advanced"];
    let idx = rand::thread_rng().gen_range(0..LEVELS.len());
    LEVELS[idx].to_string()
}

/// 随机课程类型（录播/直播/混合）
pub fn random_course_type() -> String {
    const TYPES: [&str; 3] = ["recorded", "live", "blended"];
    let idx = rand::thread_rng().gen_range(0..TYPES.len());
    TYPES[idx].to_string()
}

/// 随机题目类型
pub fn random_question_type() -> String {
    const TYPES: [&str; 5] = [
        "single_choice",
        "multiple_choice",
        "true_false",
        "essay",
        "coding",
    ];
    let idx = rand::thread_rng().gen_range(0..TYPES.len());
    TYPES[idx].to_string()
}

/// 随机成绩等级（A/B/C/D/F）
pub fn random_grade_letter() -> String {
    const GRADES: [&str; 5] = ["A", "B", "C", "D", "F"];
    let idx = rand::thread_rng().gen_range(0..GRADES.len());
    GRADES[idx].to_string()
}

/// 随机证书编号（例如 CERT-AB12-3456）
pub fn random_certificate_number() -> String {
    let mut rng = rand::thread_rng();
    format!(
        "CERT-{}-{}",
        random_string(4).to_uppercase(),
        rng.gen_range(1000..=9999)
    )
}

/// 随机作业状态
pub fn random_assignment_status() -> String {
    const STATUS: [&str; 4] = ["pending", "submitted", "graded", "late"];
    let idx = rand::thread_rng().gen_range(0..STATUS.len());
    STATUS[idx].to_string()
}

// ======================== 开发/技术场景生成器 ========================

/// 随机编程语言名称
pub fn random_programming_language() -> String {
    const LANGUAGES: [&str; 20] = [
        "Rust",
        "Python",
        "JavaScript",
        "TypeScript",
        "Go",
        "Java",
        "C#",
        "C++",
        "Swift",
        "Kotlin",
        "Ruby",
        "PHP",
        "Scala",
        "Elixir",
        "Haskell",
        "Clojure",
        "Dart",
        "Zig",
        "Nim",
        "R",
    ];
    let idx = rand::thread_rng().gen_range(0..LANGUAGES.len());
    LANGUAGES[idx].to_string()
}

/// 随机框架名称
pub fn random_framework() -> String {
    const FRAMEWORKS: [&str; 20] = [
        "Actix-web",
        "Rocket",
        "Axum",
        "Django",
        "Flask",
        "FastAPI",
        "Spring Boot",
        "Express.js",
        "NestJS",
        "React",
        "Vue.js",
        "Angular",
        "Svelte",
        "Next.js",
        "Nuxt.js",
        "Rails",
        "Laravel",
        "Symfony",
        "ASP.NET Core",
        "Quarkus",
    ];
    let idx = rand::thread_rng().gen_range(0..FRAMEWORKS.len());
    FRAMEWORKS[idx].to_string()
}

/// 随机依赖库名称
pub fn random_library() -> String {
    const LIBRARIES: [&str; 20] = [
        "serde",
        "tokio",
        "rand",
        "chrono",
        "reqwest",
        "clap",
        "anyhow",
        "thiserror",
        "sqlx",
        "diesel",
        "tracing",
        "async-graphql",
        "juniper",
        "tonic",
        "prost",
        "polars",
        "ndarray",
        "image",
        "uuid",
        "regex",
    ];
    let idx = rand::thread_rng().gen_range(0..LIBRARIES.len());
    LIBRARIES[idx].to_string()
}

/// 随机 API 端点路径（例如 `/api/v1/users`）
pub fn random_api_path() -> String {
    const PARTS: [&str; 14] = [
        "api", "v1", "v2", "users", "posts", "comments", "auth", "admin", "profile", "settings",
        "upload", "download", "search", "health",
    ];
    let mut rng = rand::thread_rng();
    let count = rng.gen_range(1..=4);
    let mut path = String::new();
    for _ in 0..count {
        let idx = rng.gen_range(0..PARTS.len());
        path.push('/');
        path.push_str(PARTS[idx]);
    }
    if path.is_empty() {
        path.push('/');
    }
    path
}

/// 随机 HTTP 方法
pub fn random_http_method() -> String {
    const METHODS: [&str; 9] = [
        "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "CONNECT", "TRACE",
    ];
    let idx = rand::thread_rng().gen_range(0..METHODS.len());
    METHODS[idx].to_string()
}

/// 随机环境名称
pub fn random_environment() -> String {
    const ENVS: [&str; 5] = ["development", "testing", "staging", "production", "local"];
    let idx = rand::thread_rng().gen_range(0..ENVS.len());
    ENVS[idx].to_string()
}

/// 随机配置键名
pub fn random_config_key() -> String {
    const KEYS: [&str; 15] = [
        "database.url",
        "redis.host",
        "log.level",
        "api.timeout",
        "jwt.secret",
        "aws.region",
        "s3.bucket",
        "email.smtp",
        "rate.limit",
        "cache.ttl",
        "feature.toggle",
        "debug.mode",
        "cors.origin",
        "max.connections",
        "pool.size",
    ];
    let idx = rand::thread_rng().gen_range(0..KEYS.len());
    KEYS[idx].to_string()
}

/// 随机 Git 短提交哈希（8位十六进制）
pub fn random_commit_hash() -> String {
    let mut rng = rand::thread_rng();
    (0..8)
        .map(|_| format!("{:x}", rng.gen_range(0_u8..16)))
        .collect()
}

/// 随机 Git 分支名
pub fn random_branch_name() -> String {
    const BRANCHES: [&str; 6] = ["main", "master", "develop", "feature", "release", "hotfix"];
    let idx = rand::thread_rng().gen_range(0..BRANCHES.len());
    match idx {
        // main / master / develop：直接返回
        0..=2 => BRANCHES[idx].to_string(),
        // feature / release / hotfix：追加随机路径
        _ => format!("{}/{}", BRANCHES[idx], random_string(6)),
    }
}

/// 随机 Git 标签名（例如 "v1.2.3"）
pub fn random_tag_name() -> String {
    format!("v{}", random_version())
}

/// 随机日志级别
pub fn random_log_level() -> String {
    const LEVELS: [&str; 5] = ["ERROR", "WARN", "INFO", "DEBUG", "TRACE"];
    let idx = rand::thread_rng().gen_range(0..LEVELS.len());
    LEVELS[idx].to_string()
}

/// 随机错误类型
pub fn random_error_type() -> String {
    const ERRORS: [&str; 8] = [
        "NotFound",
        "ValidationError",
        "DatabaseError",
        "Timeout",
        "PermissionDenied",
        "RateLimitExceeded",
        "InternalServerError",
        "ServiceUnavailable",
    ];
    let idx = rand::thread_rng().gen_range(0..ERRORS.len());
    ERRORS[idx].to_string()
}

/// 随机任务状态
pub fn random_task_status() -> String {
    const STATUS: [&str; 5] = ["pending", "running", "success", "failed", "cancelled"];
    let idx = rand::thread_rng().gen_range(0..STATUS.len());
    STATUS[idx].to_string()
}

/// 随机 Docker 镜像标签（例如 `abcdef/ghijklmn:1.2.3`）
pub fn random_docker_tag() -> String {
    format!(
        "{}/{}:{}",
        random_string(6),
        random_string(8),
        random_version()
    )
}

/// 随机容器名（例如 `abcdef-ghij`）
pub fn random_container_name() -> String {
    format!("{}-{}", random_string(6), random_string(4))
}

/// 随机数据库连接字符串（模拟，使用各数据库默认端口）
pub fn random_db_url() -> String {
    let mut rng = rand::thread_rng();
    let (scheme, port) = match rng.gen_range(0..4) {
        0 => ("postgresql", 5432_u16),
        1 => ("mysql", 3306),
        2 => ("redis", 6379),
        _ => ("mongodb", 27017),
    };
    format!(
        "{}://user:pass@localhost:{}/db_{}",
        scheme,
        port,
        random_string(6)
    )
}

/// 随机端口号字符串（用户/临时端口范围 1024–65535）
pub fn random_port_string() -> String {
    rand::thread_rng().gen_range(1024_u16..=65535).to_string()
}

/// 随机开源许可证类型
pub fn random_license() -> String {
    const LICENSES: [&str; 6] = [
        "MIT",
        "Apache-2.0",
        "GPL-3.0",
        "BSD-3-Clause",
        "MPL-2.0",
        "Unlicense",
    ];
    let idx = rand::thread_rng().gen_range(0..LICENSES.len());
    LICENSES[idx].to_string()
}
