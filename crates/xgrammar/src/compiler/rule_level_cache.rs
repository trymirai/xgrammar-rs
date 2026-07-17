//! Thread-safe LRU cache for crossing-grammar adaptive token masks.

use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
};

use super::adaptive_token_mask::AdaptiveTokenMask;

/// Unlimited cache size sentinel (matches C++ `kUnlimitedSize`).
pub const UNLIMITED_SIZE: usize = usize::MAX;

type NodeKey = (u64, i32, i32, i32);

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

/// Thread-safe LRU cache keyed by `(fsm_hash, new_state_id, state_cnt, edge_cnt)`.
#[derive(Debug)]
pub struct RuleLevelCache {
    state: Mutex<CacheState>,
    max_cache_memory_size: usize,
}

impl RuleLevelCache {
    /// Creates a cache with the given memory budget (`UNLIMITED_SIZE` = unlimited).
    #[must_use]
    pub fn new(max_cache_memory_size: usize) -> Self {
        Self {
            state: Mutex::new(CacheState::new(max_cache_memory_size)),
            max_cache_memory_size,
        }
    }

    /// Returns a cached adaptive token mask, if present.
    pub fn get_cache(
        &self,
        fsm_hash: u64,
        fsm_new_node_id: i32,
        state_cnt: i32,
        edge_cnt: i32,
    ) -> Option<AdaptiveTokenMask> {
        self.state.lock().expect("rule level cache mutex").get(fsm_hash, fsm_new_node_id, state_cnt, edge_cnt)
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
        self.state.lock().expect("rule level cache mutex").add(
            fsm_hash,
            fsm_new_node_id,
            state_cnt,
            edge_cnt,
            token_mask,
        )
    }

    /// Clears all cached entries.
    pub fn clear_cache(&self) {
        self.state.lock().expect("rule level cache mutex").clear();
    }

    /// The configured memory budget (`UNLIMITED_SIZE` = unlimited).
    #[must_use]
    pub fn max_size(&self) -> usize {
        self.max_cache_memory_size
    }
}
