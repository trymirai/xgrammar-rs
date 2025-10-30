#![cfg(feature = "hf")]
use serial_test::serial;

fn download_tokenizer_json(
    model_id: &str
) -> Result<std::path::PathBuf, String> {
    use hf_hub::{
        Repo,
        api::sync::{Api, ApiBuilder},
    };
    let api = ApiBuilder::new().build().map_err(|e| e.to_string())?;
    let repo = api.repo(Repo::model(model_id.to_string()));
    repo.get("tokenizer.json").map_err(|e| e.to_string())
}

fn extract_ordered_vocab(tk: &tokenizers::Tokenizer) -> Vec<String> {
    let mut pairs: Vec<(usize, String)> = tk
        .get_vocab(true)
        .into_iter()
        .map(|(tok, id)| (id as usize, tok))
        .collect();
    pairs.sort_by_key(|(id, _)| *id);
    let mut out = Vec::with_capacity(pairs.len());
    for (_, tok) in pairs {
        out.push(tok);
    }
    out
}

// Subset of Python's tokenizer_path__vocab_type__prepend_space for stable public models
fn cases_model_vocab() -> Vec<(&'static str, xgrammar::VocabType, bool)> {
    vec![
        ("gpt2", xgrammar::VocabType::BYTE_LEVEL, false),
        ("bert-base-uncased", xgrammar::VocabType::RAW, false),
    ]
}

#[test]
#[serial]
fn test_build_tokenizer_info() {
    for (model_id, vocab_type, add_prefix_space) in cases_model_vocab() {
        let path =
            download_tokenizer_json(model_id).expect("download tokenizer.json");
        let tk =
            tokenizers::Tokenizer::from_file(&path).expect("load tokenizer");
        let ti = xgrammar::TokenizerInfo::from_huggingface(&tk, None, None);
        assert!(ti.vocab_size() > 0, "{}", model_id);
    }
}

#[test]
#[serial]
fn test_properties() {
    for (model_id, vocab_type, add_prefix_space) in cases_model_vocab() {
        let path =
            download_tokenizer_json(model_id).expect("download tokenizer.json");
        let tk =
            tokenizers::Tokenizer::from_file(&path).expect("load tokenizer");
        let ti = xgrammar::TokenizerInfo::from_huggingface(&tk, None, None);
        let vocab = tk.get_vocab(true);
        let max_id = vocab.values().copied().max().unwrap_or(0) as usize;
        assert_eq!(ti.vocab_size(), std::cmp::max(vocab.len(), max_id + 1));
        assert_eq!(ti.add_prefix_space(), add_prefix_space);
    }
}

#[test]
#[serial]
fn test_decoded_vocab() {
    for (model_id, vocab_type, add_prefix_space) in cases_model_vocab() {
        let path =
            download_tokenizer_json(model_id).expect("download tokenizer.json");
        let tk =
            tokenizers::Tokenizer::from_file(&path).expect("load tokenizer");
        let ti = xgrammar::TokenizerInfo::from_huggingface(&tk, None, None);
        let decoded = ti.decoded_vocab();
        let vocab = tk.get_vocab(true);
        let max_id = vocab.values().copied().max().unwrap_or(0) as usize;
        assert_eq!(decoded.len(), std::cmp::max(vocab.len(), max_id + 1));
        assert_eq!(decoded.len(), ti.vocab_size());
    }
}

#[test]
#[serial]
fn test_dump_metadata_load() {
    for (model_id, vocab_type, add_prefix_space) in cases_model_vocab() {
        let path =
            download_tokenizer_json(model_id).expect("download tokenizer.json");
        let tk =
            tokenizers::Tokenizer::from_file(&path).expect("load tokenizer");
        let ti = xgrammar::TokenizerInfo::from_huggingface(&tk, None, None);
        let metadata = ti.dump_metadata();
        let ordered = extract_ordered_vocab(&tk);
        let loaded = xgrammar::TokenizerInfo::from_vocab_and_metadata_bytes(
            ordered.iter().map(|s| s.as_bytes()),
            &metadata,
        );
        assert_eq!(loaded.decoded_vocab(), ti.decoded_vocab());
    }
}

#[test]
#[serial]
fn test_customize_stop_token_ids() {
    for (model_id, vocab_type, add_prefix_space) in cases_model_vocab() {
        let path =
            download_tokenizer_json(model_id).expect("download tokenizer.json");
        let tk =
            tokenizers::Tokenizer::from_file(&path).expect("load tokenizer");
        let stop_ids = [1i32, 2i32, 3i32];
        let ti = xgrammar::TokenizerInfo::from_tokenizers_with_options(
            &tk,
            vocab_type,
            None,
            Some(&stop_ids),
            add_prefix_space,
        );
        assert_eq!(ti.stop_token_ids().as_ref(), &stop_ids);
    }
}

#[test]
#[serial]
fn test_padding_vocab_size() {
    let model_id = "bert-base-uncased";
    let path =
        download_tokenizer_json(model_id).expect("download tokenizer.json");
    let tk = tokenizers::Tokenizer::from_file(&path).expect("load tokenizer");
    let ordered = extract_ordered_vocab(&tk);
    let original = ordered.len();
    let pad_by = 5usize;
    let ti = xgrammar::TokenizerInfo::new_with_vocab_size(
        &ordered,
        xgrammar::VocabType::RAW,
        Some(original + pad_by),
        &None,
        false,
    );
    assert_eq!(ti.vocab_size(), original + pad_by);
    let specials = ti.special_token_ids();
    for i in 0..pad_by {
        assert!(specials.contains(&((original + i) as i32)));
    }
}

#[test]
fn test_special_token_detection() {
    // Matches Python test_special_token_detection (no HF needed)
    let vocab_dict =
        ["", "<s>", "</s>", "[@BOS@]", "regular", "<>", "<think>", "</think>"];
    let tokenizer_info = xgrammar::TokenizerInfo::from_vocab_and_metadata_bytes(
        vocab_dict.iter().map(|s| s.as_bytes()),
        "{\"vocab_type\":1,\"vocab_size\":8,\"add_prefix_space\":true,\"stop_token_ids\":[2]}",
    );
    let expected: std::collections::HashSet<i32> = [0].into_iter().collect();
    let got: std::collections::HashSet<i32> =
        tokenizer_info.special_token_ids().into_iter().collect();
    assert_eq!(got, expected);
}
