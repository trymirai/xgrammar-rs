//! `TokenizerInfo` binding (and the `VocabType` enum).

use crate::error::XgrammarError;

/// A thin opaque wrapper over [`xgrammar::tokenizer::TokenizerInfo`].
#[bindings::export(Class)]
#[derive(Debug, Clone)]
pub struct TokenizerInfo {
    pub(crate) inner: xgrammar::tokenizer::TokenizerInfo,
}

impl TokenizerInfo {
    pub(crate) fn wrap(inner: xgrammar::tokenizer::TokenizerInfo) -> Self {
        Self {
            inner,
        }
    }
}

#[bindings::export(Implementation)]
impl TokenizerInfo {
    /// Builds tokenizer info from an encoded vocabulary. `vocab_type` is the integer value of
    /// the Python `VocabType` enum (0 = RAW, 1 = BYTE_FALLBACK, 2 = BYTE_LEVEL).
    #[bindings::export(Method(Factory))]
    pub fn new(
        encoded_vocab: Vec<String>,
        vocab_type: i32,
        vocab_size: Option<i32>,
        stop_token_ids: Option<Vec<i32>>,
        add_prefix_space: bool,
    ) -> Result<TokenizerInfo, XgrammarError> {
        let vt =
            xgrammar::tokenizer::VocabType::try_from(i64::from(vocab_type))
                .map_err(XgrammarError::from_display)?;
        Ok(TokenizerInfo::wrap(xgrammar::tokenizer::TokenizerInfo::new(
            &encoded_vocab,
            vt,
            vocab_size,
            stop_token_ids,
            add_prefix_space,
        )))
    }

    /// Builds tokenizer info from an encoded vocabulary and a JSON metadata string.
    #[bindings::export(Method(Factory))]
    pub fn from_vocab_and_metadata(
        encoded_vocab: Vec<String>,
        metadata: String,
    ) -> Result<TokenizerInfo, XgrammarError> {
        xgrammar::tokenizer::TokenizerInfo::from_vocab_and_metadata(
            &encoded_vocab,
            &metadata,
        )
        .map(TokenizerInfo::wrap)
        .map_err(XgrammarError::from_display)
    }

    /// The vocabulary type, as the integer `VocabType` value.
    #[bindings::export(Method)]
    pub fn vocab_type(&self) -> i32 {
        self.inner.vocab_type() as i32
    }

    /// The vocabulary size (including padding tokens).
    #[bindings::export(Method)]
    pub fn vocab_size(&self) -> i32 {
        self.inner.vocab_size()
    }

    /// Whether a prefix space is added during tokenization.
    #[bindings::export(Method)]
    pub fn add_prefix_space(&self) -> bool {
        self.inner.add_prefix_space()
    }

    /// The decoded byte string of each token id.
    #[bindings::export(Method)]
    pub fn decoded_vocab(&self) -> Vec<Vec<u8>> {
        self.inner.decoded_vocab().to_vec()
    }

    /// The stop token ids.
    #[bindings::export(Method)]
    pub fn stop_token_ids(&self) -> Vec<i32> {
        self.inner.stop_token_ids().to_vec()
    }

    /// The special token ids.
    #[bindings::export(Method)]
    pub fn special_token_ids(&self) -> Vec<i32> {
        self.inner.special_token_ids().to_vec()
    }

    /// Serializes the tokenizer info to its `"v11"` JSON form.
    #[bindings::export(Method)]
    pub fn serialize_json(&self) -> String {
        self.inner.serialize_json()
    }

    /// Deserializes tokenizer info from its `"v11"` JSON form.
    #[bindings::export(Method(Factory))]
    pub fn deserialize_json(
        json_string: String
    ) -> Result<TokenizerInfo, XgrammarError> {
        xgrammar::tokenizer::TokenizerInfo::deserialize_json(&json_string)
            .map(TokenizerInfo::wrap)
            .map_err(XgrammarError::from_display)
    }
}
