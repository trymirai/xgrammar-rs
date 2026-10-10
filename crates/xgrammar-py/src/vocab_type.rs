//! Python `VocabType` enum — mirrors [`xgrammar::tokenizer::VocabType`].

/// How a tokenizer vocabulary is encoded (RAW, BYTE_FALLBACK, BYTE_LEVEL).
///
/// No explicit discriminants — NAPI's string-enum codegen rejects them. The numeric
/// mapping (0/1/2) lives in the `TryFrom<i32>` impl, which the Python layer relies on.
#[bindings::export(Enumeration)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VocabType {
    /// Tokens are used verbatim.
    Raw,
    /// SentencePiece-style byte fallback encoding.
    ByteFallback,
    /// GPT-2-style byte-level encoding.
    ByteLevel,
}

impl From<VocabType> for xgrammar::tokenizer::VocabType {
    fn from(vocab_type: VocabType) -> Self {
        match vocab_type {
            VocabType::Raw => Self::Raw,
            VocabType::ByteFallback => Self::ByteFallback,
            VocabType::ByteLevel => Self::ByteLevel,
        }
    }
}

impl From<xgrammar::tokenizer::VocabType> for VocabType {
    fn from(vocab_type: xgrammar::tokenizer::VocabType) -> Self {
        match vocab_type {
            xgrammar::tokenizer::VocabType::Raw => Self::Raw,
            xgrammar::tokenizer::VocabType::ByteFallback => Self::ByteFallback,
            xgrammar::tokenizer::VocabType::ByteLevel => Self::ByteLevel,
        }
    }
}

impl TryFrom<i32> for VocabType {
    type Error = xgrammar::tokenizer::UnknownVocabType;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        xgrammar::tokenizer::VocabType::try_from(i64::from(value)).map(Self::from)
    }
}
