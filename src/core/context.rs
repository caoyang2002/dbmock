use std::collections::HashMap;
use std::cell::RefCell;
use crate::core::ColumnSchema;
use crate::datapool::UniqueGenerator;

/// 生成上下文
pub struct GenContext {
    unique_gens: RefCell<HashMap<String, UniqueGenerator<String>>>,
}

impl GenContext {
    pub fn new() -> Self {
        Self {
            unique_gens: RefCell::new(HashMap::new()),
        }
    }

    /// 为某个列生成值，如果该列有唯一约束则自动保证唯一性
    pub fn gen_value_for_column(
        &self,
        col: &ColumnSchema,
        max_len: usize,
        generate_inner: impl Fn() -> String,
    ) -> String {
        // println!("DEBUG: gen_value_for_column called for col='{}', is_unique={}", col.name, col.is_unique);
        if col.is_unique {
            // println!("🔐 UNIQUE column: {}", col.name);
            // 使用该列专属的唯一生成器
            let mut map = self.unique_gens.borrow_mut();
            let generator = map.entry(col.name.clone()).or_insert_with(UniqueGenerator::new);
            generator.generate(|| generate_inner())
        } else {
            generate_inner()
        }
    }
}
