//! 唯一数据生成器
//!
//! 利用 `HashSet` 存储已生成的值，配合 `fake` 库生成不重复的假数据。

use std::collections::HashSet;
use std::hash::Hash;

pub struct UniqueGenerator<V> {
    pool: HashSet<V>,
}

impl<V> UniqueGenerator<V>
where
    V: Hash + Eq + Clone,
{
    pub fn new() -> Self {
        Self {
            pool: HashSet::new(),
        }
    }
    /// 插入一个值，如果该值已存在则返回 false，否则返回 true
    pub fn insert(&mut self, value: V) -> bool {
        self.pool.insert(value)
    }

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

    pub fn generate_n<F>(&mut self, count: usize, mut gen: F) -> Vec<V>
    where
        F: FnMut() -> V,
    {
        (0..count).map(|_| self.generate(&mut gen)).collect()
    }

    pub fn len(&self) -> usize {
        self.pool.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pool.is_empty()
    }

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
