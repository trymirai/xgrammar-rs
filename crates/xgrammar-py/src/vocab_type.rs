//! Python `VocabType` enum — mirrors [`xgrammar::tokenizer::VocabType`].

/// How a tokenizer vocabulary is encoded (RAW, BYTE_FALLBACK, BYTE_LEVEL).
///
/// No explicit discriminants — NAPI's string-enum codegen rejects them. The numeric
/// mapping (0/1/2) lives in [`VocabType::to_core`] / [`VocabType::from_core`] and the
/// `TryFrom<i32>` impl, which the Python layer relies on.
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

impl VocabType {
    pub(crate) fn to_core(self) -> xgrammar::tokenizer::VocabType {
        match self {
            Self::Raw => xgrammar::tokenizer::VocabType::Raw,
            Self::ByteFallback => xgrammar::tokenizer::VocabType::ByteFallback,
            Self::ByteLevel => xgrammar::tokenizer::VocabType::ByteLevel,
        }
    }

    pub(crate) fn from_core(vt: xgrammar::tokenizer::VocabType) -> Self {
        match vt {
            xgrammar::tokenizer::VocabType::Raw => Self::Raw,
            xgrammar::tokenizer::VocabType::ByteFallback => Self::ByteFallback,
            xgrammar::tokenizer::VocabType::ByteLevel => Self::ByteLevel,
        }
    }
}

impl TryFrom<i32> for VocabType {
    type Error = xgrammar::tokenizer::UnknownVocabType;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        xgrammar::tokenizer::VocabType::try_from(i64::from(value))
            .map(Self::from_core)
    }
}
