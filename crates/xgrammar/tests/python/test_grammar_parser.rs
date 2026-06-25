//! Port of `xgrammar/tests/python/test_grammar_parser.py`.
//!
//! Covers the EBNF parser via `ebnf_to_grammar_no_normalization` + `Display`. Tests that
//! exercise the grammar-functor passes (`GrammarFunctor`) or normalized `from_ebnf` are
//! added once those land (M3 functors).

use xgrammar::{
    functor::{
        dead_code_eliminator, lookahead_assertion_analyzer, rule_inliner,
        structure_normalizer,
    },
    grammar::Grammar,
    parser::ebnf_to_grammar_no_normalization,
};

/// Parse without normalization (root rule "root") and render back to EBNF.
fn no_norm(ebnf: &str) -> String {
    ebnf_to_grammar_no_normalization(ebnf, "root").unwrap().to_string()
}

/// Parse, then run the structure-normalizer pass, and render back to EBNF.
fn normalized(ebnf: &str) -> String {
    structure_normalizer(
        &ebnf_to_grammar_no_normalization(ebnf, "root").unwrap(),
    )
    .to_string()
}

/// Parse, then run the dead-code-eliminator pass, and render back to EBNF.
fn dead_code(ebnf: &str) -> String {
    dead_code_eliminator(
        &ebnf_to_grammar_no_normalization(ebnf, "root").unwrap(),
    )
    .to_string()
}

/// Parse, then run the rule-inliner pass, and render back to EBNF.
fn inlined(ebnf: &str) -> String {
    rule_inliner(&ebnf_to_grammar_no_normalization(ebnf, "root").unwrap())
        .to_string()
}

/// Full `Grammar::from_ebnf` (parse + normalize), rendered back to EBNF.
fn from_ebnf(ebnf: &str) -> String {
    Grammar::from_ebnf(ebnf, "root").unwrap().to_string()
}

/// Parse, then run the lookahead-assertion-analyzer pass, and render back to EBNF.
fn lookahead_analyzed(ebnf: &str) -> String {
    lookahead_assertion_analyzer(
        &ebnf_to_grammar_no_normalization(ebnf, "root").unwrap(),
    )
    .to_string()
}

/// The (lexer or parser) error message from parsing `ebnf` without normalization.
fn parse_err(ebnf: &str) -> String {
    ebnf_to_grammar_no_normalization(ebnf, "root").unwrap_err().to_string()
}

/// The error message from `Grammar::from_ebnf` on `ebnf`.
fn from_ebnf_err(ebnf: &str) -> String {
    Grammar::from_ebnf(ebnf, "root").unwrap_err().to_string()
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

#[test]
fn test_space() {
    let before =
        "\n\nroot::=\"a\"  \"b\" (\"c\"\"d\"\n\"e\") |\n\n\"f\" | \"g\"\n";
    assert_eq!(
        from_ebnf(before),
        concat!(r#"root ::= (("a" "b" "c" "d" "e") | ("f") | ("g"))"#, "\n")
    );
}

#[test]
fn test_nest() {
    assert_eq!(
        from_ebnf(r#"root::= "a" ("b" | "c" "d") | (("e" "f"))"#),
        r#"root ::= (("a" root_1) | ("e" "f"))
root_1 ::= (("b") | ("c" "d"))
"#
    );
}

#[test]
fn test_empty_parentheses() {
    assert_eq!(
        from_ebnf(r#"root ::= "a" ( ) "b""#),
        concat!(r#"root ::= (("a" "b"))"#, "\n")
    );
    assert_eq!(
        from_ebnf(
            r#"root ::= "a" rule1
rule1 ::= ( )
"#
        ),
        r#"root ::= (("a" rule1))
rule1 ::= ("")
"#
    );
}

#[test]
fn test_question_quantifier() {
    assert_eq!(
        no_norm(r#"root ::= "a"?"#),
        concat!(
            r#"root ::= ((root_1))"#,
            "\n",
            r#"root_1 ::= ("" | "a")"#,
            "\n"
        )
    );
}

#[test]
fn test_character_class_star() {
    assert_eq!(
        no_norm("root ::= [a-z]*"),
        concat!("root ::= (([a-z]*))", "\n")
    );
}

#[test]
fn test_repetition_range_exact() {
    assert_eq!(
        no_norm(r#"root ::= "a"{3}"#),
        concat!(
            r#"root ::= ((root_1{3, 3}))"#,
            "\n",
            r#"root_1 ::= "a""#,
            "\n"
        )
    );
}

#[test]
fn test_repetition_range_min_max() {
    assert_eq!(
        no_norm(r#"root ::= "a"{2,4}"#),
        concat!(
            r#"root ::= ((root_1{2, 4}))"#,
            "\n",
            r#"root_1 ::= "a""#,
            "\n"
        )
    );
}

#[test]
fn test_repetition_range_min_only() {
    assert_eq!(
        no_norm(r#"root ::= "a"{2,}"#),
        concat!(
            r#"root ::= ((root_1{2, -1}))"#,
            "\n",
            r#"root_1 ::= "a""#,
            "\n"
        )
    );
}

#[test]
fn test_repetition_range_unbounded_roundtrip() {
    let output_1 = from_ebnf(r#"root ::= "a"{2,}"#);
    assert!(output_1.contains("{2, -1}"));
    assert_eq!(from_ebnf(&output_1), output_1);
}

#[test]
fn test_lookahead_assertion_simple() {
    assert_eq!(
        no_norm(r#"root ::= "a" (="b")"#),
        concat!(r#"root ::= (("a")) (=(("b")))"#, "\n")
    );
}

#[test]
fn test_complex_lookahead() {
    assert_eq!(
        no_norm(r#"root ::= "a" (="b" "c" [0-9])"#),
        concat!(r#"root ::= (("a")) (=(("b" "c" [0-9])))"#, "\n")
    );
}

#[test]
fn test_escape_sequences() {
    assert_eq!(
        no_norm(r#"root ::= "\n\t\r\"\\""#),
        concat!(r#"root ::= (("\n\t\r\"\\"))"#, "\n")
    );
}

#[test]
fn test_unicode_escape() {
    // © and ☃ (U+2603) are non-printable, so they render as hex / unicode escapes.
    // Build the expectation with an explicit backslash char to avoid escaping noise.
    let bs = char::from_u32(0x5c).unwrap();
    let expected = format!(
        r#"root ::= (("ABC{bs}xa9{bs}u2603"))
"#
    );
    assert_eq!(no_norm(r#"root ::= "ABC©☃""#), expected);
}

#[test]
fn test_forward_slash_escape_in_string_literal() {
    assert_eq!(
        no_norm(r#"root ::= "a\/b""#),
        concat!(r#"root ::= (("a/b"))"#, "\n")
    );
}

#[test]
fn test_complex_grammar() {
    let before = r#"root ::= expr
expr ::= term ("+" term | "-" term)*
term ::= factor ("*" factor | "/" factor)*
factor ::= number | "(" expr ")"
number ::= [0-9]+ ("." [0-9]+)?
"#;
    let expected = r#"root ::= ((expr))
expr ::= ((term expr_1))
term ::= ((factor term_1))
factor ::= ((number) | ("(" expr ")"))
number ::= ((number_1 number_3))
expr_1 ::= ("" | ((("+" term) | ("-" term)) expr_1))
term_1 ::= ("" | ((("*" factor) | ("/" factor)) term_1))
number_1 ::= (([0-9] number_1) | [0-9])
number_2 ::= (([0-9] number_2) | [0-9])
number_3 ::= ("" | (("." number_2)))
"#;
    assert_eq!(no_norm(before), expected);
}

#[test]
fn test_lexer_parser_errors() {
    assert!(parse_err(r#"root ::= "a" ""#).contains(
        r#"EBNF lexer error at line 1, column 15: Expect " in string literal"#
    ));
    assert!(parse_err("root ::= [a\n]").contains(
        "EBNF lexer error at line 1, column 12: Character class should not contain newline"
    ));
    assert!(parse_err(r#"root ::= "\@""#).contains(
        "EBNF lexer error at line 1, column 11: Invalid escape sequence"
    ));
    assert!(parse_err(r#"root ::= "\uFF""#).contains(
        "EBNF lexer error at line 1, column 11: Invalid escape sequence"
    ));
    assert!(parse_err(r#"::= "a""#)
        .contains("EBNF lexer error at line 1, column 1: Assign should not be the first token"));
    assert!(parse_err("root ::= a b").contains(
        r#"EBNF parser error at line 1, column 10: Rule "a" is not defined"#
    ));
    assert!(
        parse_err(r#"root ::= "a" |"#)
            .contains("EBNF parser error at line 1, column 15: Expect element")
    );
    assert!(parse_err("root ::= [Z-A]").contains(
        "EBNF parser error at line 1, column 11: Invalid character class: lower bound is larger than upper bound"
    ));
    assert!(parse_err("root ::= \"a\"\nroot ::= \"b\"")
        .contains(r#"EBNF parser error at line 2, column 1: Rule "root" is defined multiple times"#));
    assert!(parse_err(r#"a ::= "a""#).contains(
        r#"EBNF parser error at line 1, column 1: The root rule with name "root" is not found"#
    ));
    assert!(
        parse_err(r#"root ::= "a" (="a") (="b")"#).contains(
            "EBNF parser error at line 1, column 21: Expect rule name"
        )
    );
}

#[test]
fn test_end_to_end_errors() {
    let err = Grammar::from_ebnf(r#"root ::= "a" (=("a" | "b"))"#, "root")
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("Choices in lookahead assertion are not supported")
    );
}

#[test]
fn test_error_consecutive_quantifiers() {
    assert!(from_ebnf_err(r#"root ::= "a"{1,3}{1,3}"#).contains(
        "EBNF parser error at line 1, column 18: Expect element, but got {"
    ));
    assert!(from_ebnf_err(r#"root ::= "a"++"#).contains(
        "EBNF parser error at line 1, column 14: Expect element, but got +"
    ));
    assert!(from_ebnf_err(r#"root ::= "a"??"#).contains(
        "EBNF parser error at line 1, column 14: Expect element, but got ?"
    ));
}

#[test]
fn test_char() {
    let before = "root ::= [a-z] [A-z] \"\\u0234\" \"\\U00000345\\xff\" [-A-Z] [--] [^a] rest
rest ::= [a-zA-Z0-9-] [\\u0234-\\U00000345] [测-试] [\\--\\]]  rest1
rest1 ::= \"\\?\\\"\\'测试あc\" \"👀\" \"\" [a-a] [b-b]
";
    let expected = "root ::= (([a-z] [A-z] \"\\u0234\" \"\\u0345\\xff\" [\\-A-Z] [\\-\\-] [^a] rest))
rest ::= (([a-zA-Z0-9\\-] [\\u0234-\\u0345] [\\u6d4b-\\u8bd5] [\\--\\]] rest1))
rest1 ::= ((\"\\?\\\"\\'\\u6d4b\\u8bd5\\u3042c\" \"\\U0001f440\" \"a\" \"b\"))
";
    assert_eq!(normalized(before), expected);
}

#[test]
fn test_bnf_comment() {
    let before = r#"# top comment
root ::= a b # inline comment
a ::= "a"
b ::= "b"
# bottom comment
"#;
    let expected = r#"root ::= ((a b))
a ::= (("a"))
b ::= (("b"))
"#;
    assert_eq!(no_norm(before), expected);
}

#[test]
fn test_combined_features() {
    let before = r#"root ::= "start" (rule1 | rule2)+ "end"
rule1 ::= [a-z]{1,3} (=":")
rule2 ::= [0-9]+ "." [0-9]*
"#;
    let expected = r#"root ::= (("start" root_1 "end"))
rule1 ::= ((rule1_1{1, 3})) (=((":")))
rule2 ::= ((rule2_1 "." [0-9]*))
root_1 ::= ((((rule1) | (rule2)) root_1) | ((rule1) | (rule2)))
rule1_1 ::= [a-z]
rule2_1 ::= (([0-9] rule2_1) | [0-9])
"#;
    assert_eq!(no_norm(before), expected);
}

#[test]
fn test_nested_quantifiers() {
    let before = "root ::= (\"a\"*)+\n";
    let expected = r#"root ::= ((root_2))
root_1 ::= ("" | ("a" root_1))
root_2 ::= ((((root_1)) root_2) | ((root_1)))
"#;
    assert_eq!(no_norm(before), expected);
}

#[test]
fn test_flatten() {
    let before = r#"root ::= or_test sequence_test nested_test empty_test
or_test ::= ([a] | "b") | "de" | "" | or_test | [^a-z]
sequence_test ::= [a] "a" ("b" ("c" | "d")) ("d" "e") sequence_test ""
nested_test ::= ("a" ("b" ("c" "d"))) | ("a" | ("b" | "c")) | nested_rest
nested_rest ::= ("a" | ("b" "c" | ("d" | "e" "f"))) | ((("g")))
empty_test ::= "d" | (("" | "" "") "" | "a" "") | ("" ("" | "")) "" ""
"#;
    let expected = r#"root ::= ((or_test sequence_test nested_test empty_test))
or_test ::= ("" | ("a") | ("b") | ("de") | (or_test) | ([^a-z]))
sequence_test ::= (("a" "a" "b" sequence_test_1 "d" "e" sequence_test))
nested_test ::= (("a" "b" "c" "d") | ("a") | ("b") | ("c") | (nested_rest))
nested_rest ::= (("a") | ("b" "c") | ("d") | ("e" "f") | ("g"))
empty_test ::= ("" | ("d") | ("a"))
sequence_test_1 ::= (("c") | ("d"))
"#;
    assert_eq!(normalized(before), expected);
}

#[test]
fn test_lookahead_assertion_analyzer() {
    let before = r#"root ::= "a" rule1 "b" rule3 rule5 rule2
rule1 ::= "b"
rule2 ::= "c"
rule3 ::= "" | "d" rule3
rule4 ::= "" | "e" rule4 "f"
rule5 ::= "" | "g" rule5 "h"
"#;
    let expected = r#"root ::= (("a" rule1 "b" rule3 rule5 rule2))
rule1 ::= (("b")) (=("b" rule3 rule5 rule2))
rule2 ::= (("c"))
rule3 ::= (("") | ("d" rule3)) (=(rule5 rule2))
rule4 ::= (("") | ("e" rule4 "f")) (=("f"))
rule5 ::= (("") | ("g" rule5 "h"))
"#;
    assert_eq!(lookahead_analyzed(before), expected);
}
