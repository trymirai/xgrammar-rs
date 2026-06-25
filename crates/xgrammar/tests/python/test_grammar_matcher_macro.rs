//! Port of `xgrammar/tests/python/test_grammar_matcher_macro.py`.
//!
//! Tag-dispatch acceptance + mask generation through the matcher (all pure, in-memory
//! tokenizers).

use xgrammar::{
    compiler::GrammarCompiler,
    grammar::Grammar,
    matcher::{
        GrammarMatcher, allocate_token_bitmask, get_masked_tokens_from_bitmask,
    },
    tokenizer::{TokenizerInfo, VocabType},
};

/// `_is_grammar_accept_string`: accept then require termination.
fn accepts(
    grammar: &str,
    input: &str,
) -> bool {
    let g = Grammar::from_ebnf(grammar, "root").unwrap();
    let mut m = GrammarMatcher::from_grammar(&g, true);
    m.accept_string(input) && m.is_terminated()
}

/// `_is_grammar_accept_string(..., require_termination=False)`.
fn accepts_no_term(
    grammar: &str,
    input: &str,
) -> bool {
    let g = Grammar::from_ebnf(grammar, "root").unwrap();
    let mut m = GrammarMatcher::from_grammar(&g, true);
    m.accept_string(input)
}

#[test]
fn test_simple() {
    let g = "root ::= TagDispatch((\"tag1\", rule1), (\"tag2\", rule2))\n\
        rule1 ::= \"abcd\"\nrule2 ::= \"efg\"\n";
    assert!(accepts(g, "tag1abcd"));
    assert!(accepts(g, "tag1abcdtag2efg"));
    assert!(accepts(g, "tag1abcdqqqqtag2efg"));
    assert!(!accepts(g, "tag1abc"));
    assert!(!accepts(g, "tag1abce"));
    assert!(!accepts(g, "ttag1abd"));
}

#[test]
fn test_complex_rule() {
    let g = "root ::= TagDispatch((\"tag1\", rule1), (\"tag2\", rule2))\n\
        rule1 ::= \"abcd\" [p]*\nrule2 ::= \"efg\" [t]*\n";
    assert!(accepts(g, "tag1abcd"));
    assert!(accepts(g, "tag1abcdppppptag2efg"));
    assert!(accepts(g, "tag2efgtttttag1abc"));
    assert!(!accepts(g, "tag1efg"));
}

#[test]
fn test_no_loop_after_dispatch() {
    let g = "root ::= TagDispatch((\"tag1\", rule1), (\"tag2\", rule2), loop_after_dispatch=false)\n\
        rule1 ::= \"abcd\" [p]*\nrule2 ::= \"efg\" [t]*\n";
    assert!(accepts(g, "tag1abcd"));
    assert!(accepts(g, "tag2efgttt"));
    assert!(!accepts(g, "tag1abcdppppptag2"));
    assert!(!accepts(g, "tag2efgtag1"));
}

#[test]
fn test_stop_str() {
    let g = "root ::= root1 stop \"w\"\n\
        root1 ::= TagDispatch((\"tag1\", rule1), (\"tag2\", rule2), excludes=(\"tag3\", \"ll\"))\n\
        stop ::= \"tag3\" | \"ll\"\nrule1 ::= \"abcd\" [p]*\nrule2 ::= \"efg\" [t]*\n";
    assert!(accepts(g, "tag1abcdllw"));
    assert!(accepts(g, "tag1abcdtag3w"));
    assert!(accepts(g, "tag1abcdqqqtag2efgtag3w"));
    assert!(accepts_no_term(g, "tag1abcd"));
    assert!(accepts_no_term(g, "tag2efgttt"));
    assert!(!accepts(g, "tag1abcd"));
    assert!(!accepts(g, "tag2efgttt"));
    assert!(!accepts(g, "tag1abce"));
    assert!(!accepts_no_term(g, "tag1abcdlltag3w"));
}

#[test]
fn test_stop_str_no_loop() {
    let g = "root ::= root1 stop \"w\"\n\
        root1 ::= TagDispatch((\"tag1\", rule1), (\"tag2\", rule2), excludes=(\"tag3\", \"ll\"), loop_after_dispatch=false)\n\
        stop ::= \"tag3\" | \"ll\"\nrule1 ::= \"abcd\" [p]*\nrule2 ::= \"efg\" [t]*\n";
    assert!(accepts(g, "tag1abcdllw"));
    assert!(accepts(g, "tag1abcdtag3w"));
    assert!(accepts_no_term(g, "tag1abcd"));
    assert!(accepts_no_term(g, "tag2efgttt"));
    assert!(!accepts(g, "tag1abcdqqqtag2efgtag3w"));
    assert!(!accepts(g, "tag1abcd"));
    assert!(!accepts(g, "tag2efgttt"));
    assert!(!accepts(g, "tag1abce"));
    assert!(!accepts_no_term(g, "tag1abcdlltag3w"));
}

#[test]
fn test_tag_dispatch_mask_generation_correctness() {
    let g = "root ::= TagDispatch((\"tag1\", rule1), (\"tag2\", rule2))\n\
        rule1 ::= \"abc\"\nrule2 ::= \"dg\"\n";
    let tokens = [
        "a", "b", "c", "d", "g", "t", "1", "2", "1a", "2d", "2a", "2dgt",
        "2dgtag1a", "2dgtag1b", "tag1a", "tag1b", "c哈哈t", "q", "abcdef",
    ];
    let input_str = "tag1abcqqtag2dgq";
    let full: Vec<&str> = vec![
        "a", "b", "c", "d", "g", "t", "1", "2", "1a", "2d", "2a", "2dgt",
        "2dgtag1a", "tag1a", "c哈哈t", "q", "abcdef",
    ];
    let full_no_2a: Vec<&str> = vec![
        "a", "b", "c", "d", "g", "t", "1", "2", "1a", "2d", "2dgt", "2dgtag1a",
        "tag1a", "c哈哈t", "q", "abcdef",
    ];
    let expected: Vec<Vec<&str>> = vec![
        full.clone(),
        full.clone(),
        full.clone(),
        full_no_2a.clone(),
        vec!["a", "abcdef"],
        vec!["b"],
        vec!["c", "c哈哈t"],
        full.clone(),
        full.clone(),
        full.clone(),
        full.clone(),
        full.clone(),
        full_no_2a.clone(),
        vec!["d"],
        vec!["g"],
        full.clone(),
        full.clone(),
    ];

    let grammar = Grammar::from_ebnf(g, "root").unwrap();
    let v: Vec<String> = tokens.iter().map(|s| (*s).to_owned()).collect();
    let info = TokenizerInfo::new(&v, VocabType::Raw, None, None, false);
    let vocab_size = info.vocab_size();
    let compiler = GrammarCompiler::new(info, 1, true, -1);
    let compiled = compiler.compile_grammar(&grammar);
    let mut m = GrammarMatcher::from_compiled_grammar(&compiled, true);
    let mut mask = allocate_token_bitmask(1, vocab_size);

    let chars: Vec<char> = input_str.chars().collect();
    for (i, c) in input_str.chars().chain(std::iter::once('0')).enumerate() {
        m.fill_next_token_bitmask(&mut mask, 0).unwrap();
        let rejected: std::collections::HashSet<i32> =
            get_masked_tokens_from_bitmask(&mask, vocab_size, 0)
                .into_iter()
                .collect();
        // Accepted token names in ascending-id order (matches the upstream sorted set).
        let accepted: Vec<&str> = (0..vocab_size)
            .filter(|t| !rejected.contains(t))
            .map(|t| tokens[t as usize])
            .collect();
        if i < chars.len() {
            assert!(m.accept_string(&c.to_string()));
        }
        assert_eq!(accepted, expected[i], "step {i}");
    }
}

#[test]
fn test_regression_multiple_tag_dispatch() {
    let g = "root ::= root1 \"w\"\n\
        root1 ::= TagDispatch((\"tag1\", rule1), (\"tag2\", rule2), loop_after_dispatch=false)\n\
        rule1 ::= rule1_dispatch rule1_stop\n\
        rule1_dispatch ::= TagDispatch((\"tag1\", rule2), (\"tag2\", rule3), excludes=(\"tag3\", \"ll\"), loop_after_dispatch=true)\n\
        rule1_stop ::= \"tag3\" | \"ll\"\nrule2 ::= \"efg\" [t]*\nrule3 ::= \"abcd\" [p]*\n";
    assert!(accepts(g, "tag1tag1efgllw"));
    assert!(accepts(g, "tag1tag2abcdtag3w"));
    assert!(!accepts(g, "tag1Ktag2abcdtag3tag1"));
    assert!(accepts(g, "tag1tag3w"));
    assert!(!accepts(g, "tag1tag3tag2abcdll"));
}

#[test]
fn test_excluded_str() {
    let g = "root ::= root_dispatch end_tag\n\
        root_dispatch ::= TagDispatch((\"start\", rule1), excludes=(\"</think>\", \"</conclude>\"), loop_after_dispatch=true)\n\
        rule1 ::= \"12345\"\nend_tag ::= \"</think>\"\n";
    let grammar = Grammar::from_ebnf(g, "root").unwrap();
    assert!(
        grammar
            .to_string()
            .contains("excludes=(\"</think>\", \"</conclude>\")")
    );
    assert!(accepts(g, "start12345</think>"));
    assert!(!accepts(g, "start12345</conclude>"));
    assert!(accepts(g, "start12345abc</think>"));
    assert!(!accepts(g, "start12345</conclude>abc"));
}
