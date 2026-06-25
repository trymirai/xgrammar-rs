//! Port of `xgrammar/tests/python/test_token_edge.py`.
//!
//! The parser/printer roundtrip slice plus the `accept_token`/bitmask cases over small
//! in-memory vocabularies are ported here. The structural-tag token suites land with the
//! structural-tag token milestone.

use std::collections::BTreeSet;

use xgrammar::{
    grammar::Grammar,
    matcher::{
        GrammarMatcher, allocate_token_bitmask, get_masked_tokens_from_bitmask,
    },
    parser::ebnf_to_grammar_no_normalization,
    tokenizer::{TokenizerInfo, VocabType},
};

fn no_norm(ebnf: &str) -> String {
    ebnf_to_grammar_no_normalization(ebnf, "root").unwrap().to_string()
}

/// `"</s>"` is auto-detected as the stop token in the test vocabularies (id 1).
const STOP_TOKEN_ID: i32 = 1;

fn make_matcher(
    vocab: &[&str],
    grammar: &str,
) -> GrammarMatcher {
    let vocab: Vec<String> = vocab.iter().map(|s| (*s).to_owned()).collect();
    let info = TokenizerInfo::new(&vocab, VocabType::Raw, None, None, false);
    let grammar = Grammar::from_ebnf(grammar, "root").unwrap();
    GrammarMatcher::from_grammar_and_tokenizer(&grammar, info)
}

/// Fills the bitmask and returns the rejected (masked) token ids.
fn rejected(
    matcher: &mut GrammarMatcher,
    vocab_size: i32,
) -> BTreeSet<i32> {
    let mut bitmask = allocate_token_bitmask(1, vocab_size);
    matcher.fill_next_token_bitmask(&mut bitmask, 0);
    get_masked_tokens_from_bitmask(&bitmask, vocab_size, 0)
        .into_iter()
        .collect()
}

/// `_get_accepted`: the complement of [`rejected`] over the vocabulary.
fn accepted(
    matcher: &mut GrammarMatcher,
    vocab_size: i32,
) -> BTreeSet<i32> {
    let rej = rejected(matcher, vocab_size);
    (0..vocab_size).filter(|i| !rej.contains(i)).collect()
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

#[test]
fn test_accept_token_basic() {
    let vocab = ["<s>", "</s>", "aa", "bb", "cc", "dd"];
    let mut m = make_matcher(&vocab, "root ::= Token(2, 4)\n");
    assert!(m.accept_token(2));
    assert!(m.accept_token(STOP_TOKEN_ID));
    assert!(m.is_terminated());
}

#[test]
fn test_accept_token_reject() {
    let vocab = ["<s>", "</s>", "aa", "bb", "cc", "dd"];
    let mut m = make_matcher(&vocab, "root ::= Token(2, 4)\n");
    assert!(!m.accept_token(3));
    assert!(!m.accept_token(5));
    assert!(m.accept_token(4));
    assert!(m.accept_token(STOP_TOKEN_ID));
    assert!(m.is_terminated());
}

#[test]
fn test_token_then_string() {
    let vocab = ["<s>", "</s>", "aa", "bb", "cc"];
    let mut m = make_matcher(&vocab, "root ::= Token(2) \"bb\"\n");
    assert!(m.accept_token(2));
    assert!(m.accept_token(3));
    assert!(m.accept_token(STOP_TOKEN_ID));
    assert!(m.is_terminated());
}

#[test]
fn test_token_or_string() {
    let vocab = ["<s>", "</s>", "aa", "bb", "cc"];

    let mut m = make_matcher(&vocab, "root ::= Token(2) | \"bb\"\n");
    assert!(m.accept_token(2));
    assert!(m.accept_token(STOP_TOKEN_ID));
    assert!(m.is_terminated());

    let mut m2 = make_matcher(&vocab, "root ::= Token(2) | \"bb\"\n");
    assert!(m2.accept_token(3));
    assert!(m2.accept_token(STOP_TOKEN_ID));
    assert!(m2.is_terminated());
}

#[test]
fn test_bitmask_token_only() {
    let vocab = ["<s>", "</s>", "aa", "bb", "cc", "dd"];
    let mut m = make_matcher(&vocab, "root ::= Token(2, 4)\n");
    assert_eq!(rejected(&mut m, 6), BTreeSet::from([0, 1, 3, 5]));
}

#[test]
fn test_bitmask_token_and_string() {
    let vocab = ["<s>", "</s>", "aa", "bb", "cc"];
    let mut m = make_matcher(&vocab, "root ::= Token(2) | \"bb\"\n");
    assert_eq!(rejected(&mut m, 5), BTreeSet::from([0, 1, 4]));
}

#[test]
fn test_bitmask_after_token() {
    let vocab = ["<s>", "</s>", "aa", "bb", "cc"];
    let mut m = make_matcher(&vocab, "root ::= Token(2) \"bb\"\n");
    assert_eq!(rejected(&mut m, 5), BTreeSet::from([0, 1, 3, 4]));
    assert!(m.accept_token(2));
    assert_eq!(rejected(&mut m, 5), BTreeSet::from([0, 1, 2, 4]));
}

#[test]
fn test_token_multiple_choices() {
    let vocab = ["<s>", "</s>", "x", "y", "z", "w"];
    let mut m = make_matcher(&vocab, "root ::= Token(2, 3, 4) | \"w\"\n");
    assert_eq!(rejected(&mut m, 6), BTreeSet::from([0, 1]));
}

#[test]
fn test_char_then_token_sequence() {
    let vocab = ["<s>", "</s>", "A", "B", "hello", "world"];
    let mut m = make_matcher(&vocab, "root ::= \"A\" Token(4, 5)\n");
    assert!(m.accept_token(2));
    assert!(m.accept_token(4));
    assert!(m.accept_token(STOP_TOKEN_ID));
    assert!(m.is_terminated());
}

// --- TokenTagDispatch tests ---

#[test]
fn test_token_tag_dispatch_trigger() {
    let vocab = ["<s>", "</s>", "hello", "trigger_tok", "content"];
    let mut m = make_matcher(
        &vocab,
        "triggered_rule ::= Token(4)\nroot ::= TokenTagDispatch(\n  (3, triggered_rule)\n)",
    );
    assert_eq!(accepted(&mut m, 5), BTreeSet::from([0, 1, 2, 3, 4]));
    assert!(m.accept_token(3)); // dispatch trigger
    assert_eq!(accepted(&mut m, 5), BTreeSet::from([4]));
}

#[test]
fn test_token_tag_dispatch_multiple_triggers() {
    let vocab = ["<s>", "</s>", "A", "B", "<tool>", "content"];
    let mut m = make_matcher(
        &vocab,
        "tool_body ::= Token(5)\nother_body ::= Token(5)\nroot ::= TokenTagDispatch(\n  (3, tool_body),\n  (4, other_body)\n)",
    );
    assert_eq!(accepted(&mut m, 6), BTreeSet::from([0, 1, 2, 3, 4, 5]));
    assert!(m.accept_token(3)); // dispatch to tool_body
    assert_eq!(accepted(&mut m, 6), BTreeSet::from([5]));
}

#[test]
fn test_token_tag_dispatch_trigger_loop() {
    let vocab = ["<s>", "</s>", "hello", "trigger", "content"];
    let mut m = make_matcher(
        &vocab,
        "body ::= Token(4)\nroot ::= TokenTagDispatch(\n  (3, body),\n  loop_after_dispatch=true\n)",
    );
    assert!(m.accept_token(3)); // trigger dispatches to body
    assert!(m.accept_token(4)); // Token(4) completes body
    assert_eq!(accepted(&mut m, 5), BTreeSet::from([0, 1, 2, 3, 4]));
}

#[test]
fn test_token_tag_dispatch_trigger_and_exclude_no_overlap() {
    // Trigger ids and excludes must not overlap.
    let grammar_str = "body ::= Token(2)\nroot ::= TokenTagDispatch(\n  (3, body),\n  excludes=(3,)\n)";
    assert!(Grammar::from_ebnf(grammar_str, "root").is_err());
}

#[test]
fn test_token_tag_dispatch_trigger_in_bitmask() {
    let vocab = ["<s>", "</s>", "hello", "trigger", "content"];
    let mut m = make_matcher(
        &vocab,
        "body ::= Token(4)\nroot ::= TokenTagDispatch(\n  (3, body)\n)",
    );
    assert_eq!(accepted(&mut m, 5), BTreeSet::from([0, 1, 2, 3, 4]));
    assert!(m.accept_token(3)); // dispatch trigger
    assert_eq!(accepted(&mut m, 5), BTreeSet::from([4]));
}

#[test]
fn test_token_tag_dispatch_full_combo() {
    let vocab = ["<s>", "</s>", "hello", "B", "<tool>", "content", "blocked"];
    let mut m = make_matcher(
        &vocab,
        "tool_body ::= Token(5)\nother_body ::= Token(5)\nroot ::= TokenTagDispatch(\n  (3, tool_body),\n  (4, other_body),\n  excludes=(6,)\n)",
    );
    assert_eq!(accepted(&mut m, 7), BTreeSet::from([0, 1, 2, 3, 4, 5]));
}

#[test]
fn test_token_tag_dispatch_exclude_no_triggers() {
    let vocab = ["<s>", "</s>", "hello", "world", "blocked_1", "blocked_2"];
    let mut m = make_matcher(
        &vocab,
        "root ::= TokenTagDispatch(\n  excludes=(4, 5)\n)",
    );
    for _ in 0..3 {
        assert_eq!(accepted(&mut m, 6), BTreeSet::from([0, 1, 2, 3]));
        m.accept_token(2);
    }
}

#[test]
fn test_token_tag_dispatch_exclude_basic() {
    let vocab = ["<s>", "</s>", "hello", "world", "bad"];
    let mut m =
        make_matcher(&vocab, "root ::= TokenTagDispatch(\n  excludes=(4,)\n)");
    assert_eq!(accepted(&mut m, 5), BTreeSet::from([0, 1, 2, 3]));
}

#[test]
fn test_token_tag_dispatch_reject_enforced_by_parser() {
    let vocab = ["<s>", "</s>", "hello", "world", "blocked"];
    let mut m =
        make_matcher(&vocab, "root ::= TokenTagDispatch(\n  excludes=(4,)\n)");
    assert!(!m.accept_token(4)); // parser must reject excluded token
    assert!(m.accept_token(2)); // "hello" still accepted
}

#[test]
fn test_token_tag_dispatch_trigger_and_exclude() {
    let vocab = ["<s>", "</s>", "A", "AB", "blocked"];
    let mut m = make_matcher(
        &vocab,
        "rule1 ::= \"done\"\nroot ::= TokenTagDispatch(\n  (3, rule1),\n  excludes=(4,)\n)",
    );
    assert_eq!(accepted(&mut m, 5), BTreeSet::from([0, 1, 2, 3]));
}
