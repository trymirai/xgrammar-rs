//! Port of `xgrammar/tests/python/test_grammar_matcher_ebnf.py`.
//!
//! The pure string-acceptance slice is ported here (the HuggingFace bitmask cases land with
//! the tokenizer milestone). `JSON_GRAMMAR` is the EBNF grammar defined inline in the
//! upstream test, not the builtin JSON grammar.

use xgrammar::{grammar::Grammar, matcher::GrammarMatcher};

/// `_is_grammar_accept_string`: accept the whole string, then require termination.
fn accepts_with_root(
    grammar: &str,
    root: &str,
    input: &str,
) -> bool {
    let g = Grammar::from_ebnf(grammar, root).unwrap();
    let mut m = GrammarMatcher::from_grammar(&g, true);
    m.accept_string(input) && m.is_terminated()
}

fn accepts(
    grammar: &str,
    input: &str,
) -> bool {
    accepts_with_root(grammar, "root", input)
}

const JSON_GRAMMAR: &str = r#"
root ::= basic_array | basic_object
basic_any ::= basic_number | basic_string | basic_boolean | basic_null | basic_array | basic_object
basic_integer ::= ("0" | "-"? [1-9] [0-9]*) ".0"?
basic_number ::= ("0" | "-"? [1-9] [0-9]*) ("." [0-9]+)? ([eE] [+-]? [0-9]+)?
basic_string ::= (([\"] basic_string_1 [\"]))
basic_string_1 ::= "" | [^"\\\x00-\x1F] basic_string_1 | "\\" escape basic_string_1
escape ::= ["\\/bfnrt] | "u" [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9]
basic_boolean ::= "true" | "false"
basic_null ::= "null"
basic_array ::= "[" ("" | ws basic_any (ws "," ws basic_any)*) ws "]"
basic_object ::= "{" ("" | ws basic_string ws ":" ws basic_any ( ws "," ws basic_string ws ":" ws basic_any)*) ws "}"
ws ::= [ \n\t]*
"#;

#[test]
fn test_simple() {
    let grammar = "root ::= rule1 rule2\n\
        rule1 ::= (rule2 | rule3) \"a\"\n\
        rule2 ::= \"b\"\n\
        rule3 ::= \"c\"\n";
    assert!(accepts(grammar, "bab"));
    assert!(!accepts(grammar, "abb"));
    assert!(accepts(grammar, "cab"));
}

#[test]
fn test_repetition() {
    let grammar = "root ::= rule {2, 3}\nrule ::= (\"a\" | [bc] {4,})";
    let cases = [
        ("aaa", true),
        ("abcbc", true),
        ("bcbcbcbcbc", true),
        ("bcbcbcbcbcbcbcb", true),
        ("d", false),
        ("aaaa", false),
    ];
    for (input, accepted) in cases {
        assert_eq!(accepts(grammar, input), accepted, "input {input:?}");
    }
}

#[test]
fn test_repetition_with_empty() {
    let grammar =
        "root ::= rule {2, 3} \"d\"?\nrule ::= (\"a\" | [bc] {4,}) | \"\"";
    let cases = [
        ("aaa", true),
        ("abcbc", true),
        ("bcbcbcbcbc", true),
        ("bcbcbcbcbcbcbcb", true),
        ("aaaa", false),
        ("", true),
        ("a", true),
        ("d", true),
    ];
    for (input, accepted) in cases {
        assert_eq!(accepts(grammar, input), accepted, "input {input:?}");
    }
}

#[test]
fn test_utf8() {
    let grammar = "root ::= [，]+";
    for input in
        ["，", "，，，", "，，，，，，，，，，，，，，，，，，，，，，"]
    {
        assert!(accepts(grammar, input), "input {input:?}");
    }
}

#[test]
fn test_custom_root_rule() {
    let grammar = r#"
root ::= basic_object
basic_any ::= basic_string | basic_object
basic_string ::= (([\"] basic_string_1 [\"]))
basic_string_1 ::= "" | [^"\\\r\n] basic_string_1 | "\\" escape basic_string_1
escape ::= ["\\/bfnrt] | "u" [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9]
basic_object ::= "{" ("" | ws basic_string ws ":" ws basic_any ( ws "," ws basic_string ws ":" ws basic_any)*) ws "}"
ws ::= [ \n\t]*
"#;
    assert!(accepts_with_root(grammar, "basic_string", r#""abc\r\n""#));
    assert!(!accepts_with_root(
        grammar,
        "basic_string",
        r#"{"name": "John" }"#
    ));
}

#[test]
fn test_json_accept() {
    let accepted = [
        r#"{"name": "John"}"#,
        r#"{ "name" : "John" }"#,
        "{}",
        "[]",
        r#"{"name": "Alice", "age": 30, "city": "New York"}"#,
        r#"{"name": "Mike", "hobbies": ["reading", "cycling", "hiking"]}"#,
        r#"{"name": "Emma", "address": {"street": "Maple Street", "city": "Boston"}}"#,
        r#"[{"name": "David"}, {"name": "Sophia"}]"#,
        r#"{"name": "William", "age": null, "married": true, "children": ["Liam", "Olivia"], "hasPets": false}"#,
        r#"{"name": "Olivia", "contact": {"email": "olivia@example.com", "address": {"city": "Chicago", "zipcode": "60601"}}}"#,
        r#"{"name": "Liam", "skills": ["Java", "Python"], "experience": [{"company": "CompanyA", "years": 5}, {"company": "CompanyB", "years": 3}]}"#,
        r#"{"person": {"name": "Ethan", "age": 40}, "education": {"degree": "Masters", "university": "XYZ University"}, "work": [{"company": "ABC Corp", "position": "Manager"}, {"company": "DEF Corp", "position": "Senior Manager"}]}"#,
        r#"{"name": "Charlotte", "details": {"personal": {"age": 35, "hobbies": ["gardening", "painting"]}, "professional": {"occupation": "Engineer", "skills": ["CAD", "Project Management"], "projects": [{"name": "Project A", "status": "Completed"}, {"name": "Project B", "status": "In Progress"}]}}}"#,
    ];
    for input in accepted {
        assert!(accepts(JSON_GRAMMAR, input), "should accept {input:?}");
    }
}

#[test]
fn test_json_refuse() {
    let refused = [
        r#"{ name: "John" }"#,
        r#"{ "name": "John" } "#,
        r#"{ "name": "John", "age": 30, }"#,
        r#"{ "name": "John", "address": { "street": "123 Main St", "city": "New York" }"#,
        r#"{ "name": "John", "age": 30, "hobbies": ["reading", "traveling",], }"#,
        r#"{ "name": "John", "age": 30.5.7 }"#,
        r#"{ "name": "John, "age": 30, "hobbies": ["reading", "traveling"] }"#,
        r#"{ "name": "John", "age": 30, "hobbies": ["reading", { "type": "outdoor", "list": ["hiking", "swimming",]}] }"#,
        r#"{ "name": "John", "age": 30, "status": "\P\J" }"#,
        r#"{ "name": "John", "age": 30, "hobbies": ["reading", "traveling"], "address": { "street": "123 Main St", "city": "New York", "coordinates": { "latitude": 40.7128, "longitude": -74.0060 }}}, "work": { "company": "Acme", "position": "developer" }}"#,
    ];
    for input in refused {
        assert!(!accepts(JSON_GRAMMAR, input), "should refuse {input:?}");
    }
}
