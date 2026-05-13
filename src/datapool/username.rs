use fake::Fake;
use fake::faker::phone_number::zh_cn::PhoneNumber;
use fake::faker::name::zh_cn::Name;
use fake::faker::address::zh_cn::{CountryName,CityName,StreetName,StateName};
use crate::datapool::UniqueGenerator;   // 之前定义的唯一生成器
use std::sync::Mutex;
use fake::faker::company::zh_cn::CompanyName;
use fake::faker::creditcard::zh_cn::CreditCardNumber;
use fake::faker::internet::zh_cn::{FreeEmail, SafeEmail};
use fake::faker::job::zh_cn::{Seniority, Title};
use fake::faker::number::raw::NumberWithFormat;
use once_cell::sync::Lazy;
use rand::random;

// 全局唯一生成器（每个字段独立）
/// 用户名生成器
static UNIQUE_USERNAME_GEN: Lazy<Mutex<UniqueGenerator<String>>> = Lazy::new(|| Mutex::new(UniqueGenerator::new()));
/// 手机号生成器
static UNIQUE_PHONE_GEN: Lazy<Mutex<UniqueGenerator<String>>> = Lazy::new(|| Mutex::new(UniqueGenerator::new()));

/// 邮箱生成器
static UNIQUE_EMAIL_GEN: Lazy<Mutex<UniqueGenerator<String>>> = Lazy::new(|| Mutex::new(UniqueGenerator::new()));

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
/// 示例：username
pub fn unique_email() -> String {
    let mut gen = UNIQUE_EMAIL_GEN.lock().unwrap();
    gen.generate(|| {
        SafeEmail().fake::<String>()
    })
}

/// 生成随机头像链接
/// 示例：https://api.dicebear.com/8.x/lorelei/svg?seed=1234
pub fn random_avatar_url() -> String {
    format!( "https://api.dicebear.com/8.x/lorelei/svg?seed={}",random::<u32>() )
}
/// 生成随机照片链接
pub fn unique_avatar_url() -> String {
    use rand::Rng;
    let random_num = rand::thread_rng().gen_range(100_000..999_999);
    format!("https://picsum.photos/1920/1440?random={}", random_num)
}

/// 生成随机地址
pub fn random_address() -> String {
    format!("{} {} {} {}",
           CountryName().fake::<String>(),
           StateName().fake::<String>(),
           CityName().fake::<String>(),
           StreetName().fake::<String>())
}

/// 生成随机职位
pub fn random_job() -> String {
    format!("{}{}",Seniority().fake::<String>(),Title().fake::<String>())
}

/// 随机生成公司名称
pub fn random_company() -> String {
    format!("{}",CompanyName().fake::<String>())
}

/// 随机生成银行卡号
pub fn random_bank_card() -> String {
    format!("{}",CreditCardNumber().fake::<String>())
}

/// 随机文章
pub fn random_article() -> String {
    format!("{}",fake::faker::lorem::zh_cn::Paragraph(2..10).fake::<String>())
}

/// 随机生成文章标题
pub fn random_article_title() -> String {
    format!("{}",fake::faker::lorem::zh_cn::Sentence(1..2).fake::<String>())
}