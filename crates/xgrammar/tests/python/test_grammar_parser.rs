//! Port of `xgrammar/tests/python/test_grammar_parser.py`.
//!
//! Covers the EBNF parser via `ebnf_to_grammar_no_normalization` + `Display`. Tests that
//! exercise the grammar-functor passes (`GrammarFunctor`) or normalized `from_ebnf` are
//! added once those land (M3 functors).

use xgrammar::functor::{dead_code_eliminator, rule_inliner, structure_normalizer};
use xgrammar::parser::ebnf_to_grammar_no_normalization;

/// Parse without normalization (root rule "root") and render back to EBNF.
fn no_norm(ebnf: &str) -> String {
    ebnf_to_grammar_no_normalization(ebnf, "root")
        .unwrap()
        .to_string()
}

/// Parse, then run the structure-normalizer pass, and render back to EBNF.
fn normalized(ebnf: &str) -> String {
    structure_normalizer(&ebnf_to_grammar_no_normalization(ebnf, "root").unwrap()).to_string()
}

/// Parse, then run the dead-code-eliminator pass, and render back to EBNF.
fn dead_code(ebnf: &str) -> String {
    dead_code_eliminator(&ebnf_to_grammar_no_normalization(ebnf, "root").unwrap()).to_string()
}

/// Parse, then run the rule-inliner pass, and render back to EBNF.
fn inlined(ebnf: &str) -> String {
    rule_inliner(&ebnf_to_grammar_no_normalization(ebnf, "root").unwrap()).to_string()
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

#[test]
fn test_star_quantifier() {
    let before = r#"root ::= b c d
b ::= [b]*
c ::= "b"*
d ::= ([b] [c] [d] | ([p] [q]))*
e ::= [e]* [f]* | [g]*
"#;
    let expected = r#"root ::= ((b c d))
b ::= (([b]*))
c ::= ((c_1))
d ::= ((d_1))
e ::= (([e]* [f]*) | ([g]*))
c_1 ::= ("" | ("b" c_1))
d_1 ::= ("" | (d_1_1 d_1))
d_1_1 ::= (("b" "c" "d") | ("p" "q"))
"#;
    assert_eq!(normalized(before), expected);

    let before = r#"root ::= [a]* [b]* rule1
rule1 ::= [abc]* [def]*
"#;
    let expected = r#"root ::= (([a]* [b]* rule1))
rule1 ::= (([abc]* [def]*))
"#;
    assert_eq!(normalized(before), expected);
}

#[test]
fn test_repetition_range() {
    let before = r#"root ::= a b c d e f g
a ::= [a]{1,2}
b ::= (a | "b"){1, 5}
c ::= "c" {0 , 2}
d ::= "d" {0,}
e ::= "e" {2, }
f ::= "f" {3}
g ::= "g" {0}
"#;
    let expected = r#"root ::= ((a b c d e f g))
a ::= ((a_1{1, 2}))
b ::= ((b_1{1, 5}))
c ::= ((c_1{0, 2}))
d ::= ((d_1{0, -1}))
e ::= ((e_1{2, -1}))
f ::= ((f_1{3, 3}))
g ::= ((g_1{0, 0}))
a_1 ::= (("a"))
b_1 ::= ((a) | ("b"))
c_1 ::= (("c"))
d_1 ::= (("d"))
e_1 ::= (("e"))
f_1 ::= (("f"))
g_1 ::= (("g"))
"#;
    assert_eq!(normalized(before), expected);
}

#[test]
fn test_lookahead_assertion_with_normalizer() {
    let before = r#"root ::= ((b c d))
b ::= (("abc" [a-z])) (=("abc"))
c ::= (("a") | ("b")) (=[a-z] "b")
d ::= (("ac") | ("b" d_choice)) (="abc")
d_choice ::= (("e") | ("d"))
"#;
    let expected = r#"root ::= ((b c d))
b ::= (("abc" [a-z])) (=("abc"))
c ::= (("a") | ("b")) (=([a-z] "b"))
d ::= (("ac") | ("b" d_choice)) (=("abc"))
d_choice ::= (("e") | ("d"))
"#;
    assert_eq!(normalized(before), expected);
}

#[test]
fn test_dead_code_eliminator() {
    // Basic dead-code elimination.
    assert_eq!(
        dead_code(
            r#"root ::= rule1 | rule2
rule1 ::= "a" | "b"
rule2 ::= "b" | "c"
unused ::= "x" | "y"
"#
        ),
        r#"root ::= ((rule1) | (rule2))
rule1 ::= (("a") | ("b"))
rule2 ::= (("b") | ("c"))
"#
    );

    // Recursive rule references, with an unused recursive cluster.
    assert_eq!(
        dead_code(
            r#"root ::= rule1 | rule2
unused1 ::= unused2 | "x"
unused2 ::= unused1 | "y"
rule1 ::= "a" rule2 | "b"
rule2 ::= "c" rule1 | "d"
"#
        ),
        r#"root ::= ((rule1) | (rule2))
rule1 ::= (("a" rule2) | ("b"))
rule2 ::= (("c" rule1) | ("d"))
"#
    );

    // Complex nested rules with unused branches.
    assert_eq!(
        dead_code(
            r#"root ::= rule1 "x" | rule2
rule1 ::= "a" rule3 | "b"
rule2 ::= "c" | "d" rule4
rule3 ::= "e" | "f"
rule4 ::= "g" | "h"
unused1 ::= "i" unused2
unused2 ::= "j" unused3
unused3 ::= "k" | "l"
"#
        ),
        r#"root ::= ((rule1 "x") | (rule2))
rule1 ::= (("a" rule3) | ("b"))
rule2 ::= (("c") | ("d" rule4))
rule3 ::= (("e") | ("f"))
rule4 ::= (("g") | ("h"))
"#
    );
}

#[test]
fn test_rule_inliner() {
    assert_eq!(
        inlined(
            r#"root ::= rule1 | rule2
rule1 ::= "a" | "b"
rule2 ::= "b" | "c"
"#
        ),
        r#"root ::= (("a") | ("b") | ("b") | ("c"))
rule1 ::= (("a") | ("b"))
rule2 ::= (("b") | ("c"))
"#
    );

    assert_eq!(
        inlined(
            r#"root ::= rule1 "a" [a-z]* | rule2 "b" "c"
rule1 ::= "a" [a-z]* | "b"
rule2 ::= "b" | "c" [b-c]
"#
        ),
        r#"root ::= (("a" [a-z]* "a" [a-z]*) | ("b" "a" [a-z]*) | ("b" "b" "c") | ("c" [b-c] "b" "c"))
rule1 ::= (("a" [a-z]*) | ("b"))
rule2 ::= (("b") | ("c" [b-c]))
"#
    );
}
