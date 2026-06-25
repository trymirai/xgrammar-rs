//! Port of `xgrammar/tests/python/test_grammar_matcher_basic.py`.
//!
//! The string-acceptance slice is ported here. The token/bitmask and HuggingFace-gated
//! cases land with the compiler/tokenizer milestones.

use xgrammar::{grammar::Grammar, matcher::GrammarMatcher};

/// `_get_matcher_from_grammar`: a matcher that only accepts strings (no tokenizer), in
/// `terminate_without_stop_token` mode.
fn matcher(grammar: &str) -> GrammarMatcher {
    let grammar = Grammar::from_ebnf(grammar, "root").unwrap();
    GrammarMatcher::from_grammar(&grammar, true)
}

#[test]
fn test_accept_string() {
    // `root ::= [^a]+` — a negated character class, one or more (exercises the multibyte
    // UTF-8 FSM encoding and the parser's byte-level scanning).
    let cases: &[(&[u8], bool)] = &[
        (b"bbb", true),
        (b"bba", false),
        ("©".as_bytes(), true),
        (b"\xe2\xa1\xa1", true),
        (b"\xe2\xa1\xa1\xa1", false),
        (b"\xe2\xa1\xe2\xa1", false),
    ];
    for &(input, accepted) in cases {
        let mut m = matcher("root ::= [^a]+");
        assert_eq!(m.accept_bytes(input), accepted, "input {input:?}");
    }
}

/// `_is_grammar_accept_string(json_grammar, ...)` against the built-in JSON grammar.
fn json_accepts(input: &str) -> bool {
    let grammar = Grammar::builtin_json_grammar();
    let mut m = GrammarMatcher::from_grammar(&grammar, true);
    m.accept_string(input) && m.is_terminated()
}

#[test]
fn test_grammar_accept() {
    for input in [r#"{"name": "John"}"#, r#"{ "name" : "John" }"#] {
        assert!(json_accepts(input), "should accept {input:?}");
    }
}

#[test]
fn test_grammar_refuse() {
    for input in [r#"{ name: "John" }"#, r#"{ "name": "John" } "#] {
        assert!(!json_accepts(input), "should refuse {input:?}");
    }
}
