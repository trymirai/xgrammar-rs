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
