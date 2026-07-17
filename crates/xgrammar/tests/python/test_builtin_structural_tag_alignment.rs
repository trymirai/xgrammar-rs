//! Alignment helpers live in upstream Python (`encoding_dsv32` / `encoding_dsv4`).
//! This module covers the Rust-side structural-tag grammar contract those suites assume.

use xgrammar::{Grammar, GrammarMatcher};

fn accepts_structural(doc: &str, instance: &str) -> bool {
    let grammar = Grammar::from_structural_tag(doc).unwrap();
    let mut matcher = GrammarMatcher::from_grammar(&grammar, true);
    matcher.accept_string(instance) && matcher.is_terminated()
}

#[test]
fn tag_dispatch_accepts_wrapped_json() {
    let doc = r#"{
        "type": "structural_tag",
        "format": {
            "type": "tag",
            "begin": "<call>",
            "content": {
                "type": "json_schema",
                "json_schema": {
                    "type": "object",
                    "properties": {"x": {"type": "integer"}},
                    "required": ["x"],
                    "additionalProperties": false
                }
            },
            "end": "</call>"
        }
    }"#;
    assert!(accepts_structural(doc, r#"<call>{"x":1}</call>"#));
    assert!(!accepts_structural(doc, r#"<call>{"x":"1"}</call>"#));
}
