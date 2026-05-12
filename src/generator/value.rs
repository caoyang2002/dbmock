use rand::Rng;
use rand::seq::SliceRandom;
use uuid::Uuid;
use chrono::{Utc, Duration};
use crate::core::schema::ColumnSchema;

/// Generate a SQL literal value for a column
pub fn generate_value(col: &ColumnSchema, db_type: &str) -> String {
    // Use column name hints first
    if let Some(v) = name_based_value(&col.name, &col.data_type) {
        return v;
    }

    type_based_value(&col.data_type, col.max_length, col.numeric_precision, col.numeric_scale, db_type)
}

fn name_based_value(name: &str, data_type: &str) -> Option<String> {
    let n = name.to_lowercase();

    // Email patterns
    if n.contains("email") || n.contains("mail") {
        return Some(format!("'{}'", fake_email()));
    }

    // UUID / GUID
    if n == "id" || n.ends_with("_id") || n.contains("uuid") || n.contains("guid") {
        if data_type.contains("uuid") || data_type.contains("char") || data_type.contains("text") {
            return Some(format!("'{}'", Uuid::new_v4()));
        }
    }

    // Name fields
    if n == "name" || n.ends_with("_name") || n == "fullname" || n == "full_name" {
        return Some(format!("'{}'", fake_full_name()));
    }
    if n == "first_name" || n == "firstname" {
        return Some(format!("'{}'", fake_first_name()));
    }
    if n == "last_name" || n == "lastname" || n == "surname" {
        return Some(format!("'{}'", fake_last_name()));
    }
    if n == "username" || n == "user_name" || n == "login" {
        return Some(format!("'{}'", fake_username()));
    }

    // Phone
    if n.contains("phone") || n.contains("mobile") || n.contains("tel") {
        return Some(format!("'{}'", fake_phone()));
    }

    // Address
    if n == "address" || n.ends_with("_address") || n.contains("street") {
        return Some(format!("'{}'", fake_address()));
    }
    if n == "city" || n.ends_with("_city") {
        return Some(format!("'{}'", fake_city()));
    }
    if n == "country" || n.ends_with("_country") {
        return Some(format!("'{}'", fake_country()));
    }
    if n == "zip" || n == "zipcode" || n == "postal_code" || n == "postcode" {
        return Some(format!("'{}'", fake_zip()));
    }

    // URL / website
    if n.contains("url") || n.contains("website") || n.contains("link") {
        return Some(format!("'{}'", fake_url()));
    }

    // Description / content / body
    if n.contains("description") || n.contains("content") || n.contains("body") || n.contains("bio") {
        return Some(format!("'{}'", fake_paragraph()));
    }
    if n.contains("title") || n.contains("subject") || n.contains("headline") {
        return Some(format!("'{}'", fake_sentence()));
    }

    // Status / state
    if n == "status" || n == "state" {
        return Some(format!("'{}'", fake_status()));
    }

    // Boolean-like flags
    if n.starts_with("is_") || n.starts_with("has_") || n.starts_with("can_") || n == "active" || n == "enabled" {
        return Some(fake_bool_str(data_type));
    }

    // Password (hashed, never real)
    if n.contains("password") || n.contains("passwd") || n.contains("pwd") {
        return Some(format!("'{}'", fake_hash()));
    }

    // JSON fields
    if n.contains("meta") || n.contains("config") || n.contains("settings") || n.contains("data") {
        if data_type.contains("json") {
            return Some("'{\"key\":\"value\"}'".to_string());
        }
    }

    // Timestamps
    if n.contains("created") || n.contains("updated") || n.contains("deleted") || n.contains("at") {
        if data_type.contains("timestamp") || data_type.contains("datetime") || data_type.contains("date") {
            return Some(fake_timestamp());
        }
    }

    // Age / count
    if n == "age" {
        return Some(rand::thread_rng().gen_range(18..=80).to_string());
    }
    if n == "count" || n.ends_with("_count") || n.ends_with("_num") || n.ends_with("_number") {
        return Some(rand::thread_rng().gen_range(0..=1000).to_string());
    }

    None
}

fn type_based_value(
    data_type: &str,
    max_length: Option<i32>,
    numeric_precision: Option<i32>,
    numeric_scale: Option<i32>,
    db_type: &str,
) -> String {
    let dt = data_type.to_lowercase();

    // Integer types
    if dt.contains("int") || dt == "integer" || dt == "bigint" || dt == "smallint" || dt == "tinyint" {
        return rand::thread_rng().gen_range(1..=100000).to_string();
    }

    // Floating point
    if dt.contains("float") || dt.contains("double") || dt.contains("real") {
        let v: f64 = rand::thread_rng().gen_range(0.0..10000.0);
        return format!("{:.4}", v);
    }

    // Decimal / numeric
    if dt.contains("decimal") || dt.contains("numeric") || dt.contains("money") {
        let scale = numeric_scale.unwrap_or(2) as usize;
        let v: f64 = rand::thread_rng().gen_range(0.0..9999.0);
        return format!("{:.prec$}", v, prec = scale);
    }

    // Boolean
    if dt.contains("bool") || dt == "bit" {
        return fake_bool_str(&dt);
    }

    // UUID
    if dt == "uuid" {
        return format!("'{}'", Uuid::new_v4());
    }

    // JSON / JSONB
    if dt.contains("json") {
        return "'{\"generated\":true}'".to_string();
    }

    // Dates / timestamps
    if dt.contains("timestamp") || dt.contains("datetime") {
        return fake_timestamp();
    }
    if dt == "date" {
        return fake_date();
    }
    if dt == "time" {
        return fake_time();
    }

    // Year
    if dt == "year" {
        return rand::thread_rng().gen_range(2000..=2024).to_string();
    }

    // Text / varchar / char / string
    if dt.contains("char") || dt.contains("text") || dt.contains("string") || dt == "clob" {
        let max = max_length.unwrap_or(50).min(100) as usize;
        let len = rand::thread_rng().gen_range(5..(max + 1).max(6));
        return format!("'{}'", random_string(len));
    }

    // Binary (skip with NULL or placeholder)
    if dt.contains("blob") || dt.contains("binary") || dt.contains("bytes") {
        return "NULL".to_string();
    }

    // Enum - return NULL or placeholder
    if dt.contains("enum") || dt.contains("set") {
        return "NULL".to_string();
    }

    // Default fallback: short string
    format!("'{}'", random_string(8))
}

fn fake_bool_str(data_type: &str) -> String {
    let b = rand::thread_rng().gen_bool(0.5);
    if data_type.contains("bool") {
        if b { "true".to_string() } else { "false".to_string() }
    } else {
        if b { "1".to_string() } else { "0".to_string() }
    }
}

fn fake_email() -> String {
    let domains = ["gmail.com", "yahoo.com", "outlook.com", "example.com", "test.org"];
    let domain = domains.choose(&mut rand::thread_rng()).unwrap();
    format!(
        "{}.{}@{}",
        fake_first_name().to_lowercase(),
        rand::thread_rng().gen_range(10..=999),
        domain
    )
}

fn fake_first_name() -> String {
    let names = [
        "Alice", "Bob", "Charlie", "Diana", "Eve", "Frank", "Grace", "Henry",
        "Iris", "Jack", "Karen", "Leo", "Mia", "Noah", "Olivia", "Paul",
        "Quinn", "Rose", "Sam", "Tina", "Uma", "Victor", "Wendy", "Xander",
        "Yara", "Zoe", "Liam", "Emma", "Ava", "Sophia", "Mason", "Logan",
    ];
    names.choose(&mut rand::thread_rng()).unwrap().to_string()
}

fn fake_last_name() -> String {
    let names = [
        "Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller",
        "Davis", "Martinez", "Wilson", "Anderson", "Taylor", "Thomas", "Moore",
        "Jackson", "White", "Harris", "Martin", "Thompson", "Young", "King",
    ];
    names.choose(&mut rand::thread_rng()).unwrap().to_string()
}

fn fake_full_name() -> String {
    format!("{} {}", fake_first_name(), fake_last_name())
}

fn fake_username() -> String {
    let first = fake_first_name().to_lowercase();
    let n: u32 = rand::thread_rng().gen_range(10..=9999);
    format!("{}{}", first, n)
}

fn fake_phone() -> String {
    let mut rng = rand::thread_rng();
    format!(
        "+1-{:03}-{:03}-{:04}",
        rng.gen_range(200..=999),
        rng.gen_range(100..=999),
        rng.gen_range(1000..=9999),
    )
}

fn fake_address() -> String {
    let streets = ["Main St", "Oak Ave", "Maple Dr", "Cedar Ln", "Park Blvd", "Lake Rd"];
    let st = streets.choose(&mut rand::thread_rng()).unwrap();
    let n: u32 = rand::thread_rng().gen_range(1..=9999);
    format!("{} {}", n, st)
}

fn fake_city() -> String {
    let cities = [
        "New York", "Los Angeles", "Chicago", "Houston", "Phoenix",
        "Philadelphia", "San Antonio", "San Diego", "Dallas", "San Jose",
        "Austin", "Jacksonville", "Seattle", "Denver", "Boston",
    ];
    cities.choose(&mut rand::thread_rng()).unwrap().to_string()
}

fn fake_country() -> String {
    let countries = ["US", "GB", "CA", "AU", "DE", "FR", "JP", "CN", "BR", "IN"];
    countries.choose(&mut rand::thread_rng()).unwrap().to_string()
}

fn fake_zip() -> String {
    let n: u32 = rand::thread_rng().gen_range(10000..=99999);
    n.to_string()
}

fn fake_url() -> String {
    let domains = ["example.com", "test.org", "demo.io", "sample.net"];
    let d = domains.choose(&mut rand::thread_rng()).unwrap();
    format!("https://www.{}/page/{}", d, rand::thread_rng().gen_range(1..=999))
}

fn fake_paragraph() -> String {
    let words = [
        "lorem", "ipsum", "dolor", "sit", "amet", "consectetur",
        "adipiscing", "elit", "sed", "do", "eiusmod", "tempor",
        "incididunt", "ut", "labore", "et", "dolore", "magna", "aliqua",
    ];
    let mut rng = rand::thread_rng();
    let count = rng.gen_range(10..=25);
    let sentence: Vec<&str> = (0..count).map(|_| *words.choose(&mut rng).unwrap()).collect();
    let text = sentence.join(" ");
    // Escape single quotes
    text.replace('\'', "''")
}

fn fake_sentence() -> String {
    let words = [
        "The", "Quick", "Brown", "Fox", "Jumps", "Over", "Lazy", "Dog",
        "Hello", "World", "New", "Update", "Feature", "Release", "Version",
        "Important", "Breaking", "News", "Today",
    ];
    let mut rng = rand::thread_rng();
    let count = rng.gen_range(3..=8);
    let sentence: Vec<&str> = (0..count).map(|_| *words.choose(&mut rng).unwrap()).collect();
    sentence.join(" ").replace('\'', "''")
}

fn fake_status() -> String {
    let statuses = ["active", "inactive", "pending", "approved", "rejected", "draft"];
    statuses.choose(&mut rand::thread_rng()).unwrap().to_string()
}

fn fake_hash() -> String {
    // bcrypt-like placeholder
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".chars().collect();
    let hash: String = (0..60).map(|_| *chars.choose(&mut rand::thread_rng()).unwrap()).collect();
    format!("$2b$12${}", hash)
}

fn fake_timestamp() -> String {
    let mut rng = rand::thread_rng();
    let days_ago = rng.gen_range(0..=730);
    let dt = Utc::now() - Duration::days(days_ago);
    format!("'{}'", dt.format("%Y-%m-%d %H:%M:%S"))
}

fn fake_date() -> String {
    let mut rng = rand::thread_rng();
    let days_ago = rng.gen_range(0..=1825);
    let dt = (Utc::now() - Duration::days(days_ago)).date_naive();
    format!("'{}'", dt.format("%Y-%m-%d"))
}

fn fake_time() -> String {
    let mut rng = rand::thread_rng();
    format!("'{:02}:{:02}:{:02}'", rng.gen_range(0..=23), rng.gen_range(0..=59), rng.gen_range(0..=59))
}

fn random_string(len: usize) -> String {
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyz0123456789".chars().collect();
    (0..len)
        .map(|_| *chars.choose(&mut rand::thread_rng()).unwrap())
        .collect()
}
