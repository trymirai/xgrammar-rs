use xgrammar::{
    Grammar, GrammarCompiler, GrammarMatcher, TokenizerInfo, VocabType,
    allocate_token_bitmask, get_masked_tokens_from_bitmask,
};

#[test]
fn root_api_compiles_and_drives_a_matcher() {
    let grammar = Grammar::from_regex("a+").unwrap();
    let vocab = ["a".to_owned(), "b".to_owned(), "</s>".to_owned()];
    let tokenizer =
        TokenizerInfo::new(&vocab, VocabType::RAW, None, None, false);
    let compiler = GrammarCompiler::with_defaults(tokenizer.clone());
    let compiled = compiler.compile_grammar(&grammar);
    let mut matcher = GrammarMatcher::from_compiled_grammar(&compiled, false);
    let mut bitmask = allocate_token_bitmask(1, tokenizer.vocab_size());

    matcher.fill_next_token_bitmask(&mut bitmask, 0).unwrap();
    assert_eq!(
        get_masked_tokens_from_bitmask(&bitmask, tokenizer.vocab_size(), 0),
        vec![1, 2]
    );
    assert!(matcher.accept_token(0));
    assert!(matcher.is_completed());
}

#[cfg(feature = "tokenizers")]
#[test]
fn huggingface_constructor_preserves_token_ids_and_model_padding() {
    use ahash::AHashMap;
    use tokenizers::{Tokenizer, models::wordlevel::WordLevel};

    let vocab = AHashMap::from([
        ("<unk>".to_owned(), 0),
        ("hello".to_owned(), 2),
        ("</s>".to_owned(), 3),
    ]);
    let model = WordLevel::builder()
        .vocab(vocab)
        .unk_token("<unk>".to_owned())
        .build()
        .unwrap();
    let tokenizer = Tokenizer::new(model);

    let info = TokenizerInfo::from_huggingface(&tokenizer, Some(6), Some(&[3]))
        .unwrap();

    assert_eq!(info.vocab_size(), 6);
    assert_eq!(info.decoded_vocab()[2], b"hello");
    assert_eq!(info.stop_token_ids(), &[3]);
    assert!(info.special_token_ids().contains(&1));
    assert!(info.special_token_ids().contains(&5));
}
