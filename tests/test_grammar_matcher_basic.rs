use xgrammar::{
    Grammar, GrammarCompiler, GrammarMatcher, TokenizerInfo, VocabType,
};

fn matcher_from_grammar(gram: &Grammar) -> GrammarMatcher {
    // Minimal vocab works for structural acceptance tests (no bitmask calc needed)
    let empty_vocab: Vec<&str> = vec![];
    let tokenizer_info =
        TokenizerInfo::new(&empty_vocab, VocabType::RAW, &None, false);
    let mut compiler = GrammarCompiler::new(&tokenizer_info, 1, false, -1);
    let cg = compiler.compile_grammar(gram);
    GrammarMatcher::new(&cg, None, false, -1)
}

#[test]
fn test_accept_string_basic() {
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
fn test_grammar_accept_refuse_json() {
    let json_grammar = Grammar::builtin_json_grammar();
    let mut matcher_accept = matcher_from_grammar(&json_grammar);
    assert!(matcher_accept.accept_string("{\"name\": \"John\"}", false));

    let mut matcher_refuse = matcher_from_grammar(&json_grammar);
    assert!(!matcher_refuse.accept_string("{ name: \"John\" }", false));

    let mut matcher_refuse_trailing = matcher_from_grammar(&json_grammar);
    assert!(
        !matcher_refuse_trailing
            .accept_string("{ \"name\": \"John\" } ", false)
    );
}

#[test]
fn test_debug_print_internal_state() {
    let json_grammar = Grammar::builtin_json_grammar();
    let mut matcher = matcher_from_grammar(&json_grammar);
    let input = "{\"name\": \"John\"}";
    for ch in input.chars() {
        let s = ch.to_string();
        assert!(matcher.accept_string(&s, false));
        let state = matcher.debug_print_internal_state();
        assert!(!state.is_empty());
    }
}

#[test]
fn test_get_jump_forward_string() {
    let ebnf = r#"root ::= "abb" | "abbd" | other_rule
other_rule ::= "a" sub_rule "b"
sub_rule ::= "b""#;
    let g = Grammar::from_ebnf(ebnf, "root");
    let mut matcher = matcher_from_grammar(&g);
    assert!(matcher.accept_string("a", false));
    assert_eq!(matcher.find_jump_forward_string(), "bb");
}
