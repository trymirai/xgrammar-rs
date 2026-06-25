//! Port of `xgrammar/tests/python/test_token_edge.py`.
//!
//! The parser/printer roundtrip slice is ported here. The `accept_token`/bitmask cases
//! depend on the token-masking compiler and land with that milestone.

use xgrammar::parser::ebnf_to_grammar_no_normalization;

fn no_norm(ebnf: &str) -> String {
    ebnf_to_grammar_no_normalization(ebnf, "root").unwrap().to_string()
}

#[test]
fn test_parse_token_basic() {
    assert_eq!(
        no_norm("root ::= Token(1, 2, 3)\n"),
        "root ::= ((Token(1, 2, 3)))\n"
    );
}

#[test]
fn test_parse_token_single() {
    assert_eq!(no_norm("root ::= Token(42)\n"), "root ::= ((Token(42)))\n");
}

#[test]
fn test_parse_token_sorted_deduped() {
    assert_eq!(
        no_norm("root ::= Token(3, 1, 2, 1, 3)\n"),
        "root ::= ((Token(1, 2, 3)))\n"
    );
}

#[test]
fn test_parse_token_in_sequence() {
    assert_eq!(
        no_norm("root ::= Token(1, 2) \"hello\"\n"),
        "root ::= ((Token(1, 2) \"hello\"))\n"
    );
}

#[test]
fn test_parse_token_in_alternation() {
    assert_eq!(
        no_norm("root ::= Token(1) | \"hello\"\n"),
        "root ::= ((Token(1)) | (\"hello\"))\n"
    );
}

#[test]
fn test_parse_exclude_token_basic() {
    assert_eq!(
        no_norm("root ::= ExcludeToken(1, 2, 3)\n"),
        "root ::= ((ExcludeToken(1, 2, 3)))\n"
    );
}

#[test]
fn test_parse_exclude_token_sorted_deduped() {
    assert_eq!(
        no_norm("root ::= ExcludeToken(3, 1, 2, 1)\n"),
        "root ::= ((ExcludeToken(1, 2, 3)))\n"
    );
}
