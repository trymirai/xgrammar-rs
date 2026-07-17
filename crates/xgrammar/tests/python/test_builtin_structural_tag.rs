//! Port of core structural-tag grammar cases (Python builtin formats stay Python-side).

use xgrammar::Grammar;

#[test]
fn structural_tag_const_string_roundtrip() {
    let doc = r#"{
        "type": "structural_tag",
        "format": {"type": "const_string", "value": "hello"}
    }"#;
    let grammar = Grammar::from_structural_tag(doc).unwrap();
    let rendered = grammar.to_string();
    assert!(rendered.contains("hello"));
    assert!(rendered.contains("root"));
}

#[test]
fn structural_tag_tag_with_json_schema_content() {
    let doc = r#"{
        "type": "structural_tag",
        "format": {
            "type": "tag",
            "begin": "<tool>",
            "content": {
                "type": "json_schema",
                "json_schema": {
                    "type": "object",
                    "properties": {"q": {"type": "string"}},
                    "required": ["q"],
                    "additionalProperties": false
                }
            },
            "end": "</tool>"
        }
    }"#;
    let grammar = Grammar::from_structural_tag(doc).unwrap();
    let rendered = grammar.to_string();
    assert!(rendered.contains("<tool>"));
    assert!(rendered.contains("</tool>"));
}

#[test]
fn structural_tag_rejects_unknown_format_type() {
    let doc = r#"{
        "type": "structural_tag",
        "format": {"type": "not_a_real_format"}
    }"#;
    assert!(Grammar::from_structural_tag(doc).is_err());
}
