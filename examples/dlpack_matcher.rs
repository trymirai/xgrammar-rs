use autocxx::{WithinBox, cxx};
use xgrammar::xgrammar::TokenizerInfo;
use xgrammar::cxx_utils;

fn main() {
    // 1) Build tokenizer info and compiler
    let vocab: Vec<String> = vec![
        "{".to_string(),
        "}".to_string(),
        "[".to_string(),
        "]".to_string(),
        ",".to_string(),
        ":".to_string(),
        "\"".to_string(),
        "0".to_string(),
        "1".to_string(),
        "2".to_string(),
    ];

    let mut encoded_vocab = cxx_utils::new_string_vector();
    {
        let mut vpin = encoded_vocab.pin_mut();
        cxx_utils::string_vec_reserve(vpin.as_mut(), vocab.len());
        for item in &vocab {
            let bytes = item.as_bytes();
            unsafe { cxx_utils::string_vec_push_bytes(vpin.as_mut(), bytes.as_ptr() as *const i8, bytes.len()); }
        }
    }
    let meta = format!(
        "{{\"vocab_type\":0,\"vocab_size\":{},\"add_prefix_space\":false,\"stop_token_ids\":[]}}",
        vocab.len()
    );
    cxx::let_cxx_string!(metadata = meta);

    let tok = TokenizerInfo::FromVocabAndMetadata(
        encoded_vocab.as_ref().unwrap(),
        &metadata
    ).within_box();

    let vocab_size = tok.GetVocabSize();
    println!("ok; vocab_size={:?} tokens", vocab_size);
}


