pub mod unique;
pub mod username;

pub use unique::UniqueGenerator;
pub use username::{unique_username,unique_email,unique_phone_number};