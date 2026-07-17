//! `TokenizerInfo` binding (and the `VocabType` enum).

use crate::{error::map_error, vocab_type::VocabType};

const BYTE_TOKEN_PREFIX: &str = "\u{e000}xgrammar-bytes:";

fn decode_vocab_transport(
    encoded_vocab: Vec<String>
) -> Result<Vec<Vec<u8>>, crate::error::BindingError> {
    encoded_vocab
        .into_iter()
        .map(|token| {
            let Some(hex) = token.strip_prefix(BYTE_TOKEN_PREFIX) else {
                return Ok(token.into_bytes());
            };
            if hex.len() % 2 != 0 {
                return Err(map_error(
                    "invalid encoded vocabulary byte transport",
                ));
            }
            hex.as_bytes()
                .chunks_exact(2)
                .map(|pair| {
                    std::str::from_utf8(pair)
                        .ok()
                        .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                        .ok_or_else(|| {
                            map_error(
                                "invalid encoded vocabulary byte transport",
                            )
                        })
                })
                .collect()
        })
        .collect()
}

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

// Called from the `new` constructor body, which only PyO3 emits a companion for.
#[cfg_attr(not(feature = "bindings-pyo3"), allow(dead_code))]
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
        let encoded_vocab = decode_vocab_transport(encoded_vocab)?;
        Ok(TokenizerInfo::wrap(
            xgrammar::tokenizer::TokenizerInfo::new_from_bytes(
                &encoded_vocab,
                vt,
                vocab_size,
                stop_token_ids,
                add_prefix_space,
            ),
        ))
    }

    /// Builds tokenizer info from an encoded vocabulary and a JSON metadata string.
    #[bindings::export(Method(Factory))]
    pub fn from_vocab_and_metadata(
        encoded_vocab: Vec<String>,
        metadata: String,
    ) -> Result<TokenizerInfo, crate::error::BindingError> {
        let encoded_vocab = decode_vocab_transport(encoded_vocab)?;
        xgrammar::tokenizer::TokenizerInfo::from_vocab_and_metadata_bytes(
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
    ///
    /// Omitted on the wasm backend, which cannot return `Vec<Vec<u8>>` directly; the Python
    /// tests only exercise this under PyO3.
    #[cfg(not(feature = "bindings-wasm"))]
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

    /// Serializes the tokenizer info to its `"v14"` JSON form.
    #[bindings::export(Method)]
    pub fn serialize_json(&self) -> String {
        self.inner.serialize_json()
    }

    /// Deserializes tokenizer info from its `"v14"` JSON form.
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
