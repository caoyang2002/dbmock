use fake::Fake;
use fake::faker::phone_number::zh_cn::PhoneNumber;
use fake::faker::name::zh_cn::Name;
use fake::faker::address::zh_cn::{CountryName,CityName,StreetName,StateName};
use crate::datapool::UniqueGenerator;   // 之前定义的唯一生成器
use std::sync::Mutex;
use fake::faker::company::raw::CompanyName;
use once_cell::sync::Lazy;


// 全局唯一生成器（每个字段独立）
static UNIQUE_USERNAME_GEN: Lazy<Mutex<UniqueGenerator<String>>> = Lazy::new(|| Mutex::new(UniqueGenerator::new()));
static UNIQUE_PHONE_GEN: Lazy<Mutex<UniqueGenerator<String>>> = Lazy::new(|| Mutex::new(UniqueGenerator::new()));

/// 生成唯一的用户名
pub fn unique_username() -> String {
    let mut gen = UNIQUE_USERNAME_GEN.lock().unwrap();
    gen.generate(|| {
        Name().fake::<String>()
    })
}

/// 生成唯一的手机号（理论上手机号空间足够大，但为了安全也做唯一保证）
pub fn unique_phone_number() -> String {
    let mut gen = UNIQUE_PHONE_GEN.lock().unwrap();
    gen.generate(|| {
        PhoneNumber().fake::<String>()
    })
}
/// 生成唯一邮箱
pub fn unique_email() -> String {
    let username = unique_username();
    format!("{}@{}", username, "example.com")
}

/// 生成头像链接
pub fn avatar_url(username: &str) -> String {
    format!( "https://api.dicebear.com/8.x/lorelei/svg?seed={}", username)
}
/// 生成随机照片链接
pub fn random_photo_url() -> String {
    format!("https://picsum.photos/1920/1440?random={}", rand::random::<u32>())
}

/// 生成随机地址
pub fn random_address() -> String {
    format!("{} {} {} {}",
           CountryName().fake::<String>(),
           StateName().fake::<String>(),
           CityName().fake::<String>(),
           StreetName().fake::<String>())
}

/// 生成随机工作
pub fn random_job() -> String {
    format!("{} {}", Name().fake::<String>(), CompanyName().fake::<String>())
}