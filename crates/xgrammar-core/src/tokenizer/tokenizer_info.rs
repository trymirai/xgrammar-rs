//! Tokenizer metadata for grammar-guided masking — a port of `TokenizerInfo` in
//! `cpp/tokenizer_info.cc`.
//!
//! Decodes the raw vocabulary to byte strings, classifies stop/special tokens, and builds the
//! sorted-vocabulary pseudo-trie (`sorted_decoded_vocab` + `trie_subtree_nodes_range`) the
//! matcher walks when computing token bitmasks.

use serde_json::{Value, json};

use super::{token_decoder::decode_token, vocab_type::VocabType};
use crate::{config::SERIALIZATION_VERSION, grammar::DeserializeError};

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

/// Decoded vocabulary plus the derived structures used during constrained decoding.
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
}

impl TokenizerInfo {
    /// Builds tokenizer info from an encoded vocabulary.
    ///
    /// `vocab_size` defaults to the vocabulary length; ids past the end are padding (special)
    /// tokens. `stop_token_ids`, when `None`, are auto-detected from [`DETECTION_STOP_TOKENS`].
    #[must_use]
    pub fn new(
        encoded_vocab: &[String],
        vocab_type: VocabType,
        vocab_size: Option<i32>,
        stop_token_ids: Option<Vec<i32>>,
        add_prefix_space: bool,
    ) -> Self {
        let vocab_size = vocab_size.unwrap_or(encoded_vocab.len() as i32);
        let mut decoded_vocab: Vec<Vec<u8>> =
            Vec::with_capacity(encoded_vocab.len());
        let mut sorted_decoded_vocab: Vec<(i32, Vec<u8>)> = Vec::new();
        let mut stop_ids: Vec<i32> = Vec::new();
        let mut special_ids: Vec<i32> = Vec::new();

        for (i, encoded) in encoded_vocab.iter().enumerate() {
            let id = i as i32;
            let token = decode_token(encoded, vocab_type);
            let is_stop = match &stop_token_ids {
                None => DETECTION_STOP_TOKENS
                    .iter()
                    .any(|s| s.as_bytes() == token.as_slice()),
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

        let mut token_id_to_sorted_vocab_index =
            vec![-1i32; vocab_size as usize];
        for (i, (id, _)) in sorted_decoded_vocab.iter().enumerate() {
            token_id_to_sorted_vocab_index[*id as usize] = i as i32;
        }

        let trie_subtree_nodes_range = build_trie_ranges(&sorted_decoded_vocab);

        Self {
            vocab_type,
            vocab_size,
            add_prefix_space,
            decoded_vocab,
            sorted_decoded_vocab,
            trie_subtree_nodes_range,
            stop_token_ids: stop_ids,
            special_token_ids: special_ids,
            token_id_to_sorted_vocab_index,
        }
    }

    /// Builds tokenizer info from an encoded vocabulary and a JSON metadata string with keys
    /// `vocab_type`, `vocab_size`, `add_prefix_space`, and optional `stop_token_ids`.
    ///
    /// # Errors
    /// Returns a message if the metadata JSON is malformed or has an invalid `vocab_type`.
    pub fn from_vocab_and_metadata(
        encoded_vocab: &[String],
        metadata: &str,
    ) -> Result<Self, String> {
        let meta: serde_json::Value = serde_json::from_str(metadata)
            .map_err(|e| format!("invalid metadata json: {e}"))?;
        let vocab_type = VocabType::try_from(
            meta["vocab_type"].as_i64().ok_or("metadata missing vocab_type")?,
        )
        .map_err(|e| e.to_string())?;
        let vocab_size = meta["vocab_size"].as_i64().map(|v| v as i32);
        let add_prefix_space =
            meta["add_prefix_space"].as_bool().unwrap_or(false);
        let stop_token_ids =
            meta.get("stop_token_ids").and_then(|v| v.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_i64().map(|n| n as i32))
                    .collect::<Vec<i32>>()
            });
        Ok(Self::new(
            encoded_vocab,
            vocab_type,
            vocab_size,
            stop_token_ids,
            add_prefix_space,
        ))
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
}

impl TokenizerInfo {
    /// Rebuilds tokenizer info from already-decoded parts (used by deserialization),
    /// recomputing the sorted vocabulary, trie ranges, and reverse index.
    #[must_use]
    fn from_decoded_parts(
        decoded_vocab: Vec<Vec<u8>>,
        vocab_type: VocabType,
        vocab_size: i32,
        add_prefix_space: bool,
        stop_token_ids: Vec<i32>,
        special_token_ids: Vec<i32>,
    ) -> Self {
        let mut sorted_decoded_vocab: Vec<(i32, Vec<u8>)> = Vec::new();
        for (i, tok) in decoded_vocab.iter().enumerate() {
            let id = i as i32;
            if !stop_token_ids.contains(&id) && !special_token_ids.contains(&id)
            {
                sorted_decoded_vocab.push((id, tok.clone()));
            }
        }
        sorted_decoded_vocab.sort_by(|a, b| a.1.cmp(&b.1));
        let mut token_id_to_sorted_vocab_index =
            vec![-1i32; vocab_size as usize];
        for (i, (id, _)) in sorted_decoded_vocab.iter().enumerate() {
            token_id_to_sorted_vocab_index[*id as usize] = i as i32;
        }
        let trie_subtree_nodes_range = build_trie_ranges(&sorted_decoded_vocab);
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
        }
    }

    /// Serializes the tokenizer info to its `"v11"` JSON form (byte strings are encoded as
    /// Latin-1, matching the C++).
    #[must_use]
    pub fn serialize_json(&self) -> String {
        let decoded: Vec<String> =
            self.decoded_vocab.iter().map(|b| bytes_to_latin1(b)).collect();
        let sorted: Vec<Value> = self
            .sorted_decoded_vocab
            .iter()
            .map(|(id, tok)| json!([id, bytes_to_latin1(tok)]))
            .collect();
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
        serde_json::to_string(&obj)
            .expect("tokenizer info JSON serialization never fails")
    }

    /// Deserializes tokenizer info from its `"v11"` JSON form.
    ///
    /// # Errors
    /// Returns [`DeserializeError`] for invalid JSON, a version mismatch, or a malformed body.
    pub fn deserialize_json(
        json_str: &str
    ) -> Result<TokenizerInfo, DeserializeError> {
        let value: Value = serde_json::from_str(json_str)
            .map_err(|e| DeserializeError::InvalidJson(e.to_string()))?;
        match value.get("__VERSION__").and_then(Value::as_str) {
            Some(SERIALIZATION_VERSION) => {},
            Some(other) => {
                return Err(DeserializeError::Version {
                    expected: SERIALIZATION_VERSION.to_owned(),
                    got: other.to_owned(),
                });
            },
            None => {
                return Err(DeserializeError::Format(
                    "missing __VERSION__".to_owned(),
                ));
            },
        }
        let field = |name: &str| {
            value.get(name).ok_or_else(|| {
                DeserializeError::Format(format!("missing {name}"))
            })
        };
        let vocab_type =
            VocabType::try_from(field("vocab_type")?.as_i64().ok_or_else(
                || DeserializeError::Format("vocab_type".to_owned()),
            )?)
            .map_err(|e| DeserializeError::Format(e.to_string()))?;
        let vocab_size = field("vocab_size")?
            .as_i64()
            .ok_or_else(|| DeserializeError::Format("vocab_size".to_owned()))?
            as i32;
        let add_prefix_space =
            field("add_prefix_space")?.as_bool().unwrap_or(false);
        let stop_token_ids = i32_array(field("stop_token_ids")?)?;
        let special_token_ids = i32_array(field("special_token_ids")?)?;
        let decoded_vocab: Vec<Vec<u8>> = field("decoded_vocab")?
            .as_array()
            .ok_or_else(|| {
                DeserializeError::Format("decoded_vocab".to_owned())
            })?
            .iter()
            .map(|v| {
                v.as_str().map(latin1_to_bytes).ok_or_else(|| {
                    DeserializeError::Format("decoded token".to_owned())
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self::from_decoded_parts(
            decoded_vocab,
            vocab_type,
            vocab_size,
            add_prefix_space,
            stop_token_ids,
            special_token_ids,
        ))
    }
}

/// Encodes a byte string as a Latin-1 string (each byte → the same-valued codepoint).
fn bytes_to_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// Decodes a Latin-1 string back into its byte string (each codepoint → its low byte).
fn latin1_to_bytes(s: &str) -> Vec<u8> {
    s.chars().map(|c| c as u8).collect()
}

fn i32_array(value: &Value) -> Result<Vec<i32>, DeserializeError> {
    value
        .as_array()
        .ok_or_else(|| DeserializeError::Format("expected array".to_owned()))?
        .iter()
        .map(|v| {
            v.as_i64().map(|n| n as i32).ok_or_else(|| {
                DeserializeError::Format("expected integer".to_owned())
            })
        })
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
    needle.len() <= haystack.len()
        && haystack.windows(needle.len()).any(|w| w == needle)
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
