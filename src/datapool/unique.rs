//! 唯一数据生成器
//!
//! 利用 `HashSet` 存储已生成的值（或其哈希），配合 `fake` 库生成不重复的假数据。

use std::collections::HashSet;
use std::hash::Hash;

/// 唯一数据生成器
///
/// 内部维护一个 `HashSet<V>`，所有通过它生成的值都会被记录，
/// 再次生成时自动跳过重复项，直到得到一个新的唯一值。
///
/// # 类型参数
/// - `V`: 要生成的数据类型，必须实现 `Hash + Eq + Clone`。
pub struct UniqueGenerator<V> {
    pool: HashSet<V>,
}

impl<V> UniqueGenerator<V>
where
    V: Hash + Eq + Clone,
{
    /// 创建一个空的唯一生成器
    pub fn new() -> Self {
        Self {
            pool: HashSet::new(),
        }
    }

    /// 生成一个不重复的值
    ///
    /// # 参数
    /// - `gen`: 一个产生 `V` 类型值的闭包或函数（通常会调用 `fake` 库的生成器）。
    ///
    /// # 返回
    /// 一个此前从未被该生成器产生过的值。
    ///
    /// # 注意
    /// 该方法会循环调用 `gen` 直到得到一个新值，如果可能永远无法得到新值
    /// （例如生成空间有限且已全部用尽），理论上会死循环，实际使用时请确保
    /// 生成器有足够的样本空间。
    pub fn generate<F>(&mut self, mut gen: F) -> V
    where
        F: FnMut() -> V,
    {
        loop {
            let candidate = gen();
            if self.pool.insert(candidate.clone()) {
                return candidate;
            }
        }
    }

    /// 批量生成多个不重复的值
    ///
    /// # 参数
    /// - `count`: 要生成的数量
    /// - `gen`: 产生单个值的闭包
    ///
    /// # 返回
    /// 包含 `count` 个不重复值的 `Vec<V>`
    ///
    /// # Panics
    /// 如果生成器无法提供足够多的不重复值（导致无限循环），本函数会永远运行。
    /// 实际使用时请确保 `count` 小于生成器的可能取值总数。
    pub fn generate_n<F>(&mut self, count: usize, mut gen: F) -> Vec<V>
    where
        F: FnMut() -> V,
    {
        (0..count).map(|_| self.generate(&mut gen)).collect()
    }

    /// 当前已生成的不同值的数量
    pub fn len(&self) -> usize {
        self.pool.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.pool.is_empty()
    }

    /// 清空所有已记录的值，重新开始
    pub fn clear(&mut self) {
        self.pool.clear();
    }
}

impl<V> Default for UniqueGenerator<V>
where
    V: Hash + Eq + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fake::faker::name::en::FirstName;
    use fake::Fake;

    #[test]
    fn test_unique_strings() {
        let mut gen = UniqueGenerator::new();

        // 生成 100 个不重复的名字（假数据）
        let names: Vec<String> = (0..100)
            .map(|_| gen.generate(|| FirstName().fake()))
            .collect();

        // 检查是否有重复
        let unique_count = names.iter().collect::<HashSet<_>>().len();
        assert_eq!(unique_count, names.len());
        assert_eq!(gen.len(), 100);
    }

    #[test]
    fn test_generate_n() {
        let mut gen = UniqueGenerator::new();
        let items = gen.generate_n(50, || (0..100).fake::<u32>());
        assert_eq!(items.len(), 50);
        assert_eq!(gen.len(), 50);
    }
}