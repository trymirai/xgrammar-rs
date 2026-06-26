//! `TokenizerInfo` binding (and the `VocabType` enum).

use crate::{error::map_error, vocab_type::VocabType};

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

fn parse_vocab_type(
    vocab_type: i32
) -> Result<xgrammar::tokenizer::VocabType, crate::error::BindingError> {
    VocabType::try_from(vocab_type).map(VocabType::to_core).map_err(map_error)
}

#[bindings::export(Implementation)]
impl TokenizerInfo {
    /// Builds tokenizer info from an encoded vocabulary.
    #[bindings::export(Method(Constructor))]
    pub fn new(
        encoded_vocab: Vec<String>,
        vocab_type: i32,
        vocab_size: Option<i32>,
        stop_token_ids: Option<Vec<i32>>,
        add_prefix_space: bool,
    ) -> Result<TokenizerInfo, crate::error::BindingError> {
        let vt = parse_vocab_type(vocab_type)?;
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
    ) -> Result<TokenizerInfo, crate::error::BindingError> {
        xgrammar::tokenizer::TokenizerInfo::from_vocab_and_metadata(
            &encoded_vocab,
            &metadata,
        )
        .map(TokenizerInfo::wrap)
        .map_err(map_error)
    }

    /// The vocabulary type, as the integer `VocabType` value.
    #[bindings::export(Method)]
    pub fn vocab_type(&self) -> i32 {
        VocabType::from_core(self.inner.vocab_type()) as i32
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
    ) -> Result<TokenizerInfo, crate::error::BindingError> {
        xgrammar::tokenizer::TokenizerInfo::deserialize_json(&json_string)
            .map(TokenizerInfo::wrap)
            .map_err(map_error)
    }

    /// Dumps tokenizer metadata (vocab type and prefix-space flag) as JSON.
    #[bindings::export(Method)]
    pub fn dump_metadata(&self) -> String {
        self.inner.dump_metadata()
    }

    /// Detects tokenizer metadata from a Hugging Face backend JSON string.
    #[bindings::export(Method(Factory))]
    pub fn _detect_metadata_from_hf(
        backend_str: String
    ) -> Result<String, crate::error::BindingError> {
        xgrammar::tokenizer::TokenizerInfo::detect_metadata_from_hf(
            &backend_str,
        )
        .map_err(map_error)
    }
}
