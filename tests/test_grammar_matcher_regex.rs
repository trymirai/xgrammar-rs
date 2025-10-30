use serial_test::serial;
use xgrammar::{
    Grammar, GrammarCompiler, GrammarMatcher, TokenizerInfo, VocabType,
};

fn matcher_from_grammar(grammar: &Grammar) -> GrammarMatcher {
    let empty_vocab: Vec<&str> = vec![];
    let stop_ids: Option<Box<[i32]>> = None;
    let tokenizer_info =
        TokenizerInfo::new(&empty_vocab, VocabType::RAW, &stop_ids, false);
    let mut compiler = GrammarCompiler::new(&tokenizer_info, 1, false, -1);
    let compiled = compiler.compile_grammar(grammar);
    GrammarMatcher::new(&compiled, None, true, -1)
}

fn is_grammar_accept_string(
    grammar: &Grammar,
    input: &str,
) -> bool {
    let mut matcher = matcher_from_grammar(grammar);
    let accepted = matcher.accept_string(input, false);
    accepted && matcher.is_terminated()
}

#[test]
#[serial]
fn test_simple() {
    let g = Grammar::from_regex("abc", false);
    assert!(is_grammar_accept_string(&g, "abc"));
    assert!(!is_grammar_accept_string(&g, "ab"));
    assert!(!is_grammar_accept_string(&g, "abcd"));
}

#[test]
#[serial]
fn test_repetition() {
    let g = Grammar::from_regex("(a|[bc]{4,}){2,3}", false);
    let cases = [
        ("aaa", true),
        ("abcbc", true),
        ("bcbcbcbcbc", true),
        ("bcbcbcbcbcbcbcb", true),
        ("d", false),
        ("aaaa", false),
    ];
    for (input, accepted) in cases {
        assert_eq!(is_grammar_accept_string(&g, input), accepted, "{}", input);
    }
}

#[test]
#[serial]
fn test_regex_accept() {
    let patterns = [
        r"abc",
        r"[abc]+",
        r"[a-z0-9]+",
        r"[^abc]+",
        r"a*b+c?",
        r"(abc|def)+",
        r"a{2,4}",
        r"[A-Z][a-z]*",
        r"[0-9]{3}-[0-9]{3}-[0-9]{4}",
    ];
    for p in patterns {
        let _ = Grammar::from_regex(p, false);
    }
}

#[test]
#[serial]
fn test_advanced() {
    let cases = [
        (r"abc", "abc", true),
        (r"abc", "def", false),
        (r"[abc]+", "aabbcc", true),
        (r"[abc]+", "abcd", false),
        (r"[a-z0-9]+", "abc123", true),
        (r"[a-z0-9]+", "ABC", false),
        (r"[^abc]+", "def", true),
        (r"[^abc]+", "aaa", false),
        (r"a*b+c?", "b", true),
        (r"a*b+c?", "aaabbc", true),
        (r"a*b+c?", "c", false),
        (r"(abc|def)+", "abcdef", true),
        (r"(abc|def)+", "abcabc", true),
        (r"(abc|def)+", "ab", false),
        (r"a{2,4}", "aa", true),
        (r"a{2,4}", "aaaa", true),
        (r"a{2,4}", "a", false),
        (r"a{2,4}", "aaaaa", false),
        (r"[A-Z][a-z]*", "Hello", true),
        (r"[A-Z][a-z]*", "hello", false),
        (r"[0-9]{3}-[0-9]{3}-[0-9]{4}", "123-456-7890", true),
        (r"[0-9]{3}-[0-9]{3}-[0-9]{4}", "12-34-567", false),
    ];
    for (regex, instance, expected) in cases {
        let g = Grammar::from_regex(regex, false);
        assert_eq!(
            is_grammar_accept_string(&g, instance),
            expected,
            "{} {}",
            regex,
            instance
        );
    }
}
