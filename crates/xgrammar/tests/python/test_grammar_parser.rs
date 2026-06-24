//! Port of `xgrammar/tests/python/test_grammar_parser.py`.
//!
//! Covers the EBNF parser via `ebnf_to_grammar_no_normalization` + `Display`. Tests that
//! exercise the grammar-functor passes (`GrammarFunctor`) or normalized `from_ebnf` are
//! added once those land (M3 functors).

use xgrammar::parser::ebnf_to_grammar_no_normalization;

/// Parse without normalization (root rule "root") and render back to EBNF.
fn no_norm(ebnf: &str) -> String {
    ebnf_to_grammar_no_normalization(ebnf, "root")
        .unwrap()
        .to_string()
}

#[test]
fn test_basic_string_literal() {
    assert_eq!(
        no_norm(r#"root ::= "hello""#),
        concat!(r#"root ::= (("hello"))"#, "\n")
    );
}

#[test]
fn test_empty_string() {
    assert_eq!(no_norm(r#"root ::= """#), concat!(r#"root ::= ((""))"#, "\n"));
}

#[test]
fn test_character_class() {
    assert_eq!(no_norm("root ::= [a-z]"), concat!("root ::= (([a-z]))", "\n"));
}

#[test]
fn test_negated_character_class() {
    assert_eq!(
        no_norm("root ::= [^a-z]"),
        concat!("root ::= (([^a-z]))", "\n")
    );
}

#[test]
fn test_complex_character_class() {
    assert_eq!(
        no_norm(r"root ::= [a-zA-Z0-9_-] [\r\n$\x10-o\]\--]"),
        concat!(r"root ::= (([a-zA-Z0-9_\-] [\r\n$\x10-o\]\-\-]))", "\n")
    );
}

#[test]
fn test_sequence() {
    assert_eq!(
        no_norm(r#"root ::= "a" "b" "c""#),
        concat!(r#"root ::= (("a" "b" "c"))"#, "\n")
    );
}

#[test]
fn test_choice() {
    assert_eq!(
        no_norm(r#"root ::= "a" | "b" | "c""#),
        concat!(r#"root ::= (("a") | ("b") | ("c"))"#, "\n")
    );
}

#[test]
fn test_grouping() {
    assert_eq!(
        no_norm(r#"root ::= ("a" "b") | ("c" "d")"#),
        concat!(r#"root ::= (((("a" "b"))) | ((("c" "d"))))"#, "\n")
    );
}

#[test]
fn test_star_quantifier_simple() {
    assert_eq!(
        no_norm(r#"root ::= "a"*"#),
        concat!(
            r#"root ::= ((root_1))"#,
            "\n",
            r#"root_1 ::= ("" | ("a" root_1))"#,
            "\n"
        )
    );
}

#[test]
fn test_plus_quantifier() {
    assert_eq!(
        no_norm(r#"root ::= "a"+"#),
        concat!(
            r#"root ::= ((root_1))"#,
            "\n",
            r#"root_1 ::= (("a" root_1) | "a")"#,
            "\n"
        )
    );
}
