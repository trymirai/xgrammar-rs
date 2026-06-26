//! Hugging Face `tokenizer.json` metadata detection — a port of `HFTokenizerAnalyzer` in
//! `cpp/tokenizer_info.cc`.

use serde_json::Value;

use super::vocab_type::VocabType;

/// Metadata extracted from a Hugging Face tokenizer backend JSON string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfMetadata {
    /// Detected vocabulary encoding type.
    pub vocab_type: VocabType,
    /// Whether a prefix space is added during tokenization.
    pub add_prefix_space: bool,
}

/// Detects vocabulary type and prefix-space behavior from a HF backend JSON string.
#[must_use]
pub fn detect_metadata_from_hf(
    backend_str: &str
) -> Result<HfMetadata, String> {
    let value: Value = serde_json::from_str(backend_str)
        .map_err(|error| format!("invalid tokenizer backend JSON: {error}"))?;
    let object = value.as_object().ok_or_else(|| {
        "tokenizer backend JSON root must be an object".to_owned()
    })?;
    Ok(HfMetadata {
        vocab_type: detect_vocab_type(object),
        add_prefix_space: detect_add_prefix_space(object),
    })
}

/// Serializes [`HfMetadata`] to the JSON string expected by [`super::TokenizerInfo::from_vocab_and_metadata`].
#[must_use]
pub fn metadata_to_json(metadata: &HfMetadata) -> String {
    serde_json::json!({
        "vocab_type": metadata.vocab_type as i32,
        "add_prefix_space": metadata.add_prefix_space,
    })
    .to_string()
}

fn detect_vocab_type(object: &serde_json::Map<String, Value>) -> VocabType {
    let Some(decoder) = object.get("decoder").and_then(Value::as_object) else {
        return VocabType::Raw;
    };
    let Some(decoder_type) = decoder.get("type").and_then(Value::as_str) else {
        return VocabType::Raw;
    };
    let decoders: Vec<&Value> = if decoder_type == "Sequence" {
        decoder
            .get("decoders")
            .and_then(Value::as_array)
            .map(|items| items.iter().collect())
            .unwrap_or_default()
    } else {
        vec![object.get("decoder").unwrap_or(&Value::Null)]
    };
    for decoder in decoders {
        let Some(decoder_obj) = decoder.as_object() else {
            continue;
        };
        let Some(kind) = decoder_obj.get("type").and_then(Value::as_str) else {
            continue;
        };
        match kind {
            "ByteLevel" => return VocabType::ByteLevel,
            "ByteFallback" => return VocabType::ByteFallback,
            _ => {},
        }
    }
    VocabType::Raw
}

fn detect_add_prefix_space(object: &serde_json::Map<String, Value>) -> bool {
    detect_prepend_normalizer(object) || detect_metaspace_pre_tokenizer(object)
}

fn detect_prepend_normalizer(object: &serde_json::Map<String, Value>) -> bool {
    let Some(normalizer) = object.get("normalizer").and_then(Value::as_object)
    else {
        return false;
    };
    let Some(normalizer_type) = normalizer.get("type").and_then(Value::as_str)
    else {
        return false;
    };
    let normalizers: Vec<&Value> = if normalizer_type == "Sequence" {
        normalizer
            .get("normalizers")
            .and_then(Value::as_array)
            .map(|items| items.iter().collect())
            .unwrap_or_default()
    } else {
        vec![object.get("normalizer").unwrap_or(&Value::Null)]
    };
    normalizers.iter().any(|normalizer| {
        let Some(normalizer_obj) = normalizer.as_object() else {
            return false;
        };
        normalizer_obj.get("type").and_then(Value::as_str) == Some("Prepend")
            && normalizer_obj.get("prepend").and_then(Value::as_str)
                == Some("▁")
    })
}

fn detect_metaspace_pre_tokenizer(
    object: &serde_json::Map<String, Value>
) -> bool {
    let Some(pre_tokenizer) =
        object.get("pre_tokenizer").and_then(Value::as_object)
    else {
        return false;
    };
    let Some(kind) = pre_tokenizer.get("type").and_then(Value::as_str) else {
        return false;
    };
    let Some(scheme) =
        pre_tokenizer.get("prepend_scheme").and_then(Value::as_str)
    else {
        return false;
    };
    kind == "Metaspace" && (scheme == "always" || scheme == "first")
}
