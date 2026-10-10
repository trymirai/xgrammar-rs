//! `TokenizerInfo` binding (and the `VocabType` enum).

use crate::{error::map_error, vocab_type::VocabType};

/// A thin opaque wrapper over [`xgrammar::tokenizer::TokenizerInfo`].
#[bindings::export(Class)]
#[derive(Debug, Clone)]
pub struct TokenizerInfo {
    inner: xgrammar::tokenizer::TokenizerInfo,
}

impl From<xgrammar::tokenizer::TokenizerInfo> for TokenizerInfo {
    fn from(inner: xgrammar::tokenizer::TokenizerInfo) -> Self {
        Self {
            inner,
        }
    }
}

impl AsRef<xgrammar::tokenizer::TokenizerInfo> for TokenizerInfo {
    fn as_ref(&self) -> &xgrammar::tokenizer::TokenizerInfo {
        &self.inner
    }
}

// The single builders behind the portable string exports and the PyO3 `str | bytes` factories.
// The constructor body only exists in backend companions, which wasm does not emit.
#[cfg_attr(not(feature = "bindings-pyo3"), allow(dead_code))]
fn from_vocab_bytes(
    encoded_vocab: &[Vec<u8>],
    vocab_type: i32,
    vocab_size: Option<i32>,
    stop_token_ids: Option<Vec<i32>>,
    add_prefix_space: bool,
) -> Result<TokenizerInfo, crate::error::BindingError> {
    let vocab_type = VocabType::try_from(vocab_type).map(xgrammar::tokenizer::VocabType::from).map_err(map_error)?;
    xgrammar::tokenizer::TokenizerInfo::new_from_bytes(
        encoded_vocab,
        vocab_type,
        vocab_size,
        stop_token_ids,
        add_prefix_space,
    )
    .map(TokenizerInfo::from)
    .map_err(map_error)
}

fn from_vocab_bytes_and_metadata(
    encoded_vocab: &[Vec<u8>],
    metadata: &str,
) -> Result<TokenizerInfo, crate::error::BindingError> {
    xgrammar::tokenizer::TokenizerInfo::from_vocab_and_metadata_bytes(encoded_vocab, metadata)
        .map(TokenizerInfo::from)
        .map_err(map_error)
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
        let encoded_vocab: Vec<Vec<u8>> = encoded_vocab.into_iter().map(String::into_bytes).collect();
        from_vocab_bytes(&encoded_vocab, vocab_type, vocab_size, stop_token_ids, add_prefix_space)
    }

    /// Builds tokenizer info from an encoded vocabulary and a JSON metadata string.
    #[bindings::export(Method(Factory))]
    pub fn from_vocab_and_metadata(
        encoded_vocab: Vec<String>,
        metadata: String,
    ) -> Result<TokenizerInfo, crate::error::BindingError> {
        let encoded_vocab: Vec<Vec<u8>> = encoded_vocab.into_iter().map(String::into_bytes).collect();
        from_vocab_bytes_and_metadata(&encoded_vocab, &metadata)
    }

    /// The vocabulary type, as the integer `VocabType` value.
    #[bindings::export(Method)]
    pub fn vocab_type(&self) -> i32 {
        VocabType::from(self.inner.vocab_type()) as i32
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
    pub fn deserialize_json(json_string: String) -> Result<TokenizerInfo, crate::error::BindingError> {
        xgrammar::tokenizer::TokenizerInfo::deserialize_json(&json_string).map(TokenizerInfo::from).map_err(map_error)
    }

    /// Dumps tokenizer metadata (vocab type and prefix-space flag) as JSON.
    #[bindings::export(Method)]
    pub fn dump_metadata(&self) -> String {
        self.inner.dump_metadata()
    }

    /// Detects tokenizer metadata from a Hugging Face backend JSON string.
    #[bindings::export(Method(Factory))]
    pub fn _detect_metadata_from_hf(backend_str: String) -> Result<String, crate::error::BindingError> {
        xgrammar::tokenizer::TokenizerInfo::detect_metadata_from_hf(&backend_str).map_err(map_error)
    }
}

/// PyO3-only factories taking each vocabulary token as Python `str` (its UTF-8 bytes) or `bytes`
/// (exact bytes), as upstream accepts `List[Union[bytes, str]]`.
#[cfg(feature = "bindings-pyo3")]
mod tokenizer_info_pyo3_ext {
    use pyo3::{
        exceptions::PyTypeError,
        prelude::*,
        types::{PyBytes, PyString},
    };

    use super::{TokenizerInfo, from_vocab_bytes, from_vocab_bytes_and_metadata};

    fn token_bytes(tokens: Vec<Bound<'_, PyAny>>) -> PyResult<Vec<Vec<u8>>> {
        tokens
            .iter()
            .map(|token| {
                if let Ok(bytes) = token.cast::<PyBytes>() {
                    Ok(bytes.as_bytes().to_vec())
                } else if let Ok(text) = token.cast::<PyString>() {
                    Ok(text.to_cow()?.as_bytes().to_vec())
                } else {
                    Err(PyTypeError::new_err("each vocabulary token must be str or bytes"))
                }
            })
            .collect()
    }

    #[pyo3::pymethods]
    impl TokenizerInfo {
        #[staticmethod]
        #[pyo3(name = "_from_vocab_tokens")]
        fn from_vocab_tokens_py(
            encoded_vocab: Vec<Bound<'_, PyAny>>,
            vocab_type: i32,
            vocab_size: Option<i32>,
            stop_token_ids: Option<Vec<i32>>,
            add_prefix_space: bool,
        ) -> PyResult<Self> {
            from_vocab_bytes(&token_bytes(encoded_vocab)?, vocab_type, vocab_size, stop_token_ids, add_prefix_space)
        }

        #[staticmethod]
        #[pyo3(name = "_from_vocab_tokens_and_metadata")]
        fn from_vocab_tokens_and_metadata_py(
            encoded_vocab: Vec<Bound<'_, PyAny>>,
            metadata: &str,
        ) -> PyResult<Self> {
            from_vocab_bytes_and_metadata(&token_bytes(encoded_vocab)?, metadata)
        }
    }
}
