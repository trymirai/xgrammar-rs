use xgrammar::{GrammarCompiler, TokenizerInfo, VocabType};

#[test]
fn test_grammar_compiler_basic() {
    // Create a minimal tokenizer info
    let vocab = vec!["a", "b", "c"];
    let tokenizer_info =
        TokenizerInfo::new(&vocab, VocabType::RAW, &None, false);

    // Create a grammar compiler
    let compiler = GrammarCompiler::new(
        &tokenizer_info,
        8,    // max_threads
        true, // cache_enabled
        -1,   // cache_limit_bytes (unlimited)
    );

    // Compile builtin JSON grammar
    let compiled_grammar = compiler.compile_builtin_json_grammar();

    // Verify the compiled grammar has the correct tokenizer info
    let retrieved_tokenizer_info = compiled_grammar.tokenizer_info();
    assert_eq!(retrieved_tokenizer_info.vocab_size(), vocab.len());
}

#[test]
fn test_grammar_compiler_json_schema() {
    let vocab = vec!["a", "b", "c", "{", "}", ":", ",", "\"", "1", "2"];
    let tokenizer_info =
        TokenizerInfo::new(&vocab, VocabType::RAW, &None, false);

    let compiler = GrammarCompiler::new(&tokenizer_info, 1, true, -1);

    // Simple JSON schema
    let schema =
        r#"{"type": "object", "properties": {"name": {"type": "string"}}}"#;
    let compiled = compiler.compile_json_schema(
        schema,
        true,                 // any_whitespace
        None,                 // indent
        None::<(&str, &str)>, // separators
        true,                 // strict_mode
    );

    // Verify it compiled successfully
    assert!(compiled.memory_size_bytes() > 0);
}

#[test]
fn test_grammar_compiler_regex() {
    let vocab = vec!["a", "b", "c", "d"];
    let tokenizer_info =
        TokenizerInfo::new(&vocab, VocabType::RAW, &None, false);

    let compiler = GrammarCompiler::new(&tokenizer_info, 1, false, -1);

    // Simple regex pattern
    let compiled = compiler.compile_regex("[abc]+");

    assert!(compiled.memory_size_bytes() > 0);
}

#[test]
fn test_grammar_compiler_from_ebnf() {
    let vocab = vec!["a", "b", "c"];
    let tokenizer_info =
        TokenizerInfo::new(&vocab, VocabType::RAW, &None, false);

    let compiler = GrammarCompiler::new(&tokenizer_info, 1, false, -1);

    // Simple EBNF grammar
    let ebnf = r#"root ::= "a" "b" | "c""#;
    let compiled = compiler.compile_grammar_from_ebnf(ebnf, "root");

    assert!(compiled.memory_size_bytes() > 0);
}

#[test]
fn test_grammar_compiler_cache() {
    let vocab = vec!["a", "b", "c"];
    let tokenizer_info =
        TokenizerInfo::new(&vocab, VocabType::RAW, &None, false);

    let mut compiler =
        GrammarCompiler::new(&tokenizer_info, 1, true, 1024 * 1024);

    // Verify cache_limit_bytes returns the configured value
    assert_eq!(compiler.cache_limit_bytes(), 1024 * 1024);

    // Compile a regex (uses regular cache)
    let _compiled1 = compiler.compile_regex("[abc]+");
    let cache_size_after_first = compiler.get_cache_size_bytes();

    // Cache should have some data after first compilation
    assert!(cache_size_after_first > 0);

    // Compile the same regex again (should hit cache)
    let _compiled2 = compiler.compile_regex("[abc]+");
    let cache_size_after_second = compiler.get_cache_size_bytes();

    // Cache size should be the same (second compilation uses cache)
    assert_eq!(cache_size_after_first, cache_size_after_second);

    // Clear cache
    compiler.clear_cache();
    assert_eq!(compiler.get_cache_size_bytes(), 0);
}
