use serial_test::serial;
use xgrammar::{
    Grammar, GrammarCompiler, GrammarMatcher, TokenizerInfo, VocabType,
};

fn matcher_from_grammar(gram: &Grammar) -> GrammarMatcher {
    // Minimal vocab works for structural acceptance tests (no bitmask calc needed)
    let empty_vocab: Vec<&str> = vec![];
    let stop_ids: Option<Box<[i32]>> = None;
    let tokenizer_info =
        TokenizerInfo::new(&empty_vocab, VocabType::RAW, &stop_ids, false);
    let mut compiler = GrammarCompiler::new(&tokenizer_info, 1, false, -1);
    let cg = compiler.compile_grammar(gram);
    GrammarMatcher::new(&cg, None, true, -1)
}

#[test]
#[serial]
fn test_accept_string() {
    // Port of grammar__input__accepted__test_accept_string (string-only cases)
    let cases: &[(&str, &str, bool)] = &[
        ("root ::= [^a]+", "bbb", true),
        ("root ::= [^a]+", "bba", false),
        ("root ::= [^a]+", "©", true), // U+00A9
    ];

    for (ebnf, input, accepted) in cases {
        let g = Grammar::from_ebnf(ebnf, "root");
        let mut m = matcher_from_grammar(&g);
        assert_eq!(m.accept_string(input, false), *accepted, "{}", input);
    }
}

#[test]
#[serial]
fn test_grammar_accept() {
    let json_grammar = Grammar::builtin_json_grammar();
    let mut matcher_1 = matcher_from_grammar(&json_grammar);
    assert!(matcher_1.accept_string("{\"name\": \"John\"}", false));

    let mut matcher_2 = matcher_from_grammar(&json_grammar);
    assert!(matcher_2.accept_string("{ \"name\" : \"John\" }", false));
}

#[test]
#[serial]
fn test_grammar_refuse() {
    let json_grammar = Grammar::builtin_json_grammar();
    let mut matcher_1 = matcher_from_grammar(&json_grammar);
    assert!(!matcher_1.accept_string("{ name: \"John\" }", false));

    let mut matcher_2 = matcher_from_grammar(&json_grammar);
    assert!(!matcher_2.accept_string("{ \"name\": \"John\" } ", false));
}

#[test]
#[serial]
fn test_debug_print_internal_state() {
    let json_grammar = Grammar::builtin_json_grammar();
    let mut matcher = matcher_from_grammar(&json_grammar);
    let input = "{\"name\": \"John\"}";
    for ch in input.chars() {
        let s = ch.to_string();
        assert!(
            matcher.accept_string(&s, false),
            "Failed to accept character: {:?}",
            ch
        );
        let state = matcher.debug_print_internal_state();
        assert!(!state.is_empty());
    }
}

#[test]
#[serial]
fn test_get_jump_forward_string() {
    let ebnf = r#"root ::= "abb" | "abbd" | other_rule
other_rule ::= "a" sub_rule "b"
sub_rule ::= "b""#;
    let g = Grammar::from_ebnf(ebnf, "root");
    let mut matcher = matcher_from_grammar(&g);
    assert!(matcher.accept_string("a", false));
    assert_eq!(matcher.find_jump_forward_string(), "bb");
}
