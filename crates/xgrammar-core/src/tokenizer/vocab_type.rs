//! The vocabulary encoding type — a port of `VocabType` in `cpp/include/xgrammar/tokenizer_info.h`.

use serde::{Deserialize, Serialize};

/// How a tokenizer's raw vocabulary strings are encoded (and thus how they decode to bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VocabType {
    /// Tokens are used verbatim.
    Raw = 0,
    /// SentencePiece-style: `<0xNN>` byte tokens and `▁` (U+2581) as space.
    ByteFallback = 1,
    /// GPT-2-style byte-to-unicode remapping.
    ByteLevel = 2,
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
