//! The tokenizer info class to handle tokenizer information for grammar-guided generation.
//!
//! Although some tokenizers encode tokens in a special format (e.g. `"<0x1B>"` for `"\u001b"`
//! in the byte-fallback tokenizer, and `"Ġ"` for `" "` in the byte-level BPE tokenizer),
//! [`TokenizerInfo`] always decodes the vocabulary to the original format (e.g. `"\u001b"` and
//! `" "`).
//!
//! Some models (e.g. Phi-3 and Deepseek-V2) may pad the vocabulary to a multiple of 32. In this
//! case, the model's `vocab_size` is larger than the tokenizer's vocabulary size. Pass the
//! model's `vocab_size` to the constructor, because this information is used to determine the
//! size of the token mask.

use serde_json::{Value, json};

use super::{error::TokenizerError, token_decoder::decode_token_bytes, vocab_type::VocabType};
use crate::{
    config::SERIALIZATION_VERSION,
    grammar::DeserializeError,
    support::{byte_to_latin1, latin1_to_bytes, parse_json},
};

/// Tokens whose presence marks a stop token when explicit ids are not supplied.
const DETECTION_STOP_TOKENS: [&str; 8] = [
    "</s>",
    "<|end_of_text|>",
    "<|eot_id|>",
    "<|endoftext|>",
    "<eos>",
    "<|eos|>",
    "<end_of_turn>",
    "<｜end▁of▁sentence｜>",
];

/// The tokenizer info contains the vocabulary, the type of the vocabulary, and necessary
/// information for grammar-guided generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenizerInfo {
    vocab_type: VocabType,
    vocab_size: i32,
    add_prefix_space: bool,
    decoded_vocab: Vec<Vec<u8>>,
    sorted_decoded_vocab: Vec<(i32, Vec<u8>)>,
    trie_subtree_nodes_range: Vec<i32>,
    stop_token_ids: Vec<i32>,
    special_token_ids: Vec<i32>,
    token_id_to_sorted_vocab_index: Vec<i32>,
    /// Unicode codepoint counts for the sorted decoded vocabulary.
    token_char_counts: Vec<i32>,
    /// The maximum of `token_char_counts`.
    max_token_chars: i32,
}

impl TokenizerInfo {
    /// Constructs the tokenizer info.
    ///
    /// `encoded_vocab` is the encoded vocabulary of the tokenizer.
    /// `vocab_type` is the type of the vocabulary; see [`VocabType`].
    /// `vocab_size`, if not provided, defaults to `encoded_vocab.len()`.
    /// `stop_token_ids`, if not provided, are auto-detected (but may not be correct).
    /// `add_prefix_space` indicates whether the tokenizer prepends a space before text during
    /// tokenization.
    ///
    /// # Errors
    /// Returns [`TokenizerError::VocabSizeTooSmall`] if `vocab_size` is smaller than the
    /// vocabulary.
    pub fn new(
        encoded_vocab: &[String],
        vocab_type: VocabType,
        vocab_size: Option<i32>,
        stop_token_ids: Option<Vec<i32>>,
        add_prefix_space: bool,
    ) -> Result<Self, TokenizerError> {
        let encoded_vocab = encoded_vocab.iter().map(|token| token.as_bytes().to_vec()).collect::<Vec<_>>();
        Self::new_from_bytes(&encoded_vocab, vocab_type, vocab_size, stop_token_ids, add_prefix_space)
    }

    /// Builds tokenizer info from an encoded vocabulary containing arbitrary bytes.
    ///
    /// # Errors
    /// Returns [`TokenizerError::VocabSizeTooSmall`] if `vocab_size` is smaller than the
    /// vocabulary.
    pub fn new_from_bytes(
        encoded_vocab: &[Vec<u8>],
        vocab_type: VocabType,
        vocab_size: Option<i32>,
        stop_token_ids: Option<Vec<i32>>,
        add_prefix_space: bool,
    ) -> Result<Self, TokenizerError> {
        let vocab_size = vocab_size.unwrap_or(encoded_vocab.len() as i32);
        // vocab_size only ever pads the encoded vocab with special ids; a smaller value would
        // leave token ids without a slot.
        if vocab_size < encoded_vocab.len() as i32 {
            return Err(TokenizerError::VocabSizeTooSmall {
                vocab_size,
                num_tokens: encoded_vocab.len(),
            });
        }
        let mut decoded_vocab: Vec<Vec<u8>> = Vec::with_capacity(encoded_vocab.len());
        let mut sorted_decoded_vocab: Vec<(i32, Vec<u8>)> = Vec::new();
        let mut stop_ids: Vec<i32> = Vec::new();
        let mut special_ids: Vec<i32> = Vec::new();

        for (i, encoded) in encoded_vocab.iter().enumerate() {
            let id = i as i32;
            let token = decode_token_bytes(encoded, vocab_type);
            let is_stop = match &stop_token_ids {
                None => DETECTION_STOP_TOKENS.iter().any(|s| s.as_bytes() == token.as_slice()),
                Some(ids) => ids.contains(&id),
            };
            if is_stop {
                stop_ids.push(id);
            } else if token.is_empty() {
                // The only special token is the empty decoded token.
                special_ids.push(id);
            } else {
                sorted_decoded_vocab.push((id, token.clone()));
            }
            decoded_vocab.push(token);
        }
        for id in encoded_vocab.len() as i32..vocab_size {
            special_ids.push(id);
        }

        sorted_decoded_vocab.sort_by(|a, b| a.1.cmp(&b.1));
        let trie_subtree_nodes_range = build_trie_ranges(&sorted_decoded_vocab);
        Ok(Self::with_derived_data(
            vocab_type,
            vocab_size,
            add_prefix_space,
            decoded_vocab,
            sorted_decoded_vocab,
            trie_subtree_nodes_range,
            stop_ids,
            special_ids,
        ))
    }

    /// Assembles tokenizer info and builds the derived per-token arrays (the reverse index and
    /// the codepoint counts), which are not serialized.
    #[allow(clippy::too_many_arguments)]
    fn with_derived_data(
        vocab_type: VocabType,
        vocab_size: i32,
        add_prefix_space: bool,
        decoded_vocab: Vec<Vec<u8>>,
        sorted_decoded_vocab: Vec<(i32, Vec<u8>)>,
        trie_subtree_nodes_range: Vec<i32>,
        stop_token_ids: Vec<i32>,
        special_token_ids: Vec<i32>,
    ) -> Self {
        let mut token_id_to_sorted_vocab_index = vec![-1i32; vocab_size as usize];
        for (i, (id, _)) in sorted_decoded_vocab.iter().enumerate() {
            token_id_to_sorted_vocab_index[*id as usize] = i as i32;
        }
        let token_char_counts: Vec<i32> = sorted_decoded_vocab
            .iter()
            .map(|(_, token)| token.iter().filter(|&&byte| (byte & 0xC0) != 0x80).count() as i32)
            .collect();
        let max_token_chars = token_char_counts.iter().copied().max().unwrap_or(0);
        Self {
            vocab_type,
            vocab_size,
            add_prefix_space,
            decoded_vocab,
            sorted_decoded_vocab,
            trie_subtree_nodes_range,
            stop_token_ids,
            special_token_ids,
            token_id_to_sorted_vocab_index,
            token_char_counts,
            max_token_chars,
        }
    }

    /// Builds tokenizer info from an encoded vocabulary and a JSON metadata string with keys
    /// `vocab_type`, `vocab_size`, `add_prefix_space`, and optional `stop_token_ids`.
    ///
    /// # Errors
    /// Returns [`TokenizerError`] if the metadata JSON is malformed, has an invalid
    /// `vocab_type`, or a `vocab_size` smaller than the vocabulary.
    pub fn from_vocab_and_metadata(
        encoded_vocab: &[String],
        metadata: &str,
    ) -> Result<Self, TokenizerError> {
        let encoded_vocab = encoded_vocab.iter().map(|token| token.as_bytes().to_vec()).collect::<Vec<_>>();
        Self::from_vocab_and_metadata_bytes(&encoded_vocab, metadata)
    }

    /// Builds tokenizer info from an arbitrary-byte vocabulary and JSON metadata.
    ///
    /// # Errors
    /// See [`Self::from_vocab_and_metadata`].
    pub fn from_vocab_and_metadata_bytes(
        encoded_vocab: &[Vec<u8>],
        metadata: &str,
    ) -> Result<Self, TokenizerError> {
        let meta = parse_json(metadata)
            .map_err(|e| TokenizerError::InvalidMetadata(format!("Failed to parse metadata: {e}")))?;
        let vocab_type = VocabType::try_from(
            meta["vocab_type"]
                .as_i64()
                .ok_or_else(|| TokenizerError::InvalidMetadata("metadata missing vocab_type".to_owned()))?,
        )
        .map_err(|e| TokenizerError::InvalidMetadata(e.to_string()))?;
        let vocab_size = meta["vocab_size"].as_i64().map(|v| v as i32);
        let add_prefix_space = meta["add_prefix_space"].as_bool().unwrap_or(false);
        let stop_token_ids = meta
            .get("stop_token_ids")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_i64().map(|n| n as i32)).collect::<Vec<i32>>());
        Self::new_from_bytes(encoded_vocab, vocab_type, vocab_size, stop_token_ids, add_prefix_space)
    }

    /// Unicode codepoint counts of the sorted decoded vocabulary.
    #[must_use]
    pub fn token_char_counts(&self) -> &[i32] {
        &self.token_char_counts
    }

    /// The largest number of codepoints in a single token.
    #[must_use]
    pub fn max_token_chars(&self) -> i32 {
        self.max_token_chars
    }

    /// The vocabulary encoding type.
    #[must_use]
    pub fn vocab_type(&self) -> VocabType {
        self.vocab_type
    }

    /// The vocabulary size (including padding tokens).
    #[must_use]
    pub fn vocab_size(&self) -> i32 {
        self.vocab_size
    }

    /// Whether a prefix space is added during tokenization.
    #[must_use]
    pub fn add_prefix_space(&self) -> bool {
        self.add_prefix_space
    }

    /// The decoded byte string of each token id.
    #[must_use]
    pub fn decoded_vocab(&self) -> &[Vec<u8>] {
        &self.decoded_vocab
    }

    /// The stop token ids.
    #[must_use]
    pub fn stop_token_ids(&self) -> &[i32] {
        &self.stop_token_ids
    }

    /// The special token ids (masked out during generation).
    #[must_use]
    pub fn special_token_ids(&self) -> &[i32] {
        &self.special_token_ids
    }

    /// All `(id, decoded)` pairs, sorted lexicographically by decoded bytes (stop/special
    /// tokens excluded).
    #[must_use]
    pub fn sorted_decoded_vocab(&self) -> &[(i32, Vec<u8>)] {
        &self.sorted_decoded_vocab
    }

    /// The pseudo-trie subtree ranges: entry `i`'s subtree spans `[i, range[i])`.
    #[must_use]
    pub fn trie_subtree_nodes_range(&self) -> &[i32] {
        &self.trie_subtree_nodes_range
    }

    /// Maps a token id to its index in [`Self::sorted_decoded_vocab`], or `-1`.
    #[must_use]
    pub fn token_id_to_sorted_vocab_index(&self) -> &[i32] {
        &self.token_id_to_sorted_vocab_index
    }

    /// Dumps the tokenizer metadata as a compact JSON string for compiled-grammar checks.
    #[must_use]
    pub fn dump_metadata(&self) -> String {
        serde_json::to_string(&self.metadata_value()).expect("tokenizer metadata serialization never fails")
    }

    /// Returns tokenizer metadata as JSON for compiled-grammar serialization.
    #[must_use]
    pub fn metadata_value(&self) -> Value {
        json!({
            "vocab_type": self.vocab_type as i32,
            "vocab_size": self.vocab_size,
            "add_prefix_space": self.add_prefix_space,
            "stop_token_ids": self.stop_token_ids,
        })
    }

    /// Returns an error when `metadata` does not match this tokenizer info.
    ///
    /// # Errors
    /// Returns [`DeserializeError::Format`] when a metadata field differs.
    pub fn check_metadata_match(
        &self,
        metadata: &Value,
    ) -> Result<(), DeserializeError> {
        let expected = self.metadata_value();
        for key in ["vocab_type", "vocab_size", "add_prefix_space", "stop_token_ids"] {
            if expected.get(key) != metadata.get(key) {
                return Err(DeserializeError::Format(format!("tokenizer metadata mismatch on {key}")));
            }
        }
        Ok(())
    }

    /// Detects tokenizer metadata from a Hugging Face backend JSON string.
    ///
    /// # Errors
    /// Returns an error if `backend_str` is not valid JSON.
    pub fn detect_metadata_from_hf(backend_str: &str) -> Result<String, String> {
        super::detect_metadata_from_hf(backend_str).map(|metadata| super::metadata_to_json(&metadata))
    }
}

#[cfg(feature = "tokenizers")]
impl TokenizerInfo {
    fn extract_ordered_vocab(
        tokenizer: &tokenizers::Tokenizer,
        vocab_size: Option<usize>,
    ) -> Vec<String> {
        let vocab = tokenizer.get_vocab(true);
        let tokenizer_vocab_size =
            vocab.values().copied().max().map_or(0, |max_id| max_id as usize + 1).max(vocab.len());
        let size = vocab_size.unwrap_or(tokenizer_vocab_size);
        let mut ordered = vec![String::new(); size];
        for (token, id) in vocab {
            if let Some(slot) = ordered.get_mut(id as usize) {
                *slot = token;
            }
        }
        ordered
    }

    /// Builds tokenizer info from a `tokenizers::Tokenizer` with explicit metadata.
    ///
    /// The vocabulary is reordered by token id, including any empty slots and optional model
    /// padding, so its indices exactly match the model's output dimension.
    #[must_use]
    pub fn from_tokenizers_with_options(
        tokenizer: &tokenizers::Tokenizer,
        vocab_type: VocabType,
        vocab_size: Option<usize>,
        stop_token_ids: Option<&[i32]>,
        add_prefix_space: bool,
    ) -> Self {
        let ordered = Self::extract_ordered_vocab(tokenizer, vocab_size);
        Self::new(
            &ordered,
            vocab_type,
            Some(ordered.len() as i32),
            stop_token_ids.map(<[i32]>::to_vec),
            add_prefix_space,
        )
        .expect("vocab_size equals the vocabulary length")
    }

    /// Builds tokenizer info from a Hugging Face tokenizer, automatically detecting its byte
    /// decoding and prefix-space behavior.
    ///
    /// # Errors
    /// Returns an error if the tokenizer backend cannot be serialized or analyzed.
    pub fn from_huggingface(
        tokenizer: &tokenizers::Tokenizer,
        vocab_size: Option<usize>,
        stop_token_ids: Option<&[i32]>,
    ) -> Result<Self, String> {
        let backend =
            tokenizer.to_string(false).map_err(|error| format!("failed to serialize tokenizer backend: {error}"))?;
        let metadata = super::detect_metadata_from_hf(&backend)?;
        Ok(Self::from_tokenizers_with_options(
            tokenizer,
            metadata.vocab_type,
            vocab_size,
            stop_token_ids,
            metadata.add_prefix_space,
        ))
    }
}

impl TokenizerInfo {
    /// Serializes the tokenizer info to its `"v16"` JSON form (byte strings are encoded as
    /// Latin-1, matching the C++).
    #[must_use]
    pub fn serialize_json(&self) -> String {
        let decoded: Vec<String> = self.decoded_vocab.iter().map(|b| byte_to_latin1(b)).collect();
        let sorted: Vec<Value> =
            self.sorted_decoded_vocab.iter().map(|(id, tok)| json!([id, byte_to_latin1(tok)])).collect();
        let obj = json!({
            "vocab_type": self.vocab_type as i32,
            "vocab_size": self.vocab_size,
            "add_prefix_space": self.add_prefix_space,
            "stop_token_ids": self.stop_token_ids,
            "special_token_ids": self.special_token_ids,
            "decoded_vocab": decoded,
            "sorted_decoded_vocab": sorted,
            "trie_subtree_nodes_range": self.trie_subtree_nodes_range,
            "__VERSION__": SERIALIZATION_VERSION,
        });
        serde_json::to_string(&obj).expect("tokenizer info JSON serialization never fails")
    }

    /// Deserializes tokenizer info from its `"v16"` JSON form. The fields are restored verbatim
    /// and validated; the derived per-token arrays are rebuilt.
    ///
    /// # Errors
    /// Returns [`DeserializeError`] for invalid JSON, a version mismatch, or a malformed body.
    pub fn deserialize_json(json_str: &str) -> Result<TokenizerInfo, DeserializeError> {
        let value = parse_json(json_str).map_err(DeserializeError::InvalidJson)?;
        match value.get("__VERSION__").and_then(Value::as_str) {
            Some(SERIALIZATION_VERSION) => {},
            Some(other) => {
                return Err(DeserializeError::Version {
                    expected: SERIALIZATION_VERSION.to_owned(),
                    got: other.to_owned(),
                });
            },
            None => {
                return Err(DeserializeError::Format("missing __VERSION__".to_owned()));
            },
        }
        let field = |name: &str| value.get(name).ok_or_else(|| DeserializeError::Format(format!("missing {name}")));
        let vocab_type = VocabType::try_from(
            field("vocab_type")?.as_i64().ok_or_else(|| DeserializeError::Format("vocab_type".to_owned()))?,
        )
        .map_err(|e| DeserializeError::Format(e.to_string()))?;
        let vocab_size =
            field("vocab_size")?.as_i64().ok_or_else(|| DeserializeError::Format("vocab_size".to_owned()))? as i32;
        let add_prefix_space = field("add_prefix_space")?.as_bool().unwrap_or(false);
        let stop_token_ids = i32_array(field("stop_token_ids")?)?;
        let special_token_ids = i32_array(field("special_token_ids")?)?;
        let format_error = |message: &str| DeserializeError::Format(message.to_owned());
        let latin1_bytes = |value: &Value| {
            value
                .as_str()
                .and_then(|token| latin1_to_bytes(token.as_bytes()).ok())
                .ok_or_else(|| format_error("decoded token"))
        };
        let decoded_vocab: Vec<Vec<u8>> = field("decoded_vocab")?
            .as_array()
            .ok_or_else(|| format_error("decoded_vocab"))?
            .iter()
            .map(latin1_bytes)
            .collect::<Result<_, _>>()?;
        let sorted_decoded_vocab: Vec<(i32, Vec<u8>)> = field("sorted_decoded_vocab")?
            .as_array()
            .ok_or_else(|| format_error("sorted_decoded_vocab"))?
            .iter()
            .map(|entry| match entry.as_array().map(Vec::as_slice) {
                Some([id, token]) => Ok((
                    id.as_i64().and_then(|id| i32::try_from(id).ok()).ok_or_else(|| format_error("token id"))?,
                    latin1_bytes(token)?,
                )),
                _ => Err(format_error("sorted_decoded_vocab entry")),
            })
            .collect::<Result<_, _>>()?;
        let trie_subtree_nodes_range = i32_array(field("trie_subtree_nodes_range")?)?;

        let num_tokens = decoded_vocab.len() as i64;
        if i64::from(vocab_size) < num_tokens {
            return Err(format_error(&format!(
                "vocab_size {vocab_size} is smaller than the number of tokens {num_tokens}"
            )));
        }
        if let Some((id, _)) = sorted_decoded_vocab.iter().find(|(id, _)| !(0..num_tokens).contains(&i64::from(*id))) {
            return Err(format_error(&format!("sorted_decoded_vocab contains token id {id} out of range")));
        }
        if trie_subtree_nodes_range.len() != sorted_decoded_vocab.len() {
            return Err(format_error("trie_subtree_nodes_range must have one entry per sorted token"));
        }
        if stop_token_ids.iter().chain(&special_token_ids).any(|id| !(0..vocab_size).contains(id)) {
            return Err(format_error("stop_token_ids or special_token_ids contains a token id out of range"));
        }
        Ok(Self::with_derived_data(
            vocab_type,
            vocab_size,
            add_prefix_space,
            decoded_vocab,
            sorted_decoded_vocab,
            trie_subtree_nodes_range,
            stop_token_ids,
            special_token_ids,
        ))
    }
}

fn i32_array(value: &Value) -> Result<Vec<i32>, DeserializeError> {
    value
        .as_array()
        .ok_or_else(|| DeserializeError::Format("expected array".to_owned()))?
        .iter()
        .map(|v| v.as_i64().map(|n| n as i32).ok_or_else(|| DeserializeError::Format("expected integer".to_owned())))
        .collect()
}

/// Whether `needle` occurs as a contiguous subsequence of `haystack` (byte `find != npos`).
fn byte_contains(
    haystack: &[u8],
    needle: &[u8],
) -> bool {
    if needle.is_empty() {
        return true;
    }
    needle.len() <= haystack.len() && haystack.windows(needle.len()).any(|w| w == needle)
}

/// Builds the pseudo-trie subtree ranges over the sorted vocabulary.
fn build_trie_ranges(sorted: &[(i32, Vec<u8>)]) -> Vec<i32> {
    let mut ranges = vec![0i32; sorted.len()];
    // Sorted indices of the currently-active prefixes.
    let mut prefix_stack: Vec<usize> = Vec::new();
    for (i, (_, token)) in sorted.iter().enumerate() {
        while let Some(&top) = prefix_stack.last() {
            if byte_contains(token, &sorted[top].1) {
                break;
            }
            ranges[top] = i as i32;
            prefix_stack.pop();
        }
        prefix_stack.push(i);
    }
    while let Some(top) = prefix_stack.pop() {
        ranges[top] = sorted.len() as i32;
    }
    ranges
}
