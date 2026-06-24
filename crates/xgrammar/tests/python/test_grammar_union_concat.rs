//! Port of `xgrammar/tests/python/test_grammar_union_concat.py`.
//!
//! `test_grammar_union_with_stag` is added once `Grammar::from_structural_tag` lands (M4).

use xgrammar::grammar::Grammar;

fn ebnf(s: &str) -> Grammar {
    Grammar::from_ebnf(s, "root").unwrap()
}

fn three_grammars() -> [Grammar; 3] {
    [
        ebnf(
            r#"root ::= r1 | r2
r1 ::= "true" | ""
r2 ::= "false" | ""
"#,
        ),
        ebnf(
            r#"root ::= "abc" | r1
r1 ::= "true" | r1
"#,
        ),
        ebnf(
            r#"root ::= r1 | r2 | r3
r1 ::= "true" | r3
r2 ::= "false" | r3
r3 ::= "abc" | ""
"#,
        ),
    ]
}

#[test]
fn test_grammar_union() {
    let grammars = three_grammars();
    let expected = r#"root ::= ((root_1) | (root_2) | (root_3))
root_1 ::= ((r1) | (r2))
r1 ::= ("" | ("true"))
r2 ::= ("" | ("false"))
root_2 ::= (("abc") | (r1_1))
r1_1 ::= (("true") | (r1_1))
root_3 ::= ((r1_2) | (r2_1) | (r3))
r1_2 ::= (("true") | (r3))
r2_1 ::= (("false") | (r3))
r3 ::= ("" | ("abc"))
"#;
    assert_eq!(Grammar::union(&grammars).to_string(), expected);
}

#[test]
fn test_grammar_concat() {
    let grammars = three_grammars();
    let expected = r#"root ::= ((root_1 root_2 root_3))
root_1 ::= ((r1) | (r2))
r1 ::= ("" | ("true"))
r2 ::= ("" | ("false"))
root_2 ::= (("abc") | (r1_1))
r1_1 ::= (("true") | (r1_1))
root_3 ::= ((r1_2) | (r2_1) | (r3))
r1_2 ::= (("true") | (r3))
r2_1 ::= (("false") | (r3))
r3 ::= ("" | ("abc"))
"#;
    assert_eq!(Grammar::concat(&grammars).to_string(), expected);
}
