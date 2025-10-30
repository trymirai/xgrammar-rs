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
        let ffi_box = unsafe { Box::from_raw(raw) };
        let ffi_pin = unsafe { Pin::new_unchecked(ffi_box) };
        Ok(Self {
            inner: ffi_pin,
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

// ---- Hugging Face tokenizers integration (feature-gated) ----
//
// The following helpers mirror Python's TokenizerInfo utilities:
// - _is_tiktoken_tokenizer
// - _is_sentencepiece_tokenizer
// - from_huggingface
//
// They are adapted to Rust and the `tokenizers` crate. Detection is heuristic-based
// using the vocabulary content, since Rust does not expose the Python runtime types.

#[cfg(feature = "tokenizers")]
impl TokenizerInfo {
    #[inline]
    fn extract_ordered_vocab(tokenizer: &tokenizers::Tokenizer) -> Vec<String> {
        let mut pairs: Vec<(usize, String)> = tokenizer
            .get_vocab(true)
            .into_iter()
            .map(|(tok, id)| (id as usize, tok))
            .collect();
        pairs.sort_by_key(|(id, _)| *id);
        pairs.into_iter().map(|(_, tok)| tok).collect()
    }

    /// Heuristically detect whether a tokenizer resembles a tiktoken-style tokenizer.
    ///
    /// In Python this checks `isinstance(tokenizer.tokenizer, tiktoken.Encoding)` or whether
    /// the vocab filename contains "tiktoken". In Rust we do not have those runtime types,
    /// so we approximate: if the vocabulary does NOT contain typical markers of
    /// SentencePiece ("▁"), Byte-level GPT-2 ("Ġ"), or ByteFallback (tokens like "<0x1B>"),
    /// we consider it RAW (tiktoken-like).
    pub fn _is_tiktoken_tokenizer(tokenizer: &tokenizers::Tokenizer) -> bool {
        let vocab = tokenizer.get_vocab(true);
        let mut has_sentencepiece_marker = false; // '▁'
        let mut has_bytelevel_marker = false; // 'Ġ'
        let mut has_bytefallback_marker = false; // tokens like "<0x..>"
        for token in vocab.keys() {
            if !has_sentencepiece_marker && token.contains('▁') {
                has_sentencepiece_marker = true;
            }
            if !has_bytelevel_marker && token.contains('Ġ') {
                has_bytelevel_marker = true;
            }
            if !has_bytefallback_marker
                && token.starts_with("<0x")
                && token.ends_with('>')
            {
                has_bytefallback_marker = true;
            }
            if has_sentencepiece_marker
                || has_bytelevel_marker
                || has_bytefallback_marker
            {
                break;
            }
        }
        !(has_sentencepiece_marker
            || has_bytelevel_marker
            || has_bytefallback_marker)
    }

    /// Heuristically detect whether a tokenizer is SentencePiece-based.
    ///
    /// In Python this checks for a `sentencepiece.SentencePieceProcessor`. Here we look for
    /// typical SentencePiece marker "▁" in the vocabulary. This is a best-effort heuristic
    /// and may not be perfect for all models.
    pub fn _is_sentencepiece_tokenizer(
        tokenizer: &tokenizers::Tokenizer
    ) -> bool {
        let vocab = tokenizer.get_vocab(true);
        vocab.keys().any(|tok| tok.contains('▁'))
    }

    /// Construct from a `tokenizers::Tokenizer` with explicit options, preserving tokenizer indexing.
    ///
    /// This matches Python's constructor path where `encoded_vocab` is built by id order and
    /// `vocab_size` may be larger than the tokenizer's vocab (model padding), with special ids
    /// reserved in the tail range.
    pub fn from_tokenizers_with_options(
        tokenizer: &tokenizers::Tokenizer,
        vocab_type: VocabType,
        vocab_size: Option<usize>,
        stop_token_ids: Option<&[i32]>,
        add_prefix_space: bool,
    ) -> Self {
        let ordered = Self::extract_ordered_vocab(tokenizer);
        let stop: Option<Box<[i32]>> =
            stop_token_ids.map(|s| s.to_vec().into_boxed_slice());
        Self::new_with_vocab_size(
            &ordered,
            vocab_type,
            vocab_size,
            &stop,
            add_prefix_space,
        )
    }

    /// Convenience: RAW vocab, detected size, no stops, no prefix space.
    pub fn from_tokenizers_simple(tokenizer: &tokenizers::Tokenizer) -> Self {
        Self::from_tokenizers_with_options(
            tokenizer,
            VocabType::RAW,
            None,
            None,
            false,
        )
    }

    /// Construct the tokenizer info from a Hugging Face `tokenizers::Tokenizer`.
    ///
    /// This mirrors Python's `TokenizerInfo.from_huggingface` and automatically detects
    /// vocab type and `add_prefix_space` using vocabulary heuristics. Provide `vocab_size`
    /// if the model's vocab differs from the tokenizer's (padding or reduced vocab). Pass
    /// `stop_token_ids` if you want to override auto-detection (Rust tokenizers do not carry
    /// EOS id consistently across models).
    pub fn from_huggingface(
        tokenizer: &tokenizers::Tokenizer,
        vocab_size: Option<usize>,
        stop_token_ids: Option<&[i32]>,
    ) -> Self {
        use crate::VocabType;

        // Heuristics for vocab type and prefix-space behavior
        let vocab = tokenizer.get_vocab(true);
        let has_bytefallback_marker =
            vocab.keys().any(|t| t.starts_with("<0x") && t.ends_with('>'));
        let has_sentencepiece_marker = vocab.keys().any(|t| t.contains('▁'));
        let has_bytelevel_marker = vocab.keys().any(|t| t.contains('Ġ'));

        let (vocab_type, add_prefix_space) = if has_bytefallback_marker {
            (VocabType::BYTE_FALLBACK, true)
        } else if has_sentencepiece_marker {
            // Some SentencePiece tokenizers can still be RAW; however, in Python they default
            // to add_prefix_space=True for SP. If the vocab also contains "<0x..>" we already
            // categorized as BYTE_FALLBACK above.
            (VocabType::RAW, true)
        } else if has_bytelevel_marker {
            (VocabType::BYTE_LEVEL, false)
        } else {
            (VocabType::RAW, false)
        };

        // Build with explicit options, preserving token id ordering
        Self::from_tokenizers_with_options(
            tokenizer,
            vocab_type,
            vocab_size,
            stop_token_ids,
            add_prefix_space,
        )
    }
}
