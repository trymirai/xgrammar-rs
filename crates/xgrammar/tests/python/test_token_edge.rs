//! Port of `xgrammar/tests/python/test_token_edge.py`.
//!
//! The parser/printer roundtrip slice, the `accept_token`/bitmask cases over small in-memory
//! vocabularies, the `TokenTagDispatch` cases, and the structural-tag token suites (JSON
//! structural tags resolved against an in-memory vocabulary) are ported here.

use std::collections::BTreeSet;

use xgrammar::{
    compiler::GrammarCompiler,
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

// --- Structural-tag token suites (JSON structural tags with token references resolved
// against the STAG_VOCAB vocabulary, compiled and driven through the matcher). ---

const STAG_VOCAB: &[&str] = &[
    "<s>",
    "</s>",
    "<tool>",
    "<code>",
    "<end>",
    "<think>",
    "<think_end>",
    "hello",
    "world",
    "{",
    "}",
    "fn(",
    ")",
    "x",
    "y",
    ",",
    "<bad>",
];

/// Builds a matcher over a compiled structural tag using [`STAG_VOCAB`] (vocab size 17,
/// `</s>` = stop token id 1).
fn stag_matcher(json: &str) -> GrammarMatcher {
    let vocab: Vec<String> =
        STAG_VOCAB.iter().map(|s| (*s).to_owned()).collect();
    let ti = TokenizerInfo::new(&vocab, VocabType::Raw, None, None, false);
    let compiler = GrammarCompiler::with_defaults(ti);
    let compiled = compiler.compile_structural_tag(json).unwrap();
    GrammarMatcher::from_compiled_grammar(&compiled, false)
}

/// `_accept_tokens`: assert each token in `tokens` is accepted in order.
fn accept_tokens(
    m: &mut GrammarMatcher,
    tokens: &[i32],
) {
    for &t in tokens {
        assert!(m.accept_token(t), "failed to accept token {t}");
    }
}

/// `_accept_and_stop`: accept all `tokens`, then the stop token, then require termination.
fn accept_and_stop(
    m: &mut GrammarMatcher,
    tokens: &[i32],
) {
    accept_tokens(m, tokens);
    assert!(m.accept_token(1), "failed to accept stop token");
    assert!(m.is_terminated());
}

#[test]
fn test_stag_token_begin_end() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<tool>\"},\"content\":{\"type\":\"const_string\",\"value\":\"hello\"},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}}}";
    let mut m = stag_matcher(json);
    accept_and_stop(&mut m, &[2, 7, 4]);
}

#[test]
fn test_stag_exclude_token_basic() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<tool>\"},\"content\":{\"type\":\"exclude_token\",\"exclude_tokens\":[16]},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}}}";
    let mut m = stag_matcher(json);
    accept_tokens(&mut m, &[2]);
    assert!(!m.accept_token(16));
    assert!(!m.accept_token(4));
    assert!(m.accept_token(7));
    accept_and_stop(&mut m, &[4]);
}

#[test]
fn test_stag_any_tokens_loop() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<think>\"},\"content\":{\"type\":\"any_tokens\",\"exclude_tokens\":[16]},\"end\":{\"type\":\"token\",\"token\":\"<think_end>\"}}}";
    let mut m = stag_matcher(json);
    accept_tokens(&mut m, &[5]);
    accept_tokens(&mut m, &[7, 8, 13, 14, 9, 10]);
    assert!(!m.accept_token(16));
    accept_tokens(&mut m, &[0, 3, 2]);
    accept_and_stop(&mut m, &[6]);
}

#[test]
fn test_stag_any_tokens_empty() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<tool>\"},\"content\":{\"type\":\"any_tokens\"},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}}}";
    let mut m = stag_matcher(json);
    accept_and_stop(&mut m, &[2, 4]);
}

#[test]
fn test_stag_token_triggered_tags_basic() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"token_triggered_tags\",\"trigger_tokens\":[\"<tool>\",\"<code>\"],\"tags\":[{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<tool>\"},\"content\":{\"type\":\"const_string\",\"value\":\"hello\"},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}},{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<code>\"},\"content\":{\"type\":\"const_string\",\"value\":\"world\"},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}}],\"exclude_tokens\":[16]}}";
    let mut m = stag_matcher(json);
    assert!(!m.accept_token(16));
    accept_tokens(&mut m, &[7, 8]);
    accept_tokens(&mut m, &[2, 7, 4]);
    accept_tokens(&mut m, &[13, 14]);
    accept_tokens(&mut m, &[3, 8, 4]);
    assert!(m.accept_token(1));
    assert!(m.is_terminated());
}

#[test]
fn test_stag_token_triggered_stop_after_first() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"token_triggered_tags\",\"trigger_tokens\":[\"<tool>\",\"<code>\"],\"tags\":[{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<tool>\"},\"content\":{\"type\":\"const_string\",\"value\":\"x\"},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}},{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<code>\"},\"content\":{\"type\":\"const_string\",\"value\":\"y\"},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}}],\"stop_after_first\":true}}";
    let mut m = stag_matcher(json);
    accept_tokens(&mut m, &[7]);
    accept_and_stop(&mut m, &[2, 13, 4]);
}

#[test]
fn test_stag_token_triggered_at_least_one() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"token_triggered_tags\",\"trigger_tokens\":[\"<tool>\"],\"tags\":[{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<tool>\"},\"content\":{\"type\":\"const_string\",\"value\":\"x\"},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}}],\"at_least_one\":true,\"stop_after_first\":true}}";
    let mut m = stag_matcher(json);
    accept_and_stop(&mut m, &[2, 13, 4]);
}

#[test]
fn test_stag_nested_token_tags_with_any_tokens() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"token_triggered_tags\",\"trigger_tokens\":[\"<tool>\",\"<code>\"],\"tags\":[{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<tool>\"},\"content\":{\"type\":\"any_tokens\",\"exclude_tokens\":[16]},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}},{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<code>\"},\"content\":{\"type\":\"sequence\",\"elements\":[{\"type\":\"exclude_token\",\"exclude_tokens\":[16]},{\"type\":\"const_string\",\"value\":\"x\"}]},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}}],\"exclude_tokens\":[16]}}";
    let mut m = stag_matcher(json);
    accept_tokens(&mut m, &[7, 8]);
    accept_tokens(&mut m, &[2, 9, 10, 13, 14, 7, 4]);
    accept_tokens(&mut m, &[14]);
    accept_tokens(&mut m, &[3, 7, 13, 4]);
    assert!(m.accept_token(1));
    assert!(m.is_terminated());
}

#[test]
fn test_stag_sequence_of_token_formats() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"sequence\",\"elements\":[{\"type\":\"token\",\"token\":\"<tool>\"},{\"type\":\"const_string\",\"value\":\"fn(\"},{\"type\":\"exclude_token\",\"exclude_tokens\":[16,4]},{\"type\":\"const_string\",\"value\":\")\"},{\"type\":\"token\",\"token\":\"<end>\"}]}}";
    let mut m = stag_matcher(json);
    accept_tokens(&mut m, &[2]);
    accept_tokens(&mut m, &[11]);
    assert!(!m.accept_token(16));
    assert!(!m.accept_token(4));
    accept_tokens(&mut m, &[7]);
    accept_tokens(&mut m, &[12]);
    accept_and_stop(&mut m, &[4]);
}

#[test]
fn test_stag_or_token_and_string_paths() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"or\",\"elements\":[{\"type\":\"sequence\",\"elements\":[{\"type\":\"token\",\"token\":\"<tool>\"},{\"type\":\"const_string\",\"value\":\"hello\"}]},{\"type\":\"const_string\",\"value\":\"world\"}]}}";
    let mut m = stag_matcher(json);
    accept_and_stop(&mut m, &[2, 7]);
    let mut m2 = stag_matcher(json);
    accept_and_stop(&mut m2, &[8]);
}

#[test]
fn test_stag_complex_multi_dispatch() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"triggered_tags\",\"triggers\":[\"<tool>\"],\"tags\":[{\"type\":\"tag\",\"begin\":\"<tool>\",\"content\":{\"type\":\"token_triggered_tags\",\"trigger_tokens\":[\"<code>\",\"<think>\"],\"tags\":[{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<code>\"},\"content\":{\"type\":\"any_tokens\"},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}},{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<think>\"},\"content\":{\"type\":\"any_tokens\",\"exclude_tokens\":[16]},\"end\":{\"type\":\"token\",\"token\":\"<think_end>\"}}]},\"end\":\"<end>\"}]}}";
    let mut m = stag_matcher(json);
    accept_tokens(&mut m, &[7, 8]);
    accept_tokens(&mut m, &[2]);
    accept_tokens(&mut m, &[13, 14]);
    accept_tokens(&mut m, &[3, 7, 8, 9, 10, 4]);
    accept_tokens(&mut m, &[7]);
    accept_tokens(&mut m, &[5, 13, 14, 7]);
    assert!(!m.accept_token(16));
    accept_tokens(&mut m, &[6]);
    accept_tokens(&mut m, &[4]);
    accept_tokens(&mut m, &[8]);
    assert!(m.accept_token(1));
    assert!(m.is_terminated());
}

#[test]
fn test_stag_star_of_token_sequence() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"star\",\"content\":{\"type\":\"sequence\",\"elements\":[{\"type\":\"token\",\"token\":\"<tool>\"},{\"type\":\"const_string\",\"value\":\"x\"},{\"type\":\"token\",\"token\":\"<end>\"}]}}}";
    let mut m = stag_matcher(json);
    accept_tokens(&mut m, &[2, 13, 4]);
    accept_tokens(&mut m, &[2, 13, 4]);
    accept_tokens(&mut m, &[2, 13, 4]);
    assert!(m.accept_token(1));
    assert!(m.is_terminated());
}

#[test]
fn test_stag_star_of_token_sequence_zero() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"star\",\"content\":{\"type\":\"sequence\",\"elements\":[{\"type\":\"token\",\"token\":\"<tool>\"},{\"type\":\"const_string\",\"value\":\"x\"}]}}}";
    let mut m = stag_matcher(json);
    assert!(m.accept_token(1));
    assert!(m.is_terminated());
}

#[test]
fn test_stag_multiple_triggered_tags_rounds() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"token_triggered_tags\",\"trigger_tokens\":[\"<tool>\",\"<code>\",\"<think>\"],\"tags\":[{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<tool>\"},\"content\":{\"type\":\"const_string\",\"value\":\"hello\"},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}},{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<code>\"},\"content\":{\"type\":\"exclude_token\",\"exclude_tokens\":[]},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}},{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<think>\"},\"content\":{\"type\":\"any_tokens\"},\"end\":{\"type\":\"token\",\"token\":\"<think_end>\"}}]}}";
    let mut m = stag_matcher(json);
    accept_tokens(&mut m, &[2, 7, 4]);
    accept_tokens(&mut m, &[13, 14, 7]);
    accept_tokens(&mut m, &[3, 8, 4]);
    accept_tokens(&mut m, &[5, 7, 8, 13, 14, 9, 10, 6]);
    accept_tokens(&mut m, &[2, 7, 4]);
    accept_tokens(&mut m, &[8]);
    assert!(m.accept_token(1));
    assert!(m.is_terminated());
}

#[test]
fn test_stag_exclude_token_with_string_excludes() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<tool>\"},\"content\":{\"type\":\"exclude_token\",\"exclude_tokens\":[\"<bad>\",\"<end>\",\"<think>\"]},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}}}";
    let mut m = stag_matcher(json);
    accept_tokens(&mut m, &[2]);
    assert!(!m.accept_token(16));
    assert!(!m.accept_token(4));
    assert!(!m.accept_token(5));
    accept_tokens(&mut m, &[7]);
    accept_and_stop(&mut m, &[4]);
}

#[test]
fn test_stag_token_triggered_string_token_refs() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"token_triggered_tags\",\"trigger_tokens\":[\"<tool>\",\"<code>\"],\"tags\":[{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<tool>\"},\"content\":{\"type\":\"any_tokens\",\"exclude_tokens\":[\"<bad>\"]},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}},{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<code>\"},\"content\":{\"type\":\"const_string\",\"value\":\"y\"},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}}],\"exclude_tokens\":[\"<bad>\"]}}";
    let mut m = stag_matcher(json);
    assert!(!m.accept_token(16));
    accept_tokens(&mut m, &[7]);
    accept_tokens(&mut m, &[2]);
    assert!(!m.accept_token(16));
    accept_tokens(&mut m, &[8, 13, 4]);
    accept_tokens(&mut m, &[3, 14, 4]);
    assert!(m.accept_token(1));
    assert!(m.is_terminated());
}

#[test]
fn test_stag_tag_with_sequence_content_mixed() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<tool>\"},\"content\":{\"type\":\"sequence\",\"elements\":[{\"type\":\"token\",\"token\":\"<code>\"},{\"type\":\"exclude_token\",\"exclude_tokens\":[16]},{\"type\":\"const_string\",\"value\":\"x\"},{\"type\":\"any_tokens\",\"exclude_tokens\":[16]}]},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}}}";
    let mut m = stag_matcher(json);
    accept_tokens(&mut m, &[2]);
    accept_tokens(&mut m, &[3]);
    assert!(!m.accept_token(16));
    accept_tokens(&mut m, &[7]);
    accept_tokens(&mut m, &[13]);
    assert!(!m.accept_token(16));
    accept_tokens(&mut m, &[8, 14, 5]);
    accept_and_stop(&mut m, &[4]);
}

#[test]
fn test_stag_or_between_token_triggered_and_string_triggered() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"or\",\"elements\":[{\"type\":\"token_triggered_tags\",\"trigger_tokens\":[\"<tool>\"],\"tags\":[{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<tool>\"},\"content\":{\"type\":\"const_string\",\"value\":\"hello\"},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}}],\"stop_after_first\":true,\"at_least_one\":true},{\"type\":\"triggered_tags\",\"triggers\":[\"fn(\"],\"tags\":[{\"type\":\"tag\",\"begin\":\"fn(\",\"content\":{\"type\":\"any_text\"},\"end\":\")\"}],\"stop_after_first\":true,\"at_least_one\":true}]}}";
    let mut m = stag_matcher(json);
    accept_and_stop(&mut m, &[2, 7, 4]);
    let mut m2 = stag_matcher(json);
    accept_tokens(&mut m2, &[11]);
    accept_tokens(&mut m2, &[7, 8]);
    accept_and_stop(&mut m2, &[12]);
}

#[test]
fn test_stag_deeply_nested_three_layers() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"triggered_tags\",\"triggers\":[\"fn(\"],\"tags\":[{\"type\":\"tag\",\"begin\":\"fn(\",\"content\":{\"type\":\"token_triggered_tags\",\"trigger_tokens\":[\"<think>\",\"<code>\"],\"tags\":[{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<think>\"},\"content\":{\"type\":\"any_tokens\",\"exclude_tokens\":[\"<bad>\"]},\"end\":{\"type\":\"token\",\"token\":\"<think_end>\"}},{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<code>\"},\"content\":{\"type\":\"exclude_token\",\"exclude_tokens\":[\"<bad>\"]},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}}],\"exclude_tokens\":[\"<bad>\"]},\"end\":\")\"}]}}";
    let mut m = stag_matcher(json);
    accept_tokens(&mut m, &[7, 8]);
    accept_tokens(&mut m, &[11]);
    accept_tokens(&mut m, &[13]);
    assert!(!m.accept_token(16));
    accept_tokens(&mut m, &[5, 7, 8, 13, 14]);
    assert!(!m.accept_token(16));
    accept_tokens(&mut m, &[6]);
    accept_tokens(&mut m, &[14]);
    accept_tokens(&mut m, &[3]);
    assert!(!m.accept_token(16));
    accept_tokens(&mut m, &[7]);
    accept_tokens(&mut m, &[4]);
    accept_tokens(&mut m, &[12]);
    accept_tokens(&mut m, &[8]);
    assert!(m.accept_token(1));
    assert!(m.is_terminated());
}

#[test]
fn test_stag_any_tokens_all_excluded_except_end() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<tool>\"},\"content\":{\"type\":\"any_tokens\",\"exclude_tokens\":[0,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16]},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}}}";
    let mut m = stag_matcher(json);
    accept_tokens(&mut m, &[2]);
    accept_and_stop(&mut m, &[4]);
}

#[test]
fn test_stag_token_tag_with_or_content() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<tool>\"},\"content\":{\"type\":\"or\",\"elements\":[{\"type\":\"const_string\",\"value\":\"hello\"},{\"type\":\"sequence\",\"elements\":[{\"type\":\"exclude_token\",\"exclude_tokens\":[16]},{\"type\":\"const_string\",\"value\":\"world\"}]}]},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}}}";
    let mut m = stag_matcher(json);
    accept_and_stop(&mut m, &[2, 7, 4]);
    let mut m2 = stag_matcher(json);
    accept_tokens(&mut m2, &[2]);
    accept_tokens(&mut m2, &[13]);
    accept_tokens(&mut m2, &[8]);
    accept_and_stop(&mut m2, &[4]);
}

#[test]
fn test_stag_mixed_begin_end_types() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"token_triggered_tags\",\"trigger_tokens\":[\"<tool>\",\"<code>\",\"<think>\"],\"tags\":[{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<tool>\"},\"content\":{\"type\":\"const_string\",\"value\":\"hello\"},\"end\":\"<end>\"},{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<code>\"},\"content\":{\"type\":\"exclude_token\",\"exclude_tokens\":[\"<bad>\"]},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}},{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<think>\"},\"content\":{\"type\":\"any_tokens\",\"exclude_tokens\":[\"<bad>\"]},\"end\":\"<think_end>\"}]}}";
    let mut m = stag_matcher(json);
    accept_tokens(&mut m, &[2]);
    accept_tokens(&mut m, &[7]);
    accept_tokens(&mut m, &[4]);
    accept_tokens(&mut m, &[13]);
    accept_tokens(&mut m, &[3]);
    assert!(!m.accept_token(16));
    accept_tokens(&mut m, &[7]);
    accept_tokens(&mut m, &[4]);
    accept_tokens(&mut m, &[5]);
    assert!(!m.accept_token(16));
    accept_tokens(&mut m, &[7, 8, 13]);
    accept_tokens(&mut m, &[6]);
    assert!(m.accept_token(1));
    assert!(m.is_terminated());
}

#[test]
fn test_stag_repeated_token_triggered_tags_different_tags() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"token_triggered_tags\",\"trigger_tokens\":[\"<tool>\",\"<code>\",\"<think>\"],\"tags\":[{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<tool>\"},\"content\":{\"type\":\"const_string\",\"value\":\"x\"},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}},{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<code>\"},\"content\":{\"type\":\"const_string\",\"value\":\"y\"},\"end\":{\"type\":\"token\",\"token\":\"<end>\"}},{\"type\":\"tag\",\"begin\":{\"type\":\"token\",\"token\":\"<think>\"},\"content\":{\"type\":\"const_string\",\"value\":\"hello\"},\"end\":{\"type\":\"token\",\"token\":\"<think_end>\"}}]}}";
    let mut m = stag_matcher(json);
    accept_tokens(&mut m, &[2, 13, 4]);
    accept_tokens(&mut m, &[7]);
    accept_tokens(&mut m, &[3, 14, 4]);
    accept_tokens(&mut m, &[5, 7, 6]);
    accept_tokens(&mut m, &[2, 13, 4]);
    accept_tokens(&mut m, &[7]);
    accept_tokens(&mut m, &[3, 14, 4]);
    accept_tokens(&mut m, &[5, 7, 6]);
    accept_tokens(&mut m, &[2, 13, 4]);
    accept_tokens(&mut m, &[7]);
    accept_tokens(&mut m, &[3, 14, 4]);
    accept_tokens(&mut m, &[5, 7, 6]);
    assert!(m.accept_token(1));
    assert!(m.is_terminated());
}

#[test]
fn test_stag_any_tokens_excludes_allow_empty_end() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"token_triggered_tags\",\"trigger_tokens\":[\"<tool>\"],\"tags\":[{\"begin\":{\"type\":\"token\",\"token\":\"<tool>\"},\"content\":{\"type\":\"any_tokens\",\"exclude_tokens\":[\"<tool>\",\"<bad>\"]},\"end\":\"\"}]}}";
    let mut m = stag_matcher(json);
    accept_tokens(&mut m, &[2, 7, 8]);
    accept_tokens(&mut m, &[2, 13, 14]);
    assert!(m.accept_token(1));
    assert!(m.is_terminated());
}

#[test]
fn test_stag_any_tokens_exclude_redispatch() {
    let json = "{\"type\":\"structural_tag\",\"format\":{\"type\":\"token_triggered_tags\",\"trigger_tokens\":[\"<tool>\"],\"tags\":[{\"begin\":{\"type\":\"token\",\"token\":\"<tool>\"},\"content\":{\"type\":\"any_tokens\",\"exclude_tokens\":[\"<tool>\"]},\"end\":\"\"}]}}";
    let mut m = stag_matcher(json);
    accept_tokens(&mut m, &[2, 7]);
    accept_tokens(&mut m, &[2, 8]);
    accept_tokens(&mut m, &[2, 13]);
    assert!(m.accept_token(1));
    assert!(m.is_terminated());
}

// --- Lookahead assertion + kToken, and rollback ---

#[test]
fn test_lookahead_exact_with_token_set() {
    let vocab = ["<s>", "</s>", "abc", "abcd", "X"];
    let mut m =
        make_matcher(&vocab, "rule_a ::= [a-z]+\nroot ::= rule_a Token(4)\n");
    assert_eq!(rejected(&mut m, 5), BTreeSet::from([0, 1, 4]));
}

#[test]
fn test_lookahead_token_set_suffix_nonempty_rejected() {
    let vocab = ["<s>", "</s>", "ab", "a", "X"];
    let mut m =
        make_matcher(&vocab, "rule_a ::= \"a\"\nroot ::= rule_a Token(4)\n");
    assert_eq!(rejected(&mut m, 5), BTreeSet::from([0, 1, 2, 4]));
}

#[test]
fn test_lookahead_mixed_char_and_token() {
    let vocab = ["<s>", "</s>", "abc", "abc!", "X"];
    let mut m = make_matcher(
        &vocab,
        "rule_a ::= [a-z]+\nroot ::= rule_a \"!\" Token(4)\n",
    );
    assert_eq!(rejected(&mut m, 5), BTreeSet::from([0, 1, 4]));
}

#[test]
fn test_rollback() {
    let vocab = [
        "<s>", "</s>", "<tool>", "<code>", "hello", "world", "fn(", ")", "x",
        "y",
    ];
    let grammar = "arg ::= [a-z]+\ncall ::= \"fn(\" Token(8, 9) \",\" arg \")\"\nroot ::= TokenTagDispatch(\n  (2, call),\n  excludes=(3,)\n)";
    let mut m = make_matcher(&vocab, grammar);

    let mask_0 = accepted(&mut m, 10);
    assert!(m.accept_token(2)); // <tool> trigger
    let mask_1 = accepted(&mut m, 10);
    assert!(m.accept_token(6)); // fn(
    let mask_2 = accepted(&mut m, 10);
    assert!(m.accept_token(8)); // x (Token edge)
    let mask_3 = accepted(&mut m, 10);

    // Rollback all 3 tokens.
    m.rollback(3);
    assert_eq!(accepted(&mut m, 10), mask_0);

    // Re-accept and verify masks match.
    assert!(m.accept_token(2));
    assert_eq!(accepted(&mut m, 10), mask_1);
    assert!(m.accept_token(6));
    assert_eq!(accepted(&mut m, 10), mask_2);
    assert!(m.accept_token(8));
    assert_eq!(accepted(&mut m, 10), mask_3);

    // Rollback 2, then continue on a different path.
    m.rollback(2);
    assert_eq!(accepted(&mut m, 10), mask_1);
    assert!(m.accept_token(6));
    assert!(m.accept_token(9)); // y instead of x
    assert_eq!(accepted(&mut m, 10), mask_3); // same: need ","

    // Rollback 1 past the token edge, re-accept.
    m.rollback(1);
    assert_eq!(accepted(&mut m, 10), mask_2);
    assert!(m.accept_token(8));
    assert_eq!(accepted(&mut m, 10), mask_3);
}

// --- End-to-end nested-dispatch tests (step-DSL over a fresh matcher per path). ---

/// One step's expectation: the accepted-token set after the step, a reject (the token must
/// not be accepted), or a stop (accept the token, then require termination).
enum E {
    Set(&'static [i32]),
    Reject,
    Stop,
}

/// Runs one path of `(token, expectation)` steps over a fresh matcher. A `None` token only
/// checks the current accepted set; `trailing_stop` appends an accept-stop-token + terminate
/// after all steps (the `test_e2e_complex` driver shape).
fn run_e2e(
    vocab: &[&str],
    grammar: &str,
    vocab_size: i32,
    steps: &[(Option<i32>, E)],
    trailing_stop: bool,
) {
    let mut m = make_matcher(vocab, grammar);
    for (tok, exp) in steps {
        match exp {
            E::Set(want) => {
                if let Some(t) = tok {
                    assert!(m.accept_token(*t), "failed to accept token {t}");
                }
                let want: BTreeSet<i32> = want.iter().copied().collect();
                assert_eq!(accepted(&mut m, vocab_size), want);
            },
            E::Reject => {
                assert!(!m.accept_token(tok.unwrap()));
            },
            E::Stop => {
                assert!(m.accept_token(tok.unwrap()));
                assert!(m.is_terminated());
            },
        }
    }
    if trailing_stop {
        assert!(m.accept_token(1));
        assert!(m.is_terminated());
    }
}

#[test]
fn test_e2e_complex() {
    let vocab = [
        "<s>",
        "</s>",
        "<tool>",
        "<code>",
        "<blocked>",
        "hello",
        "he",
        "name",
        "val",
        "x",
        "y",
        "{",
        "}",
        ":",
        ",",
        "[",
        "]",
        ";",
        "42",
        "a:",
        "{a",
        "a}",
        "a;",
        "fn(",
        ")",
    ];
    let grammar = "
value ::= [a-z]+ | [0-9]+
entry ::= [a-z]+ \":\" value
inner ::= entry (\";\" entry)*
body ::= \"{\" inner \"}\" | \"[\" inner \"]\"
tool_body ::= body (\",\" body)*
arg ::= [a-z]+
call ::= \"fn(\" Token(9, 10) \",\" arg \")\"
code_body ::= call (\";\" call)*
root ::= TokenTagDispatch(
    (2, tool_body),
    (3, code_body),
    excludes=(4,)
)
";
    let paths: &[&[(Option<i32>, E)]] = &[
        &[
            (
                None,
                E::Set(&[
                    0, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17,
                    18, 19, 20, 21, 22, 23, 24,
                ]),
            ),
            (Some(2), E::Set(&[11, 15, 20])),
            (Some(20), E::Set(&[5, 6, 7, 8, 9, 10, 13, 19])),
            (Some(19), E::Set(&[5, 6, 7, 8, 9, 10, 18, 21, 22])),
            (Some(18), E::Set(&[12, 17, 18])),
            (Some(17), E::Set(&[5, 6, 7, 8, 9, 10, 19])),
            (Some(7), E::Set(&[5, 6, 7, 8, 9, 10, 13, 19])),
            (Some(13), E::Set(&[5, 6, 7, 8, 9, 10, 18, 21, 22])),
            (Some(8), E::Set(&[5, 6, 7, 8, 9, 10, 12, 17, 21, 22])),
            (
                Some(12),
                E::Set(&[
                    0, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17,
                    18, 19, 20, 21, 22, 23, 24,
                ]),
            ),
        ],
        &[
            (
                None,
                E::Set(&[
                    0, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17,
                    18, 19, 20, 21, 22, 23, 24,
                ]),
            ),
            (Some(3), E::Set(&[23])),
            (Some(23), E::Set(&[9, 10])),
            (Some(9), E::Set(&[14])),
            (Some(14), E::Set(&[5, 6, 7, 8, 9, 10])),
            (Some(7), E::Set(&[5, 6, 7, 8, 9, 10, 24])),
            (
                Some(24),
                E::Set(&[
                    0, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17,
                    18, 19, 20, 21, 22, 23, 24,
                ]),
            ),
        ],
    ];
    for steps in paths {
        run_e2e(&vocab, grammar, 25, steps, true);
    }
}

#[test]
fn test_e2e_nested_dispatch() {
    let vocab = [
        "<s>",
        "</s>",
        "<outer>",
        "<inner>",
        "<o_block>",
        "<i_block>",
        "hello",
        "world",
        "fn(",
        ")",
        "x",
        "y",
    ];
    let grammar = "
    leaf ::= Token(10, 11)
    inner ::= TokenTagDispatch((3, leaf), excludes=(5,))
    tool_fn ::= \"fn(\" inner \")\"
    root ::= TokenTagDispatch((2, tool_fn), excludes=(4,))
    ";
    let paths: &[&[(Option<i32>, E)]] = &[
        &[
            (None, E::Set(&[0, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11])),
            (Some(2), E::Set(&[8])),
            (Some(8), E::Set(&[0, 2, 3, 4, 6, 7, 8, 9, 10, 11])),
            (Some(6), E::Set(&[0, 2, 3, 4, 6, 7, 8, 9, 10, 11])),
            (Some(3), E::Set(&[10, 11])),
            (Some(10), E::Set(&[0, 2, 3, 4, 6, 7, 8, 9, 10, 11])),
            (Some(9), E::Set(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11])),
            (Some(1), E::Stop),
        ],
        &[
            (None, E::Set(&[0, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11])),
            (Some(6), E::Set(&[0, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11])),
            (Some(5), E::Set(&[0, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11])),
            (Some(4), E::Reject),
            (Some(1), E::Stop),
        ],
        &[
            (None, E::Set(&[0, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11])),
            (Some(4), E::Reject),
        ],
        &[
            (Some(2), E::Set(&[8])),
            (Some(8), E::Set(&[0, 2, 3, 4, 6, 7, 8, 9, 10, 11])),
            (Some(4), E::Set(&[0, 2, 3, 4, 6, 7, 8, 9, 10, 11])),
            (Some(9), E::Set(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11])),
            (Some(1), E::Stop),
        ],
    ];
    for steps in paths {
        run_e2e(&vocab, grammar, 12, steps, false);
    }
}

#[test]
fn test_e2e_nested_exclude_loop() {
    let vocab =
        ["<s>", "</s>", "hello", "world", "###", "<END>", "foo", "done"];
    let grammar = "
    loop ::= TokenTagDispatch(excludes=(4, 5))
    root ::= [a-z]+ loop Token(5) [a-z]+
    ";
    let paths: &[&[(Option<i32>, E)]] = &[&[
        (None, E::Set(&[2, 3, 6, 7])),
        (Some(4), E::Reject),
        (Some(2), E::Set(&[0, 2, 3, 5, 6, 7])),
        (Some(0), E::Set(&[0, 2, 3, 5, 6, 7])),
        (Some(3), E::Set(&[0, 2, 3, 5, 6, 7])),
        (Some(5), E::Set(&[2, 3, 6, 7])),
        (Some(7), E::Set(&[1, 2, 3, 6, 7])),
        (Some(1), E::Stop),
    ]];
    for steps in paths {
        run_e2e(&vocab, grammar, 8, steps, false);
    }
}

#[test]
fn test_e2e_mixed_tag_and_token_dispatch() {
    let vocab = [
        "<s>", "</s>", "<call>", "<mid>", "<skip>", "<end>", "<bad>", "hello",
        "world", "x", "y", "done",
    ];
    let grammar = "
    leaf ::= [a-z]+
    inner ::= TagDispatch((\"<end>\", leaf), excludes=(\"<bad>\"))
    mid_body ::= Token(9, 10) inner
    mid ::= TokenTagDispatch((3, mid_body), excludes=(4,))
    root ::= TagDispatch((\"<call>\", mid), excludes=(\"<bad>\"))
    ";
    let paths: &[&[(Option<i32>, E)]] = &[
        &[
            (None, E::Set(&[0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11])),
            (Some(7), E::Set(&[0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11])),
            (Some(2), E::Set(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11])),
            (Some(3), E::Set(&[0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11])),
            (Some(9), E::Set(&[0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11])),
            (Some(8), E::Set(&[0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11])),
            (Some(5), E::Set(&[0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11])),
            (Some(11), E::Set(&[0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11])),
            (Some(1), E::Stop),
        ],
        &[
            (None, E::Set(&[0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11])),
            (Some(7), E::Set(&[0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11])),
            (Some(8), E::Set(&[0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11])),
            (Some(6), E::Reject),
            (Some(1), E::Stop),
        ],
        &[
            (None, E::Set(&[0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11])),
            (Some(6), E::Reject),
        ],
        &[
            (Some(2), E::Set(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11])),
            (Some(6), E::Set(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11])),
        ],
        &[
            (Some(2), E::Set(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11])),
            (Some(3), E::Set(&[0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11])),
            (Some(9), E::Set(&[0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11])),
            (Some(6), E::Reject),
        ],
        &[(Some(4), E::Set(&[0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11]))],
        &[
            (Some(2), E::Set(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11])),
            (Some(3), E::Set(&[0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11])),
            (Some(9), E::Set(&[0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11])),
            (Some(4), E::Set(&[0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11])),
        ],
    ];
    for steps in paths {
        run_e2e(&vocab, grammar, 12, steps, false);
    }
}
