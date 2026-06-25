//! Port of `xgrammar/tests/python/test_grammar_matcher_regex.py`.
//!
//! Regex acceptance through `Grammar::from_regex` + the matcher (pure). The HF
//! `fill_next_token_bitmask` cases land with the HF tokenizer.

use xgrammar::{grammar::Grammar, matcher::GrammarMatcher};

fn accepts(
    regex: &str,
    input: &str,
) -> bool {
    let g = Grammar::from_regex(regex).unwrap();
    let mut m = GrammarMatcher::from_grammar(&g, true);
    m.accept_string(input) && m.is_terminated()
}

#[test]
fn test_simple() {
    assert!(accepts("abc", "abc"));
    assert!(!accepts("abc", "ab"));
    assert!(!accepts("abc", "abcd"));
}

#[test]
fn test_repetition() {
    let cases = [
        ("aaa", true),
        ("abcbc", true),
        ("bcbcbcbcbc", true),
        ("bcbcbcbcbcbcbcb", true),
        ("d", false),
        ("aaaa", false),
    ];
    for (input, accepted) in cases {
        assert_eq!(
            accepts("(a|[bc]{4,}){2,3}", input),
            accepted,
            "input {input:?}"
        );
    }
}

#[test]
fn test_regex_accept() {
    let regexes = [
        r"abc",
        r"[abc]+",
        r"[a-z0-9]+",
        r"[^abc]+",
        r"a*b+c?",
        r"(abc|def)+",
        r"a{2,4}",
        r"\d+",
        r"\w+",
        r"[A-Z][a-z]*",
        r"[0-9]{3}-[0-9]{3}-[0-9]{4}",
        r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}",
    ];
    for regex in regexes {
        assert!(Grammar::from_regex(regex).is_ok(), "should build {regex:?}");
    }
}

#[test]
fn test_regex_refuse() {
    let regexes = [r"a{,3}", r"a{3,2}", r"[z-a]", r"a++", r"(?=a)", r"(?!a)"];
    for regex in regexes {
        assert!(Grammar::from_regex(regex).is_err(), "should reject {regex:?}");
    }
}

#[test]
#[allow(clippy::type_complexity)]
fn test_advanced() {
    let cases: &[(&str, &str, bool)] = &[
        (r"abc", "abc", true),
        (r"abc", "def", false),
        (r"[abc]+", "aabbcc", true),
        (r"[abc]+", "abcd", false),
        (r"[a-z0-9]+", "abc123", true),
        (r"[a-z0-9]+", "ABC", false),
        (r"[^abc]+", "def", true),
        (r"[^abc]+", "aaa", false),
        (r"[abc]+?abc", "aabc", true),
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
        (r"\d+", "123", true),
        (r"\d+", "abc", false),
        (r"\w+", "abc123", true),
        (r"\w+", "!@#", false),
        (r"[A-Z][a-z]*", "Hello", true),
        (r"[A-Z][a-z]*", "hello", false),
        (r"[0-9]{3}-[0-9]{3}-[0-9]{4}", "123-456-7890", true),
        (r"[0-9]{3}-[0-9]{3}-[0-9]{4}", "12-34-567", false),
        (
            r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,4}",
            "test@email.com",
            true,
        ),
        (
            r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,4}",
            "invalid.email",
            false,
        ),
    ];
    for &(regex, instance, is_accepted) in cases {
        assert_eq!(
            accepts(regex, instance),
            is_accepted,
            "regex {regex:?} instance {instance:?}"
        );
    }
}
