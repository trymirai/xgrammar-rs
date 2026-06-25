//! Port of `xgrammar/tests/python/test_grammar_union_concat.py`.

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

/// The legacy `(tags, triggers)` structural-tag JSON for a single `start`-triggered tag whose
/// content is the object schema `{"arg": string}` (mirrors the Python wrapper's
/// `StructuralTag.from_legacy_structural_tag`).
const STAG_JSON: &str = r#"{"type":"structural_tag","format":{"type":"triggered_tags","triggers":["start"],"tags":[{"type":"tag","begin":"start","content":{"type":"json_schema","json_schema":{"type":"object","properties":{"arg":{"type":"string"}}},"style":"json"},"end":"end"}],"at_least_one":false,"stop_after_first":false,"excludes":[]}}"#;

/// Everything after the `root` line — identical for the union and concat results.
const STAG_BODY: &str = r#"basic_escape ::= (([\"\\/bfnrt]) | ("u" [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9]))
basic_string_sub ::= (("\"") | ([^\0-\x1f\"\\\r\n] basic_string_sub) | ("\\" basic_escape basic_string_sub)) (=([ \n\t]* [,}\]:]))
basic_any ::= ((basic_number) | (basic_string) | (basic_boolean) | (basic_null) | (basic_array) | (basic_object))
basic_integer ::= (("0") | (basic_integer_1 [1-9] [0-9]*))
basic_number ::= ((basic_number_1 basic_number_7 basic_number_3 basic_number_6))
basic_string ::= (("\"" basic_string_sub))
basic_boolean ::= (("true") | ("false"))
basic_null ::= (("null"))
basic_array ::= (("[" [ \n\t]* basic_any basic_array_1 [ \n\t]* "]") | ("[" [ \n\t]* "]"))
basic_object ::= (("{" [ \n\t]* basic_string [ \n\t]* ":" [ \n\t]* basic_any basic_object_1 [ \n\t]* "}") | ("{" [ \n\t]* "}"))
root_0 ::= (("{" [ \n\t]* "\"arg\"" [ \n\t]* ":" [ \n\t]* basic_string [ \n\t]* "}") | ("{" [ \n\t]* "}"))
basic_integer_1 ::= ("" | ("-"))
basic_number_1 ::= ("" | ("-"))
basic_number_2 ::= (([0-9] basic_number_2) | ([0-9]))
basic_number_3 ::= ("" | ("." basic_number_2))
basic_number_4 ::= ("" | ([+\-]))
basic_number_5 ::= (([0-9] basic_number_5) | ([0-9]))
basic_number_6 ::= ("" | ([eE] basic_number_4 basic_number_5))
basic_array_1 ::= ("" | ([ \n\t]* "," [ \n\t]* basic_any basic_array_1))
basic_object_1 ::= ("" | ([ \n\t]* "," [ \n\t]* basic_string [ \n\t]* ":" [ \n\t]* basic_any basic_object_1))
basic_number_7 ::= (("0") | ([1-9] [0-9]*))
triggered_tags_group ::= (("" root_0 "end"))
triggered_tags ::= TagDispatch(
  ("start", triggered_tags_group),
  loop_after_dispatch=true,
  excludes=()
)
root_1 ::= ((triggered_tags))
root_2 ::= (([a-z] root_2) | ([a-z]))
"#;

#[test]
fn test_grammar_union_with_stag() {
    let stag = Grammar::from_structural_tag(STAG_JSON).unwrap();
    let start = ebnf("root ::= [a-z] root | [a-z]");

    let expected_union = format!("root ::= ((root_1) | (root_2))\n{STAG_BODY}");
    assert_eq!(
        Grammar::union(&[stag.clone(), start.clone()]).to_string(),
        expected_union
    );

    let expected_concat = format!("root ::= ((root_1 root_2))\n{STAG_BODY}");
    assert_eq!(Grammar::concat(&[stag, start]).to_string(), expected_concat);
}
