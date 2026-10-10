//! Thread-safe LRU cache for crossing-grammar adaptive token masks.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Mutex, MutexGuard},
};

use super::adaptive_token_mask::AdaptiveTokenMask;
use crate::support::hash_combine;

/// Unlimited cache size sentinel (matches C++ `kUnlimitedSize`).
pub const UNLIMITED_SIZE: usize = usize::MAX;

/// The cache is sharded to reduce lock contention: token mask cache generation queries and inserts
/// from all compilation threads, and a single global mutex would serialize them.
const NUM_SHARDS: usize = 16;

type NodeKey = (u64, i32, i32, i32);

/// One shard of the cache, with its own LRU order and memory budget.
#[derive(Debug)]
struct CacheState {
    max_cache_memory_size: usize,
    current_cache_memory_size: i64,
    order: VecDeque<NodeKey>,
    cache: HashMap<NodeKey, AdaptiveTokenMask>,
}

impl CacheState {
    fn new(max_cache_memory_size: usize) -> Self {
        Self {
            max_cache_memory_size,
            current_cache_memory_size: 0,
            order: VecDeque::new(),
            cache: HashMap::new(),
        }
    }

    fn clear(&mut self) {
        self.order.clear();
        self.cache.clear();
        self.current_cache_memory_size = 0;
    }

    fn get(
        &mut self,
        fsm_hash: u64,
        fsm_new_node_id: i32,
        state_cnt: i32,
        edge_cnt: i32,
    ) -> Option<AdaptiveTokenMask> {
        let key = (fsm_hash, fsm_new_node_id, state_cnt, edge_cnt);
        let mask = self.cache.get(&key)?.clone();
        if let Some(pos) = self.order.iter().position(|entry| entry == &key) {
            let entry = self.order.remove(pos).expect("position valid");
            self.order.push_back(entry);
        }
        Some(mask)
    }

    fn add(
        &mut self,
        fsm_hash: u64,
        fsm_new_node_id: i32,
        state_cnt: i32,
        edge_cnt: i32,
        token_mask: AdaptiveTokenMask,
    ) -> bool {
        let key = (fsm_hash, fsm_new_node_id, state_cnt, edge_cnt);
        if self.max_cache_memory_size != UNLIMITED_SIZE && token_mask.memory_size_bytes() > self.max_cache_memory_size {
            return false;
        }
        if self.cache.contains_key(&key) {
            return false;
        }

        if self.max_cache_memory_size != UNLIMITED_SIZE {
            let new_item_size = token_mask.memory_size_bytes();
            while self.current_cache_memory_size > self.max_cache_memory_size as i64 - new_item_size as i64 {
                let Some(oldest) = self.order.pop_front() else {
                    break;
                };
                if let Some(removed) = self.cache.remove(&oldest) {
                    self.current_cache_memory_size -= removed.memory_size_bytes() as i64;
                }
            }
        }

        self.current_cache_memory_size += token_mask.memory_size_bytes() as i64;
        self.cache.insert(key, token_mask);
        self.order.push_back(key);
        true
    }
}

/// Thread-safe LRU cache keyed by `(fsm_hash, new_state_id, state_cnt, edge_cnt)`, sharded by
/// `(fsm_hash, new_state_id)`. Eviction is performed per shard, each owning an equal share of the
/// memory budget.
#[derive(Debug)]
pub struct RuleLevelCache {
    shards: [Mutex<CacheState>; NUM_SHARDS],
    max_cache_memory_size: usize,
}

impl RuleLevelCache {
    /// Creates a cache with the given memory budget (`UNLIMITED_SIZE` = unlimited).
    #[must_use]
    pub fn new(max_cache_memory_size: usize) -> Self {
        let shard_max_size = if max_cache_memory_size == UNLIMITED_SIZE {
            UNLIMITED_SIZE
        } else {
            max_cache_memory_size / NUM_SHARDS
        };
        Self {
            shards: std::array::from_fn(|_| Mutex::new(CacheState::new(shard_max_size))),
            max_cache_memory_size,
        }
    }

    fn shard(
        &self,
        fsm_hash: u64,
        fsm_new_node_id: i32,
    ) -> MutexGuard<'_, CacheState> {
        let index = hash_combine(&[fsm_hash, fsm_new_node_id as u64]) % NUM_SHARDS as u64;
        self.shards[index as usize].lock().expect("rule level cache mutex")
    }

    /// Returns a cached adaptive token mask, if present.
    pub fn get_cache(
        &self,
        fsm_hash: u64,
        fsm_new_node_id: i32,
        state_cnt: i32,
        edge_cnt: i32,
    ) -> Option<AdaptiveTokenMask> {
        self.shard(fsm_hash, fsm_new_node_id).get(fsm_hash, fsm_new_node_id, state_cnt, edge_cnt)
    }

    /// Inserts an adaptive token mask into the cache.
    pub fn add_cache(
        &self,
        fsm_hash: u64,
        fsm_new_node_id: i32,
        state_cnt: i32,
        edge_cnt: i32,
        token_mask: AdaptiveTokenMask,
    ) -> bool {
        self.shard(fsm_hash, fsm_new_node_id).add(fsm_hash, fsm_new_node_id, state_cnt, edge_cnt, token_mask)
    }

    /// Clears all cached entries.
    pub fn clear_cache(&self) {
        for shard in &self.shards {
            shard.lock().expect("rule level cache mutex").clear();
        }
    }

    /// The configured memory budget (`UNLIMITED_SIZE` = unlimited).
    #[must_use]
    pub fn max_size(&self) -> usize {
        self.max_cache_memory_size
    }
}
