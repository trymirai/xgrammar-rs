//! Port of `xgrammar/tests/python/test_serialization.py`.
//!
//! Grammar JSON serialization (`"v11"` format), roundtrip, functional equivalence, and the
//! version/format/JSON error family. TokenizerInfo and CompiledGrammar serialization land
//! with those types' serializers.

use serde_json::{Value, json};
use xgrammar::{
    config::get_serialization_version,
    grammar::{DeserializeError, Grammar},
    matcher::GrammarMatcher,
};

fn construct_grammar() -> Grammar {
    Grammar::from_ebnf(
        "rule1 ::= ([^0-9] rule1) | \"\"\nroot_rule ::= rule1 \"a\"\n",
        "root_rule",
    )
    .unwrap()
}

fn json_accepts(
    grammar: &Grammar,
    input: &str,
) -> bool {
    let mut m = GrammarMatcher::from_grammar(grammar, true);
    m.accept_string(input) && m.is_terminated()
}

#[test]
fn test_get_serialization_version() {
    assert_eq!(get_serialization_version(), "v11");
}

#[test]
fn test_serialize_grammar() {
    let grammar = construct_grammar();
    let serialized = grammar.serialize_json();
    let actual: Value = serde_json::from_str(&serialized).unwrap();
    let expected = json!({
        "rules": [["rule1", 4, -1, false], ["root", 8, -1, false]],
        "grammar_expr_data": [0, 5, 8, 12, 14, 18, 21, 24, 28],
        "grammar_expr_indptr": [
            1, 3, 1, 48, 57, 4, 1, 0, 5, 2, 0, 1, 3, 0, 6, 2, 3, 2, 4, 1, 0, 0, 1, 97, 5, 2, 5, 6,
            6, 1, 7,
        ],
        "root_rule_id": 1,
        "complete_fsm": Value::Null,
        "per_rule_fsms": [],
        "allow_empty_rule_ids": [],
        "optimized": false,
        "__VERSION__": "v11",
    });
    assert_eq!(actual, expected);
}

#[test]
fn test_serialize_grammar_exception() {
    let valid = construct_grammar().serialize_json();

    // Wrong version → version error.
    let bad_version = valid.replace("\"v11\"", "\"v1\"");
    assert!(matches!(
        Grammar::deserialize_json(&bad_version),
        Err(DeserializeError::Version { .. })
    ));

    // Missing a required field → format error.
    let mut obj: Value = serde_json::from_str(&valid).unwrap();
    obj.as_object_mut().unwrap().remove("rules");
    assert!(matches!(
        Grammar::deserialize_json(&obj.to_string()),
        Err(DeserializeError::Format(_))
    ));

    // Not valid JSON → invalid-JSON error.
    assert!(matches!(
        Grammar::deserialize_json("not a valid json string"),
        Err(DeserializeError::InvalidJson(_))
    ));
}

#[test]
fn test_serialize_grammar_roundtrip() {
    let original = construct_grammar();
    let serialized = original.serialize_json();
    let recovered = Grammar::deserialize_json(&serialized).unwrap();
    assert_eq!(serialized, recovered.serialize_json());
}

#[test]
fn test_serialize_grammar_functional() {
    let original = construct_grammar();
    let recovered =
        Grammar::deserialize_json(&original.serialize_json()).unwrap();
    assert_eq!(original.to_string(), recovered.to_string());
}

#[test]
fn test_serialize_grammar_utf8() {
    let grammar = Grammar::from_ebnf(
        "root ::= \"こんにちは\" | \"😊\" | \"你好\" | \"hello\" | \"\\n\"",
        "root",
    )
    .unwrap();
    let recovered =
        Grammar::deserialize_json(&grammar.serialize_json()).unwrap();
    assert!(json_accepts(&recovered, "こんにちは"));
    assert!(json_accepts(&recovered, "😊"));
    assert!(json_accepts(&recovered, "你好"));
    assert!(json_accepts(&recovered, "hello"));
    assert!(json_accepts(&recovered, "\n"));
}
