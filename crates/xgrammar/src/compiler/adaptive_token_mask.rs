//! Adaptive token-mask cache.
//!.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread,
};

use serde_json::{Value, json};

use super::{
    rule_level_cache::RuleLevelCache, tag_dispatch_optimization::tag_dispatch_optimization,
    token_mask_cache_builder::GrammarMatcherForTokenMaskCache,
};
use crate::{
    grammar::{DeserializeError, Grammar},
    parser::{ParserState, ParserStateCacheKey},
    support::DynamicBitset,
    tokenizer::TokenizerInfo,
};

/// How accepted/rejected sorted-vocab indices are stored (mirrors C++ `StoreType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum AdaptiveTokenMaskStoreType {
    /// Store accepted indices; rejected = complement − uncertain.
    Accepted = 0,
    /// Store rejected indices; accepted = complement − uncertain.
    Rejected = 1,
    /// Store accepted token ids in a bitset indexed by token id.
    AcceptedBitset = 2,
}

impl AdaptiveTokenMaskStoreType {
    fn from_i32(value: i32) -> Result<Self, DeserializeError> {
        match value {
            0 => Ok(Self::Accepted),
            1 => Ok(Self::Rejected),
            2 => Ok(Self::AcceptedBitset),
            other => Err(DeserializeError::Format(format!("invalid adaptive token mask store_type {other}"))),
        }
    }
}

/// Precomputed token classification for one parser state (mirrors C++ `AdaptiveTokenMask`).
///
/// Indices refer to positions in [`TokenizerInfo::sorted_decoded_vocab`], not token ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaptiveTokenMask {
    pub store_type: AdaptiveTokenMaskStoreType,
    pub accepted_indices: Vec<i32>,
    pub rejected_indices: Vec<i32>,
    pub accepted_bitset: DynamicBitset,
    pub uncertain_indices: Vec<i32>,
}

impl AdaptiveTokenMask {
    /// Threshold above which accepted/rejected lists switch to a bitset (mirrors C++).
    pub const USE_BITSET_THRESHOLD: usize = 1000;

    /// Builds a mask from accepted, rejected, and uncertain sorted-vocab indices.
    #[must_use]
    pub fn from_classifications(
        vocab_size: usize,
        sorted_decoded_vocab: &[(i32, Vec<u8>)],
        accepted_indices: &[i32],
        rejected_indices: &[i32],
        uncertain_indices: &[i32],
    ) -> Self {
        let size_acc = accepted_indices.len();
        let size_rej = rejected_indices.len();
        let store_type = if size_acc >= Self::USE_BITSET_THRESHOLD && size_rej >= Self::USE_BITSET_THRESHOLD {
            AdaptiveTokenMaskStoreType::AcceptedBitset
        } else if size_acc < size_rej {
            AdaptiveTokenMaskStoreType::Accepted
        } else {
            AdaptiveTokenMaskStoreType::Rejected
        };

        let mut accepted_bitset = DynamicBitset::new(0);
        let (accepted_indices, rejected_indices) = match store_type {
            AdaptiveTokenMaskStoreType::AcceptedBitset => {
                accepted_bitset = DynamicBitset::new(vocab_size);
                for &idx in accepted_indices {
                    let token_id = sorted_decoded_vocab[idx as usize].0 as usize;
                    accepted_bitset.set(token_id, true);
                }
                (Vec::new(), Vec::new())
            },
            AdaptiveTokenMaskStoreType::Accepted => (accepted_indices.to_vec(), Vec::new()),
            AdaptiveTokenMaskStoreType::Rejected => (Vec::new(), rejected_indices.to_vec()),
        };

        Self {
            store_type,
            accepted_indices,
            rejected_indices,
            accepted_bitset,
            uncertain_indices: uncertain_indices.to_vec(),
        }
    }

    /// Builds a mask storing only accepted indices (rejected inferred at runtime).
    #[must_use]
    pub fn from_accepted_and_uncertain(
        vocab_size: usize,
        sorted_decoded_vocab: &[(i32, Vec<u8>)],
        accepted_indices: &[i32],
        uncertain_indices: &[i32],
    ) -> Self {
        let store_type = if accepted_indices.len() >= Self::USE_BITSET_THRESHOLD {
            AdaptiveTokenMaskStoreType::AcceptedBitset
        } else {
            AdaptiveTokenMaskStoreType::Accepted
        };
        let mut accepted_bitset = DynamicBitset::new(0);
        let accepted_indices = match store_type {
            AdaptiveTokenMaskStoreType::AcceptedBitset => {
                accepted_bitset = DynamicBitset::new(vocab_size);
                for &idx in accepted_indices {
                    let token_id = sorted_decoded_vocab[idx as usize].0 as usize;
                    accepted_bitset.set(token_id, true);
                }
                Vec::new()
            },
            AdaptiveTokenMaskStoreType::Accepted => accepted_indices.to_vec(),
            AdaptiveTokenMaskStoreType::Rejected => unreachable!(),
        };
        Self {
            store_type,
            accepted_indices,
            rejected_indices: Vec::new(),
            accepted_bitset,
            uncertain_indices: uncertain_indices.to_vec(),
        }
    }

    /// Approximate memory footprint in bytes.
    #[must_use]
    pub fn memory_size_bytes(&self) -> usize {
        self.accepted_indices.len() * 4
            + self.rejected_indices.len() * 4
            + self.uncertain_indices.len() * 4
            + self.accepted_bitset.as_words().len() * 4
    }

    /// Serializes to a JSON object (mirrors C++ reflection member table).
    #[must_use]
    pub fn serialize_json_value(&self) -> Value {
        json!({
            "store_type": self.store_type as i32,
            "accepted_indices": self.accepted_indices,
            "rejected_indices": self.rejected_indices,
            "accepted_bitset": self.accepted_bitset.serialize_json_value_cpp(),
            "uncertain_indices": self.uncertain_indices,
        })
    }

    /// Deserializes from a JSON object.
    ///
    /// # Errors
    /// Returns [`DeserializeError`] when fields are missing or invalid.
    pub fn deserialize_json_value(value: &Value) -> Result<Self, DeserializeError> {
        let store_type = AdaptiveTokenMaskStoreType::from_i32(
            value
                .get("store_type")
                .and_then(Value::as_i64)
                .ok_or_else(|| DeserializeError::Format("adaptive token mask missing store_type".to_owned()))?
                as i32,
        )?;
        let accepted_indices = value
            .get("accepted_indices")
            .and_then(Value::as_array)
            .ok_or_else(|| DeserializeError::Format("adaptive token mask missing accepted_indices".to_owned()))?
            .iter()
            .map(|item| {
                item.as_i64()
                    .ok_or_else(|| {
                        DeserializeError::Format("adaptive token mask accepted_indices must be integers".to_owned())
                    })
                    .map(|v| v as i32)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let rejected_indices = value
            .get("rejected_indices")
            .and_then(Value::as_array)
            .ok_or_else(|| DeserializeError::Format("adaptive token mask missing rejected_indices".to_owned()))?
            .iter()
            .map(|item| {
                item.as_i64()
                    .ok_or_else(|| {
                        DeserializeError::Format("adaptive token mask rejected_indices must be integers".to_owned())
                    })
                    .map(|v| v as i32)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let uncertain_indices = value
            .get("uncertain_indices")
            .and_then(Value::as_array)
            .ok_or_else(|| DeserializeError::Format("adaptive token mask missing uncertain_indices".to_owned()))?
            .iter()
            .map(|item| {
                item.as_i64()
                    .ok_or_else(|| {
                        DeserializeError::Format("adaptive token mask uncertain_indices must be integers".to_owned())
                    })
                    .map(|v| v as i32)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let accepted_bitset = DynamicBitset::deserialize_json_value_cpp(
            value
                .get("accepted_bitset")
                .ok_or_else(|| DeserializeError::Format("adaptive token mask missing accepted_bitset".to_owned()))?,
        )
        .map_err(DeserializeError::Format)?;

        Ok(Self {
            store_type,
            accepted_indices,
            rejected_indices,
            accepted_bitset,
            uncertain_indices,
        })
    }
}

/// Mapping from parser-state cache keys to precomputed masks.
pub type AdaptiveTokenMaskCache = HashMap<ParserStateCacheKey, AdaptiveTokenMask>;

/// Serializes the cache as `List[Tuple[ParserState member array, AdaptiveTokenMask]]`.
#[must_use]
pub fn serialize_cache_json_value(cache: &AdaptiveTokenMaskCache) -> Value {
    let mut entries: Vec<(ParserStateCacheKey, &AdaptiveTokenMask)> =
        cache.iter().map(|(key, mask)| (*key, mask)).collect();
    entries.sort_by_key(|(key, _)| (key.rule_id, key.sequence_id, key.element_id, key.sub_element_id));
    let entries: Vec<Value> = entries
        .into_iter()
        .map(|(key, mask)| {
            let state = ParserState::new(
                key.rule_id,
                key.sequence_id,
                key.element_id,
                ParserState::NO_PREV_INPUT_POS,
                key.sub_element_id,
            );
            json!([state.serialize_member_array(), mask.serialize_json_value(),])
        })
        .collect();
    Value::Array(entries)
}

/// Deserializes a cache from JSON.
///
/// # Errors
/// Returns [`DeserializeError`] when entries are malformed.
pub fn deserialize_cache_json_value(value: &Value) -> Result<AdaptiveTokenMaskCache, DeserializeError> {
    let entries = value
        .as_array()
        .ok_or_else(|| DeserializeError::Format("adaptive_token_mask_cache must be a JSON array".to_owned()))?;
    let mut cache = AdaptiveTokenMaskCache::new();
    for entry in entries {
        let pair = entry
            .as_array()
            .ok_or_else(|| DeserializeError::Format("adaptive token mask cache entry must be a pair".to_owned()))?;
        if pair.len() != 2 {
            return Err(DeserializeError::Format("adaptive token mask cache entry must have two elements".to_owned()));
        }
        let members: Vec<i32> = pair[0]
            .as_array()
            .ok_or_else(|| DeserializeError::Format("adaptive token mask cache key must be an array".to_owned()))?
            .iter()
            .map(|item| {
                item.as_i64()
                    .ok_or_else(|| {
                        DeserializeError::Format("parser state member array must contain integers".to_owned())
                    })
                    .map(|v| v as i32)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let state = ParserState::from_member_array(&members);
        let mask = AdaptiveTokenMask::deserialize_json_value(&pair[1])?;
        cache.insert(state.cache_key(), mask);
    }
    Ok(cache)
}

/// Builds the adaptive token-mask cache for every scanable reachable FSM state.
#[must_use]
pub fn build_adaptive_token_mask_cache(
    grammar: &Grammar,
    tokenizer_info: Arc<TokenizerInfo>,
    max_threads: i32,
    rule_level_cache: Option<Arc<RuleLevelCache>>,
) -> AdaptiveTokenMaskCache {
    if tokenizer_info.vocab_size() == 0 {
        return AdaptiveTokenMaskCache::new();
    }

    let tag_dispatch_bitsets = Arc::new(tag_dispatch_optimization(grammar, &tokenizer_info));
    let grammar = Arc::new(grammar.clone());
    let root_rule_id = grammar.root_rule_id();
    let mut tasks: Vec<(ParserState, bool)> = Vec::new();

    for rule_id in 0..grammar.num_rules() {
        let rule = grammar.rule(rule_id);
        let fsm = grammar.per_rule_fsm(rule_id).expect("optimized grammar has per-rule FSMs").fsm();
        for element_id in fsm.reachable_states() {
            if !fsm.is_scanable_state(element_id) {
                continue;
            }
            tasks.push((
                ParserState::new(rule_id, rule.body_expr_id, element_id, ParserState::NO_PREV_INPUT_POS, 0),
                rule_id == root_rule_id,
            ));
        }
    }

    if tasks.is_empty() {
        return AdaptiveTokenMaskCache::new();
    }

    if max_threads > 1 {
        let cache = Arc::new(Mutex::new(AdaptiveTokenMaskCache::new()));
        thread::scope(|scope| {
            let chunk_size = tasks.len().div_ceil(max_threads as usize).max(1);
            for chunk in tasks.chunks(chunk_size) {
                let chunk = chunk.to_vec();
                let grammar = Arc::clone(&grammar);
                let tokenizer_info = Arc::clone(&tokenizer_info);
                let tag_dispatch_bitsets = Arc::clone(&tag_dispatch_bitsets);
                let rule_level_cache = rule_level_cache.as_ref().map(Arc::clone);
                let cache = Arc::clone(&cache);
                scope.spawn(move || {
                    let local = build_masks_for_states(
                        &grammar,
                        &tokenizer_info,
                        &tag_dispatch_bitsets,
                        rule_level_cache,
                        &chunk,
                    );
                    let mut guard = cache.lock().expect("cache mutex");
                    guard.extend(local);
                });
            }
        });
        Arc::try_unwrap(cache).expect("no other cache owners").into_inner().expect("cache mutex")
    } else {
        build_masks_for_states(&grammar, &tokenizer_info, &tag_dispatch_bitsets, rule_level_cache, &tasks)
    }
}

fn build_masks_for_states(
    grammar: &Arc<Grammar>,
    tokenizer_info: &Arc<TokenizerInfo>,
    tag_dispatch_bitsets: &Arc<super::tag_dispatch_optimization::TagDispatchSecondSlicingBitsets>,
    rule_level_cache: Option<Arc<RuleLevelCache>>,
    tasks: &[(ParserState, bool)],
) -> AdaptiveTokenMaskCache {
    let mut cache = AdaptiveTokenMaskCache::new();
    for &(state, is_root_rule) in tasks {
        let mut matcher = GrammarMatcherForTokenMaskCache::new(
            Arc::clone(grammar),
            state,
            Arc::clone(tag_dispatch_bitsets),
            Arc::clone(tokenizer_info),
            rule_level_cache.as_ref().map(Arc::clone),
        );
        let mask = matcher.get_adaptive_token_mask(is_root_rule);
        cache.insert(state.cache_key(), mask);
    }
    cache
}
