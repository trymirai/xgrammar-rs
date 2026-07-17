//! Compile the built-in JSON grammar and accept a sample JSON string.

use xgrammar::{GrammarCompiler, GrammarMatcher, TokenizerInfo, VocabType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Minimal byte-level vocab: one token per byte 0..=255, plus a stop token.
    let mut encoded_vocab: Vec<String> = (0u8..=255)
        .map(|byte| char::from_u32(u32::from(byte)).unwrap().to_string())
        .collect();
    encoded_vocab.push("</s>".to_string());

    let tokenizer_info = TokenizerInfo::new(
        &encoded_vocab,
        VocabType::ByteFallback,
        None,
        Some(vec![256]),
        false,
    );
    let compiler = GrammarCompiler::with_defaults(tokenizer_info);
    let compiled = compiler.compile_builtin_json_grammar();
    let mut matcher = GrammarMatcher::from_compiled_grammar(&compiled, false);

    let sample = r#"{"name":"Ada","role":"student"}"#;
    assert!(
        matcher.accept_string(sample),
        "grammar rejected: {sample}"
    );
    println!("{sample}");
    println!(
        "completed={} terminated={}",
        matcher.is_completed(),
        matcher.is_terminated()
    );
    Ok(())
}
