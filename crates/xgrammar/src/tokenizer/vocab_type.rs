//! The type of the vocabulary. Used in [`TokenizerInfo`](super::TokenizerInfo).
//!
//! XGrammar supports three types of vocabularies: [`Raw`](VocabType::Raw),
//! [`ByteFallback`](VocabType::ByteFallback), and [`ByteLevel`](VocabType::ByteLevel).

use serde::{Deserialize, Serialize};

/// How a tokenizer's raw vocabulary strings are encoded (and thus how they decode to bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VocabType {
    /// The vocabulary is in the raw format.
    ///
    /// The tokens in the vocabulary are kept in their original form without any processing. This
    /// kind of tokenizer includes the tiktoken tokenizer, e.g. `microsoft/Phi-3-small-8k-instruct`,
    /// `Qwen/Qwen-7B-Chat`, etc.
    Raw = 0,
    /// The vocabulary used in the byte fallback BPE tokenizer.
    ///
    /// The tokens are encoded through the byte-fallback conversion. E.g. `"\u001b"` → `"<0x1B>"`,
    /// `" apple"` → `"▁apple"`. This kind of tokenizer includes `meta-llama/Llama-2-7b-chat`,
    /// `microsoft/Phi-3.5-mini-instruct`, etc.
    ByteFallback = 1,
    /// The vocabulary used in the byte level BPE tokenizer.
    ///
    /// The tokens are encoded through the byte-to-unicode conversion, as in the GPT-2
    /// tokenization scheme. This kind of tokenizer includes `meta-llama/Meta-Llama-3-8B-Instruct`,
    /// `meta-llama/Meta-Llama-3.1-8B-Instruct`, etc.
    ByteLevel = 2,
}

impl VocabType {
    /// Backwards-compatible spelling used by xgrammar-rs 0.2.
    pub const RAW: Self = Self::Raw;
    /// Backwards-compatible spelling used by xgrammar-rs 0.2.
    pub const BYTE_FALLBACK: Self = Self::ByteFallback;
    /// Backwards-compatible spelling used by xgrammar-rs 0.2.
    pub const BYTE_LEVEL: Self = Self::ByteLevel;
}

/// Error converting an integer to a [`VocabType`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid vocab type: {0}")]
pub struct UnknownVocabType(pub i64);

impl TryFrom<i64> for VocabType {
    type Error = UnknownVocabType;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Raw),
            1 => Ok(Self::ByteFallback),
            2 => Ok(Self::ByteLevel),
            other => Err(UnknownVocabType(other)),
        }
    }
}
