use std::collections::BTreeMap;
use std::hash::Hasher;

use mea::rwlock::RwLock;

pub trait ShardDataKey: Ord + Clone {
    fn get_shard_idx(&self, totcap: usize) -> usize;
}

pub struct ShardedLockedData<K, T> {
    shards: Vec<RwLock<BTreeMap<K, T>>>,
}

impl<K: ShardDataKey, T> ShardedLockedData<K, T> {
    pub fn new(cap: usize) -> ShardedLockedData<K, T> {
        let mut shards = vec![];
        for _ in 0..cap {
            shards.push(RwLock::new(BTreeMap::new()));
        }
        ShardedLockedData { shards }
    }

    fn get_shard(&self, key: &K) -> &RwLock<BTreeMap<K, T>> {
        let idx = key.get_shard_idx(self.shards.len());
        debug_assert!(idx < self.shards.len());
        self.shards.get(idx).unwrap()
    }

    pub async fn contains_key(&self, key: &K) -> bool {
        let shard = self.get_shard(key);
        shard.read().await.contains_key(key)
    }

    pub async fn insert(&self, key: K, val: T) -> Option<T> {
        let shard = self.get_shard(&key);
        shard.write().await.insert(key, val)
    }

    pub async fn remove(&self, key: &K) -> Option<T> {
        let shard = self.get_shard(key);
        shard.write().await.remove(key)
    }

    pub async fn map<F, V>(&self, key: &K, f: F) -> Option<V>
    where
        F: FnOnce(&mut T) -> V,
    {
        let shard = self.get_shard(key);
        let mut sref = shard.write().await;
        let val = sref.get_mut(key)?;
        Some(f(val))
    }

    pub async fn get_all_keys(&self) -> Vec<K> {
        let mut result = vec![];
        for shard in self.shards.iter() {
            result.extend(shard.read().await.keys().cloned())
        }
        result.sort();
        result
    }
}

impl<K: ShardDataKey, T: Clone> ShardedLockedData<K, T> {
    pub async fn clone_val(&self, key: &K) -> Option<T> {
        let shard = self.get_shard(key);
        shard.write().await.get(key).cloned()
    }
}

impl ShardDataKey for u16 {
    fn get_shard_idx(&self, totcap: usize) -> usize {
        let sep = (u16::MAX as usize) / totcap;
        (*self as usize) / sep
    }
}

impl ShardDataKey for u64 {
    fn get_shard_idx(&self, totcap: usize) -> usize {
        let sep = (u64::MAX as usize) / totcap;
        (*self as usize) / sep
    }
}

impl ShardDataKey for crate::player::PlayerKey {
    fn get_shard_idx(&self, totcap: usize) -> usize {
        let mut h = std::hash::DefaultHasher::new();
        h.write(self);
        let n = h.finish() as usize;
        let sep = usize::MAX / totcap;
        n / sep
    }
}

impl ShardDataKey for String {
    fn get_shard_idx(&self, totcap: usize) -> usize {
        let mut h = std::hash::DefaultHasher::new();
        h.write(self.as_bytes());
        let n = h.finish() as usize;
        let sep = usize::MAX / totcap;
        let res = n / sep;
        log::debug!("shard data key {self} = {res} (totcap {totcap})");
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::block_on;

    #[test]
    fn test_u16_shard_idx_in_bounds() {
        let totcap = 100;
        for k in [0u16, 1, 12345, u16::MAX] {
            assert!(k.get_shard_idx(totcap) <= totcap);
        }
        assert_eq!(0u16.get_shard_idx(totcap), 0);
    }

    #[test]
    fn test_u64_shard_idx_in_bounds() {
        let totcap = 100;
        for k in [0u64, 1, 999_999, u64::MAX / 2] {
            assert!(k.get_shard_idx(totcap) <= totcap);
        }
    }

    #[test]
    fn test_insert_then_contains_and_clone() {
        block_on(async {
            let data: ShardedLockedData<u64, i32> = ShardedLockedData::new(10);
            assert!(!data.contains_key(&42).await);
            let prev = data.insert(42, 7).await;
            assert!(prev.is_none());
            assert!(data.contains_key(&42).await);
            assert_eq!(data.clone_val(&42).await, Some(7));
        });
    }

    #[test]
    fn test_insert_overwrite_returns_previous() {
        block_on(async {
            let data: ShardedLockedData<u64, i32> = ShardedLockedData::new(10);
            data.insert(1, 100).await;
            let prev = data.insert(1, 200).await;
            assert_eq!(prev, Some(100));
            assert_eq!(data.clone_val(&1).await, Some(200));
        });
    }

    #[test]
    fn test_remove() {
        block_on(async {
            let data: ShardedLockedData<u64, i32> = ShardedLockedData::new(10);
            data.insert(5, 50).await;
            assert_eq!(data.remove(&5).await, Some(50));
            assert!(!data.contains_key(&5).await);
            assert!(data.remove(&5).await.is_none());
        });
    }

    #[test]
    fn test_map_mutates_value() {
        block_on(async {
            let data: ShardedLockedData<u64, i32> = ShardedLockedData::new(10);
            data.insert(3, 10).await;
            let out = data
                .map(&3, |v| {
                    *v += 5;
                    *v
                })
                .await;
            assert_eq!(out, Some(15));
            assert_eq!(data.clone_val(&3).await, Some(15));
            // Mapping a missing key yields None
            assert!(data.map(&999, |v| *v).await.is_none());
        });
    }

    #[test]
    fn test_get_all_keys_sorted() {
        block_on(async {
            let data: ShardedLockedData<u64, i32> = ShardedLockedData::new(10);
            data.insert(30, 0).await;
            data.insert(10, 0).await;
            data.insert(20, 0).await;
            assert_eq!(data.get_all_keys().await, vec![10, 20, 30]);
        });
    }
}
