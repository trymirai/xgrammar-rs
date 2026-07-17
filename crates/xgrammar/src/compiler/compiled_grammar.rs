//! The compiled grammar — a port of `CompiledGrammar` in `cpp/compiled_grammar.cc`.
//!
//! Bundles an optimized [`Grammar`] with the [`TokenizerInfo`] it was compiled against and a
//! per-state [`AdaptiveTokenMask`] cache that accelerates [`GrammarMatcher::fill_next_token_bitmask`].

use std::sync::Arc;

use serde_json::{Value, json};

use super::{
    adaptive_token_mask::{
        AdaptiveTokenMaskCache, build_adaptive_token_mask_cache, deserialize_cache_json_value,
        serialize_cache_json_value,
    },
    rule_level_cache::RuleLevelCache,
};
use crate::{
    config::SERIALIZATION_VERSION,
    grammar::{DeserializeError, Grammar},
    tokenizer::TokenizerInfo,
};

/// The preprocessing result that a [`GrammarMatcher`](crate::matcher::GrammarMatcher) runs on:
/// an optimized grammar plus its tokenizer and adaptive token-mask cache.
#[derive(Debug, Clone)]
pub struct CompiledGrammar {
    grammar: Grammar,
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
            grammar,
            tokenizer_info: Arc::new(tokenizer_info),
            adaptive_token_mask_cache: Arc::new(adaptive_token_mask_cache),
        }
    }

    /// The associated (optimized) grammar.
    #[must_use]
    pub fn grammar(&self) -> &Grammar {
        &self.grammar
    }

    /// The associated tokenizer info.
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
        let cache =
            build_adaptive_token_mask_cache(&grammar, Arc::clone(&tokenizer_info), max_threads, rule_level_cache);
        Self {
            grammar,
            tokenizer_info,
            adaptive_token_mask_cache: Arc::new(cache),
        }
    }

    /// An approximate memory footprint of the compiled grammar, in bytes.
    #[must_use]
    pub fn memory_size_bytes(&self) -> usize {
        let exprs = self.grammar.num_exprs() as usize * 4;
        let rules = self.grammar.num_rules() as usize * 32;
        let vocab: usize = self.tokenizer_info.decoded_vocab().iter().map(|t| t.len() + 16).sum();
        let cache: usize = self.adaptive_token_mask_cache.values().map(|mask| mask.memory_size_bytes()).sum();
        exprs + rules + vocab + cache
    }

    /// Serializes the compiled grammar without embedding the full tokenizer info.
    #[must_use]
    pub fn serialize_json(&self) -> String {
        serde_json::to_string(&self.serialize_json_value()).expect("compiled grammar JSON serialization never fails")
    }

    /// Serializes the compiled grammar to a JSON value.
    #[must_use]
    pub fn serialize_json_value(&self) -> Value {
        let grammar = self.grammar.serialize_json_value_with_fsm();
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
        let value: Value =
            serde_json::from_str(json_str).map_err(|error| DeserializeError::InvalidJson(error.to_string()))?;
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
        let adaptive_token_mask_cache = match value.get("adaptive_token_mask_cache") {
            Some(cache_value) => deserialize_cache_json_value(cache_value)?,
            None => AdaptiveTokenMaskCache::new(),
        };
        Ok(Self::new(grammar, tokenizer_info.clone(), adaptive_token_mask_cache))
    }
}
