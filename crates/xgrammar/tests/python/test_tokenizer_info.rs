//! Port of `xgrammar/tests/python/test_tokenizer_info.py`.
//!
//! The pure special-token-detection case is ported here; the HuggingFace-backed cases
//! (real tokenizers, metadata dump/load against models) land with the `hf` feature.

use std::collections::BTreeSet;

use xgrammar::tokenizer::TokenizerInfo;

#[test]
fn test_special_token_detection() {
    // Only the empty string "" is treated as a special token.
    let vocab: Vec<String> =
        ["", "<s>", "</s>", "[@BOS@]", "regular", "<>", "<think>", "</think>"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
    let info = TokenizerInfo::from_vocab_and_metadata(
        &vocab,
        r#"{"vocab_type":1,"vocab_size":8,"add_prefix_space":true,"stop_token_ids":[2]}"#,
    )
    .unwrap();
    let special: BTreeSet<i32> =
        info.special_token_ids().iter().copied().collect();
    assert_eq!(special, BTreeSet::from([0]));
}
