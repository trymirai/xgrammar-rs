use xgrammar::{
    Grammar, GrammarCompiler, GrammarMatcher, TokenizerInfo, VocabType, allocate_token_bitmask, get_bitmask_size,
    get_masked_tokens_from_bitmask,
};

#[test]
fn root_api_compiles_and_drives_a_matcher() {
    let grammar = Grammar::from_regex("a+").unwrap();
    let vocab = ["a".to_owned(), "b".to_owned(), "</s>".to_owned()];
    let tokenizer = TokenizerInfo::new(&vocab, VocabType::RAW, None, None, false).unwrap();
    let compiler = GrammarCompiler::with_defaults(tokenizer.clone());
    let compiled = compiler.compile_grammar(&grammar).unwrap();
    let mut matcher = GrammarMatcher::from_compiled_grammar(&compiled, false);
    let mut bitmask = allocate_token_bitmask(1, tokenizer.vocab_size());

    let shape = [1, get_bitmask_size(tokenizer.vocab_size()).into()];
    matcher.fill_next_token_bitmask(&mut bitmask, &shape, 0).unwrap();
    assert_eq!(get_masked_tokens_from_bitmask(&bitmask, tokenizer.vocab_size(), 0), vec![1, 2]);
    assert!(matcher.accept_token(0));
    assert!(matcher.is_completed());
}

#[test]
fn matcher_and_compiled_grammar_are_thread_safe() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<GrammarMatcher>();
    assert_send_sync::<xgrammar::CompiledGrammar>();
    assert_send_sync::<GrammarCompiler>();
}

#[test]
fn distinct_serialized_rule_graphs_preserve_mask_walks() {
    let vocab = ["a".to_owned(), "b".to_owned(), "</s>".to_owned()];
    let tokenizer = TokenizerInfo::new(&vocab, VocabType::RAW, None, Some(vec![2]), false).unwrap();
    let compiled =
        GrammarCompiler::with_defaults(tokenizer.clone()).compile_grammar(&Grammar::from_regex("a+").unwrap()).unwrap();
    let original = compiled.serialize_json_value();
    for pad_aux in [false, true] {
        let mut value = original.clone();
        let graph = &mut value["grammar"]["per_rule_fsms"][0][0][0];
        if pad_aux {
            graph["edge_aux_data"].as_array_mut().unwrap().push(serde_json::json!(0));
        } else {
            let offsets = graph["edges"]["indptr_"].as_array_mut().unwrap();
            offsets.push(offsets.last().unwrap().clone());
        }
        let restored = xgrammar::CompiledGrammar::deserialize_json_value(&value, &tokenizer).unwrap();
        let mut expected = GrammarMatcher::from_compiled_grammar(&compiled, false);
        let mut actual = GrammarMatcher::from_compiled_grammar(&restored, false);
        let shape = [1, get_bitmask_size(tokenizer.vocab_size()).into()];
        for token in [0, 0, 2] {
            let mut expected_mask = allocate_token_bitmask(1, tokenizer.vocab_size());
            let mut actual_mask = expected_mask.clone();
            assert_eq!(
                expected.fill_next_token_bitmask(&mut expected_mask, &shape, 0).unwrap(),
                actual.fill_next_token_bitmask(&mut actual_mask, &shape, 0).unwrap()
            );
            assert_eq!(expected_mask, actual_mask);
            assert_eq!(expected.accept_token(token), actual.accept_token(token));
            assert_eq!(expected.is_completed(), actual.is_completed());
        }
    }
}

#[test]
fn shared_rule_storage_keeps_accepting_states_per_rule() {
    let vocab = ["a".to_owned(), "b".to_owned(), "</s>".to_owned()];
    let tokenizer = TokenizerInfo::new(&vocab, VocabType::RAW, None, Some(vec![2]), false).unwrap();
    let compiled = GrammarCompiler::with_defaults(tokenizer.clone())
        .compile_grammar(&Grammar::from_ebnf("root ::= \"a\" second\nsecond ::= \"b\"", "root").unwrap())
        .unwrap();
    let mut value = compiled.serialize_json_value();
    let root_start = value["grammar"]["per_rule_fsms"][0][0][1].clone();
    value["grammar"]["per_rule_fsms"][1][0][1] = root_start.clone();
    value["grammar"]["per_rule_fsms"][1][0][2] = serde_json::json!([root_start]);
    let restored = xgrammar::CompiledGrammar::deserialize_json_value(&value, &tokenizer).unwrap();
    let mut matcher = GrammarMatcher::from_compiled_grammar(&restored, false);
    assert!(matcher.accept_token(0));
    assert!(matcher.is_completed());
    assert!(matcher.accept_token(2));
    assert!(matcher.is_terminated());
}

#[cfg(feature = "tokenizers")]
#[test]
fn huggingface_constructor_preserves_token_ids_and_model_padding() {
    use ahash::AHashMap;
    use tokenizers::{Tokenizer, models::wordlevel::WordLevel};

    let vocab = AHashMap::from([("<unk>".to_owned(), 0), ("hello".to_owned(), 2), ("</s>".to_owned(), 3)]);
    let model = WordLevel::builder().vocab(vocab).unk_token("<unk>".to_owned()).build().unwrap();
    let tokenizer = Tokenizer::new(model);

    let info = TokenizerInfo::from_huggingface(&tokenizer, Some(6), Some(&[3])).unwrap();

    assert_eq!(info.vocab_size(), 6);
    assert_eq!(info.decoded_vocab()[2], b"hello");
    assert_eq!(info.stop_token_ids(), &[3]);
    assert!(info.special_token_ids().contains(&1));
    assert!(info.special_token_ids().contains(&5));
}
