use serde_json::{self, Value};
use xgrammar::{TokenizerInfo, VocabType};

fn construct_tokenizer_info() -> TokenizerInfo {
    // Vocabulary matching the Python test
    let vocab = vec!["1", "212", "a", "A", "b", "一", "-", "aBc", "abc"];
    let stop_ids: Option<Box<[i32]>> =
        Some(vec![0_i32, 1_i32].into_boxed_slice());
    TokenizerInfo::new_with_vocab_size(
        &vocab,
        VocabType::BYTE_FALLBACK,
        Some(10),
        &stop_ids,
        true,
    )
}

#[test]
fn test_serialize_tokenizer_info() {
    let tokenizer_info = construct_tokenizer_info();
    let serialized = tokenizer_info.serialize_json();

    let expected_json = r#"{
        "vocab_type":1,
        "vocab_size":10,
        "add_prefix_space":true,
        "stop_token_ids":[0,1],
        "special_token_ids":[9],
        "decoded_vocab":["1","212","a","A","b","\u00e4\u00b8\u0080","-","aBc","abc"],
        "sorted_decoded_vocab":[[6,"-"],[3,"A"],[2,"a"],[7,"aBc"],[8,"abc"],[4,"b"],[5,"\u00e4\u00b8\u0080"]],
        "trie_subtree_nodes_range":[1,2,5,4,5,6,7],
        "__VERSION__":"v5"
    }"#;

    let got: Value = serde_json::from_str(&serialized).unwrap();
    let exp: Value = serde_json::from_str(expected_json).unwrap();

    assert_eq!(got, exp);
}

#[test]
fn test_serialize_tokenizer_info_roundtrip() {
    let original = construct_tokenizer_info();
    let serialized = original.serialize_json();
    let recovered = TokenizerInfo::deserialize_json(&serialized)
        .expect("failed to deserialize TokenizerInfo");
    let serialized_new = recovered.serialize_json();
    assert_eq!(serialized, serialized_new);
}

#[test]
fn test_serialize_tokenizer_info_functional() {
    let original = construct_tokenizer_info();
    let serialized = original.serialize_json();
    let recovered = TokenizerInfo::deserialize_json(&serialized)
        .expect("failed to deserialize TokenizerInfo");

    assert_eq!(original.vocab_type() as i32, recovered.vocab_type() as i32);
    assert_eq!(original.vocab_size(), recovered.vocab_size());
    assert_eq!(original.add_prefix_space(), recovered.add_prefix_space());

    let o_stop: Vec<i32> = original.stop_token_ids().into();
    let r_stop: Vec<i32> = recovered.stop_token_ids().into();
    assert_eq!(o_stop, r_stop);

    let o_spec: Vec<i32> = original.special_token_ids().into();
    let r_spec: Vec<i32> = recovered.special_token_ids().into();
    assert_eq!(o_spec, r_spec);

    // Compare decoded vocab: bytes per token
    let o_dec: Vec<Vec<u8>> =
        original.decoded_vocab().iter().map(|b| b.to_vec()).collect();
    let r_dec: Vec<Vec<u8>> =
        recovered.decoded_vocab().iter().map(|b| b.to_vec()).collect();
    assert_eq!(o_dec, r_dec);
}
