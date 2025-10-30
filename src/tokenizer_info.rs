use std::pin::Pin;

use autocxx::prelude::*;

use crate::{FFITokenizerInfo, VocabType, cxx_utils};

type StopTokenIds = Option<Box<[i32]>>;

/// TokenizerInfo contains the vocabulary, its type, and metadata used by the
/// grammar-guided generation.
///
/// Notes:
/// - Tokens may be encoded differently depending on `VocabType` (e.g. ByteFallback
///   uses "<0x1B>", ByteLevel uses unicode mappings). This wrapper exposes the
///   decoded vocabulary in the same form as the original text via
///   `decoded_vocab_as_bytes`.
/// - Some models pad their vocab size to a multiple of 32 or similar. If your
///   model's vocab size differs from `encoded_vocab.len()`, use
///   `new_with_vocab_size` to pass the model's vocab size so bitmask sizes are
///   computed correctly.
pub struct TokenizerInfo {
    inner: Pin<Box<FFITokenizerInfo>>,
}

impl TokenizerInfo {
    /// Construct a TokenizerInfo with vocab size derived from `encoded_vocab`.
    ///
    /// If the model's vocab size differs from `encoded_vocab.len()`, prefer
    /// `new_with_vocab_size`.
    pub fn new<T: AsRef<str>>(
        encoded_vocab: &[T],
        vocab_type: VocabType,
        stop_token_ids: &StopTokenIds,
        add_prefix_space: bool,
    ) -> Self {
        Self::new_with_vocab_size(
            encoded_vocab,
            vocab_type,
            Some(encoded_vocab.len()),
            stop_token_ids,
            add_prefix_space,
        )
    }

    /// Construct a TokenizerInfo with an explicit model `vocab_size`.
    ///
    /// Use this when the model's vocab size (e.g., padded to a multiple of 32)
    /// differs from the tokenizer's `encoded_vocab.len()`. Indices in the range
    /// `[encoded_vocab.len(), vocab_size)` are treated as special/reserved.
    pub fn new_with_vocab_size<T: AsRef<str>>(
        encoded_vocab: &[T],
        vocab_type: VocabType,
        vocab_size: Option<usize>,
        stop_token_ids: &StopTokenIds,
        add_prefix_space: bool,
    ) -> Self {
        let mut cxx_vec = cxx_utils::new_string_vector();
        {
            let mut cxx_vec_pin = cxx_vec.pin_mut();
            cxx_utils::string_vec_reserve(
                cxx_vec_pin.as_mut(),
                encoded_vocab.len(),
            );
            for string in encoded_vocab.iter() {
                let bytes = string.as_ref().as_bytes();
                unsafe {
                    cxx_utils::string_vec_push_bytes(
                        cxx_vec_pin.as_mut(),
                        bytes.as_ptr() as *const i8,
                        bytes.len(),
                    );
                }
            }
        }
        let (has_vocab_size, vocab_size_i32) = match vocab_size {
            Some(sz) => (true, sz as i32),
            None => (false, 0i32),
        };

        let (has_stop_ids, stop_ptr, stop_len) = match stop_token_ids.as_ref() {
            Some(slice) if !slice.is_empty() => {
                (true, slice.as_ptr(), slice.len())
            },
            _ => (false, std::ptr::null(), 0usize),
        };

        let ffi_obj = unsafe {
            cxx_utils::make_tokenizer_info(
                cxx_vec.as_ref().unwrap(),
                vocab_type,
                has_vocab_size,
                vocab_size_i32,
                has_stop_ids,
                stop_ptr,
                stop_len,
                add_prefix_space,
            )
        };

        Self {
            inner: ffi_obj.within_box(),
        }
    }

    /// Construct TokenizerInfo from encoded vocab (bytes) and a metadata JSON
    /// string produced by `dump_metadata`.
    pub fn from_vocab_and_metadata_bytes<I, B>(
        encoded_vocab: I,
        metadata: &str,
    ) -> Self
    where
        I: IntoIterator<Item = B>,
        B: AsRef<[u8]>,
    {
        let mut cxx_vec = cxx_utils::new_string_vector();
        {
            let mut cxx_vec_pin = cxx_vec.pin_mut();
            for string in encoded_vocab.into_iter() {
                let bytes = string.as_ref();
                unsafe {
                    cxx_utils::string_vec_push_bytes(
                        cxx_vec_pin.as_mut(),
                        bytes.as_ptr() as *const i8,
                        bytes.len(),
                    );
                }
            }
        }

        cxx::let_cxx_string!(metadata_cxx = metadata);
        let ffi_pin = FFITokenizerInfo::FromVocabAndMetadata(
            cxx_vec.as_ref().unwrap(),
            &metadata_cxx,
        )
        .within_box();
        Self {
            inner: ffi_pin,
        }
    }

    /// The type of the vocabulary.
    pub fn vocab_type(&self) -> VocabType {
        self.inner.GetVocabType()
    }

    /// The size of the vocabulary.
    pub fn vocab_size(&self) -> usize {
        usize::try_from(self.inner.GetVocabSize().0)
            .expect("GetVocabSize returned a negative value")
    }

    /// Whether the tokenizer will prepend a space before the text in the tokenization
    /// process.
    pub fn add_prefix_space(&self) -> bool {
        self.inner.GetAddPrefixSpace()
    }

    /// The decoded vocabulary of the tokenizer. This converts tokens in the
    /// LLM's vocabulary back to the original text form (e.g., ByteFallback
    /// "<0x1B>" -> "\u001b").
    pub fn decoded_vocab(&self) -> Box<[Box<[u8]>]> {
        let cxx_vec = self.inner.GetDecodedVocab();
        let mut result: Vec<Box<[u8]>> = Vec::with_capacity(cxx_vec.len());
        for cxx_string in cxx_vec.iter() {
            result.push(
                cxx_string
                    .to_string_lossy()
                    .into_owned()
                    .into_bytes()
                    .into_boxed_slice(),
            );
        }
        result.into_boxed_slice()
    }

    /// Stop token ids.
    pub fn stop_token_ids(&self) -> Box<[i32]> {
        let cxx_vec = self.inner.GetStopTokenIds();
        cxx_vec.iter().copied().collect::<Vec<_>>().into_boxed_slice()
    }

    /// The special token ids. Special tokens include control tokens, reserved tokens,
    /// padded tokens, etc. Now it is automatically detected from the vocabulary.
    pub fn special_token_ids(&self) -> Box<[i32]> {
        let cxx_vec = self.inner.GetSpecialTokenIds();
        cxx_vec.iter().copied().collect::<Vec<_>>().into_boxed_slice()
    }

    /// Dump the metadata of the tokenizer to a json string. It can be used to construct the
    /// tokenizer info from the vocabulary and the metadata string.
    pub fn dump_metadata(&self) -> String {
        self.inner.DumpMetadata().to_string()
    }

    /// Serialize the tokenizer info to a JSON string.
    pub fn serialize_json(&self) -> String {
        self.inner.SerializeJSON().to_string()
    }

    /// Deserialize a `TokenizerInfo` from a JSON string.
    ///
    /// Returns
    /// - `Ok(TokenizerInfo)` on success
    /// - `Err(String)` when deserialization fails due to any of the following:
    ///   - invalid JSON syntax
    ///   - schema/format mismatch with `TokenizerInfo` serialization
    ///   - serialization version mismatch (via the `__VERSION__` field)
    /// The error string mirrors the C++ exception message.
    pub fn deserialize_json(json: &str) -> Result<Self, String> {
        cxx::let_cxx_string!(json_cxx = json);
        cxx::let_cxx_string!(error_out_cxx = "");
        let uptr = unsafe {
            cxx_utils::tokenizer_info_deserialize_json_or_error(
                &json_cxx,
                error_out_cxx.as_mut().get_unchecked_mut(),
            )
        };
        if uptr.is_null() {
            return Err(error_out_cxx.to_string());
        }
        let raw = uptr.into_raw();
        let boxed = unsafe { Box::from_raw(raw) };
        let pinned = unsafe { Pin::new_unchecked(boxed) };
        Ok(Self {
            inner: pinned,
        })
    }

    pub(crate) fn ffi_ref(&self) -> &FFITokenizerInfo {
        self.inner.as_ref().get_ref()
    }

    pub(crate) fn from_pinned_ffi(inner: Pin<Box<FFITokenizerInfo>>) -> Self {
        Self {
            inner,
        }
    }
}
