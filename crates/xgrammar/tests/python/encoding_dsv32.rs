//! Upstream Python fixture `encoding_dsv32.py` is used by alignment tests.
//! Keep a smoke test that the DeepSeek-style structural tag grammar compiles.

use xgrammar::Grammar;

#[test]
fn deepseek_style_function_calls_tag_compiles() {
    let doc = r#"{
        "type": "structural_tag",
        "format": {
            "type": "tag",
            "begin": "<｜DSML｜function_calls>",
            "content": {
                "type": "json_schema",
                "json_schema": {"type": "object"}
            },
            "end": "</｜DSML｜function_calls>"
        }
    }"#;
    let grammar = Grammar::from_structural_tag(doc).unwrap();
    assert!(grammar.to_string().contains("function_calls"));
}
