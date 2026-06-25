//! The compiled grammar — a port of `CompiledGrammar` in `cpp/compiled_grammar.cc`.
//!
//! Bundles an optimized [`Grammar`] with the [`TokenizerInfo`] it was compiled against. The
//! C++ also precomputes a per-state `AdaptiveTokenMask` cache purely to speed up
//! `fill_next_token_bitmask`; that cache is a performance optimization (the masks it yields
//! are identical to the matcher's on-the-fly computation) and is deferred to the perf phase.

use serde_json::{Value, json};

use crate::{
    config::SERIALIZATION_VERSION,
    grammar::{DeserializeError, Grammar},
    tokenizer::TokenizerInfo,
};

/// The preprocessing result that a [`GrammarMatcher`](crate::matcher::GrammarMatcher) runs on:
/// an optimized grammar plus its tokenizer.
#[derive(Debug, Clone)]
pub struct CompiledGrammar {
    grammar: Grammar,
    tokenizer_info: TokenizerInfo,
}

impl CompiledGrammar {
    /// Creates a compiled grammar from an (already optimized) grammar and tokenizer.
    #[must_use]
    pub fn new(
        grammar: Grammar,
        tokenizer_info: TokenizerInfo,
    ) -> Self {
        Self {
            grammar,
            tokenizer_info,
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

    /// An approximate memory footprint of the compiled grammar, in bytes.
    #[must_use]
    pub fn memory_size_bytes(&self) -> usize {
        let exprs = self.grammar.num_exprs() as usize * 4;
        let rules = self.grammar.num_rules() as usize * 32;
        let vocab: usize = self
            .tokenizer_info
            .decoded_vocab()
            .iter()
            .map(|t| t.len() + 16)
            .sum();
        exprs + rules + vocab
    }

    /// Serializes the compiled grammar without embedding the full tokenizer info.
    #[must_use]
    pub fn serialize_json(&self) -> String {
        serde_json::to_string(&self.serialize_json_value())
            .expect("compiled grammar JSON serialization never fails")
    }

    /// Serializes the compiled grammar to a JSON value.
    #[must_use]
    pub fn serialize_json_value(&self) -> Value {
        let grammar = self.grammar.serialize_json_value_with_fsm();
        json!({
            "grammar": grammar,
            "tokenizer_metadata": self.tokenizer_info.metadata_value(),
            "adaptive_token_mask_cache": json!([]),
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
        let value: Value = serde_json::from_str(json_str).map_err(|error| {
            DeserializeError::InvalidJson(error.to_string())
        })?;
        match value.get("__VERSION__").and_then(Value::as_str) {
            Some(SERIALIZATION_VERSION) => {},
            Some(other) => {
                return Err(DeserializeError::Version {
                    expected: SERIALIZATION_VERSION.to_owned(),
                    got: other.to_owned(),
                });
            },
            None => {
                return Err(DeserializeError::Format(
                    "missing __VERSION__".to_owned(),
                ));
            },
        }
        let grammar_value = value.get("grammar").ok_or_else(|| {
            DeserializeError::Format("missing grammar".to_owned())
        })?;
        let metadata = value.get("tokenizer_metadata").ok_or_else(|| {
            DeserializeError::Format("missing tokenizer_metadata".to_owned())
        })?;
        tokenizer_info.check_metadata_match(metadata)?;
        let grammar = Grammar::deserialize_json_value_embedded(grammar_value)?;
        Ok(Self::new(grammar, tokenizer_info.clone()))
    }
}
