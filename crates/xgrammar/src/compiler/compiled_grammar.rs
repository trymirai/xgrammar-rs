//! The compiled grammar: preprocessing results of a [`GrammarMatcher`](crate::matcher::GrammarMatcher).
//!
//! Contains the preprocessing results of the grammar and tokenizer.

use std::sync::Arc;

use serde_json::{Value, json};

use super::{
    adaptive_token_mask::{
        AdaptiveTokenMask, AdaptiveTokenMaskCache, AdaptiveTokenMaskStoreType, build_adaptive_token_mask_cache,
        deserialize_cache_json_value, serialize_cache_json_value,
    },
    rule_level_cache::RuleLevelCache,
};
use crate::{
    config::SERIALIZATION_VERSION,
    grammar::{DeserializeError, Grammar},
    support::parse_json,
    tokenizer::TokenizerInfo,
};

/// The compiled grammar of a [`GrammarMatcher`](crate::matcher::GrammarMatcher).
///
/// It contains the preprocessing results of the grammar and tokenizer.
#[derive(Debug, Clone)]
pub struct CompiledGrammar {
    grammar: Arc<Grammar>,
    tokenizer_info: Arc<TokenizerInfo>,
    adaptive_token_mask_cache: Arc<AdaptiveTokenMaskCache>,
}

impl CompiledGrammar {
    /// Creates a compiled grammar from an (already optimized) grammar, tokenizer, and cache.
    #[must_use]
    pub fn new(
        grammar: Grammar,
        tokenizer_info: TokenizerInfo,
        adaptive_token_mask_cache: AdaptiveTokenMaskCache,
    ) -> Self {
        Self {
            grammar: Arc::new(grammar),
            tokenizer_info: Arc::new(tokenizer_info),
            adaptive_token_mask_cache: Arc::new(adaptive_token_mask_cache),
        }
    }

    /// Gets the associated grammar, shared with every matcher built from this compiled grammar.
    #[must_use]
    pub fn grammar(&self) -> &Arc<Grammar> {
        &self.grammar
    }

    /// Gets the associated tokenizer info.
    #[must_use]
    pub fn tokenizer_info(&self) -> &TokenizerInfo {
        &self.tokenizer_info
    }

    /// Shared handle to the tokenizer (cheap to clone).
    #[must_use]
    pub fn tokenizer_info_arc(&self) -> Arc<TokenizerInfo> {
        Arc::clone(&self.tokenizer_info)
    }

    /// The precomputed adaptive token-mask cache.
    #[must_use]
    pub fn adaptive_token_mask_cache(&self) -> &AdaptiveTokenMaskCache {
        &self.adaptive_token_mask_cache
    }

    /// Shared handle to the adaptive token-mask cache (cheap to clone).
    #[must_use]
    pub fn adaptive_token_mask_cache_arc(&self) -> Arc<AdaptiveTokenMaskCache> {
        Arc::clone(&self.adaptive_token_mask_cache)
    }

    /// Builds a compiled grammar, including the adaptive token-mask cache.
    ///
    /// `max_threads` controls parallel cache construction.
    #[must_use]
    pub fn build(
        grammar: Grammar,
        tokenizer_info: Arc<TokenizerInfo>,
        max_threads: i32,
        rule_level_cache: Option<Arc<RuleLevelCache>>,
    ) -> Self {
        let grammar = Arc::new(grammar);
        let cache =
            build_adaptive_token_mask_cache(&grammar, Arc::clone(&tokenizer_info), max_threads, rule_level_cache);
        Self {
            grammar,
            tokenizer_info,
            adaptive_token_mask_cache: Arc::new(cache),
        }
    }

    /// Returns the approximate memory usage of the grammar in bytes.
    #[must_use]
    pub fn memory_size_bytes(&self) -> usize {
        let exprs = self.grammar.num_exprs() as usize * 4;
        let rules = self.grammar.num_rules() as usize * 32;
        let vocab: usize = self.tokenizer_info.decoded_vocab().iter().map(|t| t.len() + 16).sum();
        let cache: usize = self.adaptive_token_mask_cache.values().map(|mask| mask.memory_size_bytes()).sum();
        exprs + rules + vocab + cache
    }

    /// Returns the serialized JSON string of the compiled grammar.
    #[must_use]
    pub fn serialize_json(&self) -> String {
        serde_json::to_string(&self.serialize_json_value()).expect("compiled grammar JSON serialization never fails")
    }

    /// Serializes the compiled grammar to a JSON value.
    #[must_use]
    pub fn serialize_json_value(&self) -> Value {
        let grammar = self.grammar.serialize_json_value_embedded();
        json!({
            "grammar": grammar,
            "tokenizer_metadata": self.tokenizer_info.metadata_value(),
            "adaptive_token_mask_cache": serialize_cache_json_value(&self.adaptive_token_mask_cache),
            "__VERSION__": SERIALIZATION_VERSION,
        })
    }

    /// Deserializes a compiled grammar and binds it to `tokenizer_info`.
    ///
    /// # Errors
    /// Returns [`DeserializeError`] when JSON, version, metadata, or grammar body is invalid.
    pub fn deserialize_json(
        json_str: &str,
        tokenizer_info: &TokenizerInfo,
    ) -> Result<Self, DeserializeError> {
        let value: Value = parse_json(json_str)
            .map_err(|error| DeserializeError::InvalidJson(format!("Failed to parse JSON: {error}")))?;
        Self::deserialize_json_value(&value, tokenizer_info)
    }

    /// Deserializes a compiled grammar from a JSON value.
    ///
    /// # Errors
    /// Returns [`DeserializeError`] when version, metadata, or grammar body is invalid.
    pub fn deserialize_json_value(
        value: &Value,
        tokenizer_info: &TokenizerInfo,
    ) -> Result<Self, DeserializeError> {
        match value.get("__VERSION__").and_then(Value::as_str) {
            Some(SERIALIZATION_VERSION) => {},
            Some(other) => {
                return Err(DeserializeError::Version {
                    expected: SERIALIZATION_VERSION.to_owned(),
                    got: other.to_owned(),
                });
            },
            None => {
                return Err(DeserializeError::Format("missing __VERSION__".to_owned()));
            },
        }
        let grammar_value =
            value.get("grammar").ok_or_else(|| DeserializeError::Format("missing grammar".to_owned()))?;
        let metadata = value
            .get("tokenizer_metadata")
            .ok_or_else(|| DeserializeError::Format("missing tokenizer_metadata".to_owned()))?;
        tokenizer_info.check_metadata_match(metadata)?;
        let grammar = Grammar::deserialize_json_value_embedded(grammar_value)?;
        let cache_value = value
            .get("adaptive_token_mask_cache")
            .ok_or_else(|| DeserializeError::Format("Expect a 'adaptive_token_mask_cache' field".to_owned()))?;
        let adaptive_token_mask_cache = deserialize_cache_json_value(cache_value)?;
        // The masks index the sorted vocabulary and are OR-ed into a vocab_size-bit bitset, so
        // their contents must match the tokenizer they are deserialized with.
        let num_sorted_tokens = tokenizer_info.sorted_decoded_vocab().len() as i32;
        let indices_ok = |indices: &[i32]| indices.iter().all(|index| (0..num_sorted_tokens).contains(index));
        let mask_ok = |mask: &AdaptiveTokenMask| {
            (mask.store_type != AdaptiveTokenMaskStoreType::AcceptedBitset
                || mask.accepted_bitset.len() == tokenizer_info.vocab_size() as usize)
                && indices_ok(&mask.accepted_indices)
                && indices_ok(&mask.rejected_indices)
                && indices_ok(&mask.uncertain_indices)
        };
        if !adaptive_token_mask_cache.values().all(mask_ok) {
            return Err(DeserializeError::Format(
                "adaptive_token_mask_cache contains a mask that does not match the tokenizer".to_owned(),
            ));
        }
        Ok(Self::new(grammar, tokenizer_info.clone(), adaptive_token_mask_cache))
    }
}
