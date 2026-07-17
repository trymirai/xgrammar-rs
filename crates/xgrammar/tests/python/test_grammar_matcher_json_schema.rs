//! Port of non-HF cases from `xgrammar/tests/python/test_grammar_matcher_json_schema.py`.

use xgrammar::{Grammar, GrammarMatcher};

fn accepts(schema: &str, instance: &str) -> bool {
    let grammar =
        Grammar::from_json_schema(schema, true, None, None, true, None)
            .unwrap();
    let mut matcher = GrammarMatcher::from_grammar(&grammar, true);
    matcher.accept_string(instance) && matcher.is_terminated()
}

#[test]
fn simple_object_schema() {
    let schema = r#"{
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "integer"}
        },
        "required": ["name", "age"],
        "additionalProperties": false
    }"#;
    assert!(accepts(schema, r#"{"name":"Ada","age":36}"#));
    assert!(!accepts(schema, r#"{"name":"Ada"}"#));
    assert!(!accepts(schema, r#"{"name":"Ada","age":"36"}"#));
}

#[test]
fn integer_enum_schema() {
    let schema = r#"{"type":"integer","enum":[1,2,3]}"#;
    assert!(accepts(schema, "2"));
    assert!(!accepts(schema, "4"));
}

#[test]
fn array_of_strings() {
    let schema = r#"{"type":"array","items":{"type":"string"},"minItems":1}"#;
    assert!(accepts(schema, r#"["a"]"#));
    assert!(accepts(schema, r#"["a","b"]"#));
    assert!(!accepts(schema, "[]"));
}
