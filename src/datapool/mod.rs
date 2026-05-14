// datapool/mod.rs

mod mock_generators;
mod unique;
pub use unique::UniqueGenerator;
// 统一导出所有生成器（唯一 + 随机）
pub use mock_generators::{
    random_action, random_address, random_alphanum, random_api_path, random_article,
    random_article_title, random_assignment_status, random_avatar_url, random_bank_card,
    random_branch_name, random_brand, random_certificate_number, random_color, random_commit_hash,
    random_company, random_config_key, random_container_name, random_coupon_code,
    random_course_name, random_course_type, random_cron_expr, random_database_url,
    random_difficulty, random_docker_tag, random_environment, random_error_type, random_extension,
    random_file_hash, random_framework, random_grade_letter, random_http_method, random_image_url,
    random_ip, random_job, random_jti, random_library, random_license, random_log_level,
    random_mime_type, random_moderation_status, random_notification_type, random_order_status,
    random_password_hash, random_payment_method, random_port_string, random_post_status,
    random_post_type, random_product_name, random_programming_language, random_question_type,
    random_report_type, random_role, random_sku, random_slug, random_subject, random_tag_name,
    random_target_type, random_task_status, random_token, random_tracking_number, random_url,
    random_user_agent, random_version, random_violation_type, unique_email, unique_phone_number,
    unique_username,
};
