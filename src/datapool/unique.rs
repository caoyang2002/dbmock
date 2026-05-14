//! 唯一数据生成器
//!
//! 利用 `HashSet` 存储已生成的值，配合 `fake` 库生成不重复的假数据。
//! 增加最大重试次数，防止因碰撞过高导致无限循环。

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

    /// 生成一个唯一值，最多重试 `MAX_RETRIES` 次。
    /// 如果重试耗尽仍然冲突，则返回最后一次生成的候选值（并强制插入），
    /// 保证了函数一定会返回，不会无限循环。
    pub fn generate<F>(&mut self, mut gen: F) -> V
    where
        F: FnMut() -> V,
    {
        const MAX_RETRIES: usize = 1000;
        let mut last_candidate = gen();

        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                last_candidate = gen();
            }
            if self.pool.insert(last_candidate.clone()) {
                return last_candidate;
            }
        }

        // Fallback: 强制插入最后一次尝试的值（即使重复）
        self.pool.insert(last_candidate.clone());
        last_candidate
    }

    /// 生成 `count` 个唯一值
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
