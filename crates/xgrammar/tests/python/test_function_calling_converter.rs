//! Port of core cases from `xgrammar/tests/python/test_function_calling_converter.py`.

use xgrammar::{
    Grammar, GrammarMatcher, qwen_xml_tool_calling_to_ebnf,
};

fn accepts_qwen(schema: &str, instance: &str) -> bool {
    let ebnf = qwen_xml_tool_calling_to_ebnf(schema).unwrap();
    let grammar = Grammar::from_ebnf(&ebnf, "root").unwrap();
    let mut matcher = GrammarMatcher::from_grammar(&grammar, true);
    matcher.accept_string(instance) && matcher.is_terminated()
}

#[test]
fn qwen_xml_string_schema_accepts_basic_parameters() {
    let schema = r#"{
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "integer"}
        },
        "required": ["name", "age"]
    }"#;

    assert!(accepts_qwen(
        schema,
        "<parameter=name>Bob</parameter><parameter=age>100</parameter>"
    ));
    assert!(accepts_qwen(
        schema,
        "<parameter=name>Bob</parameter>\t\n<parameter=age>\t100\n</parameter>"
    ));
}

#[test]
fn qwen_xml_emits_parameter_tags() {
    let schema = r#"{"type":"object","properties":{"q":{"type":"string"}},"required":["q"]}"#;
    let ebnf = qwen_xml_tool_calling_to_ebnf(schema).unwrap();
    assert!(ebnf.contains("<parameter="));
    assert!(ebnf.contains("</parameter>"));
    assert!(ebnf.contains("root"));
}
