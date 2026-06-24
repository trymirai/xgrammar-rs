//! Port of `xgrammar/tests/python/test_regex_converter.py`.
//!
//! Covers the `regex_to_ebnf` string output and error messages. The `_is_grammar_accept_string`
//! acceptance assertions and the HF `test_mask_generation` cases are added once the matcher
//! lands (M6).

use xgrammar::converter::regex_to_ebnf;
use xgrammar::grammar::Grammar;

/// Convert a regex to its EBNF text (with the `root ::=` rule name), matching `_regex_to_ebnf`.
fn re(regex: &str) -> String {
    regex_to_ebnf(regex, true).unwrap()
}

/// The error message from converting `regex`.
fn re_err(regex: &str) -> String {
    regex_to_ebnf(regex, true).unwrap_err().to_string()
}

#[test]
fn test_basic() {
    assert_eq!(re("123"), "root ::= \"1\" \"2\" \"3\"\n");
}

#[test]
fn test_unicode() {
    assert_eq!(re("ww我😁"), "root ::= \"w\" \"w\" \"\\u6211\" \"\\U0001f601\"\n");
}

#[test]
fn test_escape() {
    assert_eq!(
        re("\\^\\$\\.\\*\\+\\?\\\\\\(\\)\\[\\]\\{\\}\\|\\/"),
        "root ::= \"^\" \"$\" \".\" \"*\" \"+\" \"\\?\" \"\\\\\" \"(\" \")\" \"[\" \"]\" \"{\" \"}\" \"|\" \"/\"\n"
    );
    assert_eq!(
        re("\\\"\\'\\a\\f\\n\\r\\t\\v\\0\\e"),
        "root ::= \"\\\"\" \"\\'\" \"\\a\" \"\\f\" \"\\n\" \"\\r\" \"\\t\" \"\\v\" \"\\0\" \"\\e\"\n"
    );
    assert_eq!(
        re("\\u{20BB7}\\u0300\\x1F\\cJ"),
        "root ::= \"\\U00020bb7\" \"\\u0300\" \"\\x1f\" \"\\n\"\n"
    );
    assert_eq!(
        re("[\\r\\n\\$\\u0010-\\u006F\\]\\--]+"),
        "root ::= [\\r\\n$\\x10-o\\]\\--]+\n"
    );
}

#[test]
fn test_escaped_char_class() {
    assert_eq!(
        re("\\w\\w\\W\\d\\D\\s\\S"),
        "root ::= [a-zA-Z0-9_] [a-zA-Z0-9_] [^a-zA-Z0-9_] [0-9] [^0-9] \
         [\\f\\n\\r\\t\\v\\u0020\\u00a0] [^[\\f\\n\\r\\t\\v\\u0020\\u00a0]\n"
    );
}

#[test]
fn test_char_class() {
    assert_eq!(re("[-a-zA-Z+--]+"), "root ::= [-a-zA-Z+--]+\n");
}

#[test]
fn test_boundary() {
    assert_eq!(re("^abc$"), "root ::= \"a\" \"b\" \"c\"\n");
}

#[test]
fn test_disjunction() {
    assert_eq!(re("abc|de(f|g)"), "root ::= \"a\" \"b\" \"c\" | \"d\" \"e\" ( \"f\" | \"g\" )\n");
}

#[test]
fn test_space() {
    assert_eq!(
        re(" abc | df | g "),
        "root ::= \" \" \"a\" \"b\" \"c\" \" \" | \" \" \"d\" \"f\" \" \" | \" \" \"g\" \" \"\n"
    );
}

#[test]
fn test_quantifier() {
    assert_eq!(
        re("(a|b)?[a-z]+(abc)*"),
        "root ::= ( \"a\" | \"b\" )? [a-z]+ ( \"a\" \"b\" \"c\" )*\n"
    );
}

#[test]
fn test_consecutive_quantifiers() {
    for regex in ["a{1,3}?{1,3}", "a???", "a++", "a+?{1,3}"] {
        assert!(
            re_err(regex).contains("Two consecutive repetition modifiers are not allowed."),
            "regex: {regex}"
        );
    }
}

#[test]
fn test_group() {
    assert_eq!(re("(a|b)(c|d)"), "root ::= ( \"a\" | \"b\" ) ( \"c\" | \"d\" )\n");
}

#[test]
fn test_any() {
    assert_eq!(
        re(".+a.+"),
        "root ::= [\\u0000-\\U0010FFFF]+ \"a\" [\\u0000-\\U0010FFFF]+\n"
    );
}

#[test]
fn test_ipv4() {
    let regex = "((25[0-5]|2[0-4]\\d|[01]?\\d\\d?).)((25[0-5]|2[0-4]\\d|[01]?\\d\\d?).)\
                 ((25[0-5]|2[0-4]\\d|[01]?\\d\\d?).)(25[0-5]|2[0-4]\\d|[01]?\\d\\d?)";
    let expected = "root ::= ( ( \"2\" \"5\" [0-5] | \"2\" [0-4] [0-9] | [01]? [0-9] [0-9]? ) \
        [\\u0000-\\U0010FFFF] ) ( ( \"2\" \"5\" [0-5] | \"2\" [0-4] [0-9] | [01]? [0-9] \
        [0-9]? ) [\\u0000-\\U0010FFFF] ) ( ( \"2\" \"5\" [0-5] | \"2\" [0-4] [0-9] | [01]? [0-9] \
        [0-9]? ) [\\u0000-\\U0010FFFF] ) ( \"2\" \"5\" [0-5] | \"2\" [0-4] [0-9] | [01]? [0-9] [0-9]? )\n";
    assert_eq!(re(regex), expected);
}

#[test]
fn test_empty_character_class() {
    assert!(re_err("[]").contains("Empty character class is not allowed in regex."));
}

#[test]
fn test_group_modifiers() {
    assert_eq!(re("(?:abc)"), "root ::= ( \"a\" \"b\" \"c\" )\n");
    assert_eq!(re("(?<name>abc)"), "root ::= ( \"a\" \"b\" \"c\" )\n");
    for regex in ["(?=abc)", "(?!abc)", "(?<=abc)", "(?<!abc)", "(?i)abc"] {
        assert!(regex_to_ebnf(regex, true).is_err(), "regex: {regex}");
    }
}

#[test]
fn test_unmatched_parentheses() {
    assert!(re_err("abc)").contains("Unmatched ')'"));
    assert!(re_err("abc((a)").contains("The parenthesis is not closed."));
}

#[test]
fn test_empty_parentheses() {
    assert_eq!(re("()"), "root ::= ( )\n");
    assert_eq!(re("a()b"), "root ::= \"a\" ( ) \"b\"\n");
}

#[test]
fn test_empty_alternative() {
    assert_eq!(re("(a|)"), "root ::= ( \"a\" | \"\" )\n");
    assert_eq!(re("ab(c|)"), "root ::= \"a\" \"b\" ( \"c\" | \"\" )\n");
}

#[test]
fn test_non_greedy_quantifier() {
    assert_eq!(re("a{1,3}?"), "root ::= \"a\"{1,3}\n");
    assert_eq!(re("a+?"), "root ::= \"a\"+\n");
    assert_eq!(re("a*?"), "root ::= \"a\"*\n");
    assert_eq!(re("a??"), "root ::= \"a\"?\n");
}

#[test]
fn test_empty() {
    for regex in ["", "^$", "(())", "()", "^", "$", "()|()"] {
        let grammar = Grammar::from_regex(regex).unwrap();
        assert_eq!(grammar.to_string(), "root ::= (\"\")\n", "regex: {regex}");
    }
}
