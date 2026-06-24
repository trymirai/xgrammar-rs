//! Port of `xgrammar/tests/python/test_grammar_parser_macro.py`.
//!
//! `test_lookahead_assertion_analyzer_tag_dispatch` is added once the `byte_string_fuser`
//! and `lookahead_assertion_analyzer` passes land.

use xgrammar::grammar::Grammar;
use xgrammar::parser::ebnf_to_grammar_no_normalization;

/// Parse without normalization (root rule "root") and render back to EBNF.
fn no_norm(ebnf: &str) -> String {
    ebnf_to_grammar_no_normalization(ebnf, "root")
        .unwrap()
        .to_string()
}

/// Full `Grammar::from_ebnf` (parse + normalize), rendered back to EBNF.
fn from_ebnf(ebnf: &str) -> String {
    Grammar::from_ebnf(ebnf, "root").unwrap().to_string()
}

/// The parser error message from parsing `ebnf` without normalization.
fn parse_err(ebnf: &str) -> String {
    ebnf_to_grammar_no_normalization(ebnf, "root")
        .unwrap_err()
        .to_string()
}

#[test]
fn test_tag_dispatch() {
    let before = r#"root ::= TagDispatch(
    ("tag1", rule1),
    ("tag2", rule2),
    excludes = ("abc", "def"),
    loop_after_dispatch = false
)
rule1 ::= "a"
rule2 ::= "b"
"#;
    let expected = r#"root ::= ((TagDispatch(
  ("tag1", rule1),
  ("tag2", rule2),
  loop_after_dispatch=false,
  excludes=("abc", "def")
)))
rule1 ::= (("a"))
rule2 ::= (("b"))
"#;
    assert_eq!(no_norm(before), expected);
}

#[test]
fn test_tag_dispatch_default_parameters() {
    let before = r#"root ::= TagDispatch(("tag1", rule1), ("tag2", rule2))
rule1 ::= "a"
rule2 ::= "b"
"#;
    let expected = r#"root ::= ((TagDispatch(
  ("tag1", rule1),
  ("tag2", rule2),
  loop_after_dispatch=true,
  excludes=()
)))
rule1 ::= (("a"))
rule2 ::= (("b"))
"#;
    assert_eq!(no_norm(before), expected);
}

#[test]
fn test_tag_dispatch_end_to_end() {
    let before = r#"root ::= TagDispatch(("tag1", rule1), ("tag2", rule2), ("tag3", rule3))
rule1 ::= "a"
rule2 ::= "b"
rule3 ::= "c"
"#;
    let expected = r#"root ::= TagDispatch(
  ("tag1", rule1),
  ("tag2", rule2),
  ("tag3", rule3),
  loop_after_dispatch=true,
  excludes=()
)
rule1 ::= (("a"))
rule2 ::= (("b"))
rule3 ::= (("c"))
"#;
    assert_eq!(from_ebnf(before), expected);
}

#[test]
fn test_tag_dispatch_end_to_end_complex() {
    let before = r#"root ::= TagDispatch(("tag1", rule1), ("tag2", rule2), ("tag3", rule3))
rule1 ::= ("a" TagDispatch(("tag1", rule2), ("tag2", rule3)) | "zzz")
rule2 ::= TagDispatch(("tag1", rule2), ("tag2", rule3)) | TagDispatch(("tag3", rule2), ("tag4", rule3))
rule3 ::= "c"
"#;
    let expected = r#"root ::= TagDispatch(
  ("tag1", rule1),
  ("tag2", rule2),
  ("tag3", rule3),
  loop_after_dispatch=true,
  excludes=()
)
rule1 ::= (("a" rule1_1) | ("zzz"))
rule2 ::= ((rule2_1) | (rule2_2))
rule3 ::= (("c"))
rule1_1 ::= TagDispatch(
  ("tag1", rule2),
  ("tag2", rule3),
  loop_after_dispatch=true,
  excludes=()
)
rule2_1 ::= TagDispatch(
  ("tag1", rule2),
  ("tag2", rule3),
  loop_after_dispatch=true,
  excludes=()
)
rule2_2 ::= TagDispatch(
  ("tag3", rule2),
  ("tag4", rule3),
  loop_after_dispatch=true,
  excludes=()
)
"#;
    assert_eq!(from_ebnf(before), expected);
}

#[test]
fn test_e2e_tag_dispatch_roundtrip() {
    let before = r#"root ::= TagDispatch(
  ("tag1", rule1),
  ("tag2", rule2),
  ("tag3", rule3),
  loop_after_dispatch=false,
  excludes=()
)
rule1 ::= (("a"))
rule2 ::= (("b"))
rule3 ::= (("c"))
"#;
    let output_string_1 = from_ebnf(before);
    let output_string_2 = from_ebnf(&output_string_1);
    assert_eq!(before, output_string_1);
    assert_eq!(output_string_1, output_string_2);
}

#[test]
fn test_tag_dispatch_parser_errors() {
    let cases = [
        (
            "root ::= TagDispatch((\"\", rule1))\nrule1 ::= \"a\"",
            "EBNF parser error at line 1, column 21: Tag must be a non-empty string literal",
        ),
        (
            "root ::= TagDispatch((\"tag1\", undefined_rule))",
            "EBNF parser error at line 1, column 21: Rule \"undefined_rule\" is not defined",
        ),
        (
            "root ::= TagDispatch(\"tag1\", rule1)",
            "EBNF parser error at line 1, column 21: Each tag dispatch element must be a tuple",
        ),
        (
            "root ::= TagDispatch((\"tag1\" rule1))",
            "EBNF parser error at line 1, column 30: Expect , or ) in tuple",
        ),
        (
            "root ::= TagDispatch((\"tag1\", rule1), stop_str=true)\nrule1 ::= \"a\"",
            "EBNF parser error at line 1, column 21: Unknown named argument for TagDispatch: stop_str",
        ),
        (
            "root ::= TagDispatch((\"tag1\", rule1), stop_eos=false)\nrule1 ::= \"a\"",
            "EBNF parser error at line 1, column 21: Unknown named argument for TagDispatch: stop_eos",
        ),
        (
            "root ::= TagDispatch((\"tag1\", rule1), excludes=(\"tag1\"))\nrule1 ::= \"a\"",
            "EBNF parser error at line 1, column 21: Exclude string must not be a prefix of trigger string: tag1",
        ),
    ];
    for (ebnf, expected) in cases {
        assert_eq!(parse_err(ebnf), expected, "input: {ebnf:?}");
    }
}
