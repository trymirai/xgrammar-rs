//! Port of `external/xgrammar/tests/cpp/test_regular_grammar_fsm.cc`.
//!
//! Preserves EBNF terminal and call semantics when flattening regular grammars.

use xgrammar::{Grammar, GrammarCompiler, GrammarMatcher, TokenizerInfo, VocabType, functor::GrammarFsmBuilder};

/// The C++ `FromRegularGrammar` default state limit.
const MAX_NUM_STATES: i32 = 100_000;

#[test]
fn test_repeated_calls_have_separate_returns() {
    let grammar = Grammar::from_ebnf("root ::= part \"x\" part \"y\"\npart ::= \"a\" part | \"b\"\n", "root").unwrap();
    let fsm = GrammarFsmBuilder::from_regular_grammar(&grammar, MAX_NUM_STATES).unwrap();
    for text in ["bxby", "abxby", "bxaaby", "aaabxaaaby"] {
        assert!(fsm.accept_string(text), "{text}");
    }
    for text in ["", "by", "bxy", "bxay", "abxay", "bybx", "bxbyy"] {
        assert!(!fsm.accept_string(text), "{text}");
    }
}

#[test]
fn test_recursive_callee_cannot_use_callers_optional_exit() {
    let grammar = Grammar::from_ebnf("root ::= part? \"z\" part*\npart ::= \"a\" part | \"b\"\n", "root").unwrap();
    let fsm = GrammarFsmBuilder::from_regular_grammar(&grammar, MAX_NUM_STATES).unwrap();
    for text in ["z", "bz", "abz", "zb", "zaab", "bzab", "abzbaab"] {
        assert!(fsm.accept_string(text), "{text}");
    }
    for text in ["az", "za", "bza", "aaz", "zba"] {
        assert!(!fsm.accept_string(text), "{text}");
    }
}

#[test]
fn test_regex_fallback_matches_original_grammar() {
    let tokenizer = TokenizerInfo::new(&[], VocabType::RAW, None, None, false).unwrap();
    let compiler = GrammarCompiler::new(tokenizer, 1, false, -1);
    let patterns = [
        r"(ab|c){1,3}d?",
        r"(a*b)+c{2,}",
        r"(ab|){0,3}z",
        r"[é你]+|a\s\D",
        r"[^你]",
        r"^[a-zA-Z0-9\.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9-]+\.[a-zA-Z]{2,}$",
    ];
    let samples = [
        "",
        "ab",
        "abc",
        "abccd",
        "d",
        "cccc",
        "bcc",
        "aabbccc",
        "abcc",
        "acccc",
        "z",
        "abz",
        "ababz",
        "abababz",
        "ababababz",
        "é你é",
        "你",
        "a b",
        "a 1",
        "a\tb",
        "name@example.com",
        "name@example",
        "name@.com",
        "<|close|>",
    ];
    for pattern in patterns {
        let grammar = Grammar::from_regex(pattern).unwrap();
        let fsm = GrammarFsmBuilder::from_regular_grammar(&grammar, MAX_NUM_STATES).unwrap();
        let compiled = compiler.compile_grammar(&grammar).unwrap();
        let mut matcher = GrammarMatcher::from_compiled_grammar(&compiled, true);
        for sample in samples {
            matcher.reset();
            let expected = matcher.accept_string(sample) && matcher.is_terminated();
            assert_eq!(fsm.accept_string(sample), expected, "{pattern} input={sample}");
        }
    }
}

#[test]
fn test_rejects_unsupported_recursion_and_bounds() {
    for source in [
        "root ::= \"a\" root \"b\" | \"\"",
        "root ::= \"a\" child | \"\"\nchild ::= \"b\" root | \"c\"",
        "root ::= Token(1)",
    ] {
        let grammar = Grammar::from_ebnf(source, "root").unwrap();
        assert!(GrammarFsmBuilder::from_regular_grammar(&grammar, MAX_NUM_STATES).is_err(), "{source}");
    }
    let grammar = Grammar::from_regex("(ab){100}").unwrap();
    assert!(GrammarFsmBuilder::from_regular_grammar(&grammar, 10).is_err());
}
