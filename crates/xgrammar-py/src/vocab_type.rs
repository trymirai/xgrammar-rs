//! Python `VocabType` enum — mirrors [`xgrammar::tokenizer::VocabType`].

/// How a tokenizer vocabulary is encoded (RAW, BYTE_FALLBACK, BYTE_LEVEL).
#[bindings::export(Enumeration)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VocabType {
    /// Tokens are used verbatim.
    Raw = 0,
    /// SentencePiece-style byte fallback encoding.
    ByteFallback = 1,
    /// GPT-2-style byte-level encoding.
    ByteLevel = 2,
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
