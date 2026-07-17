//! Upstream Python fixture `encoding_dsv4.py` is used by alignment tests.
//! Keep a smoke test that nested DSML invoke tags compile.

use xgrammar::Grammar;

#[test]
fn deepseek_v4_style_invoke_tag_compiles() {
    let doc = r#"{
        "type": "structural_tag",
        "format": {
            "type": "sequence",
            "elements": [
                {
                    "type": "tag",
                    "begin": "<｜DSML｜invoke name=\"tool\">",
                    "content": {"type": "const_string", "value": "ok"},
                    "end": "</｜DSML｜invoke>"
                }
            ]
        }
    }"#;
    let grammar = Grammar::from_structural_tag(doc).unwrap();
    let rendered = grammar.to_string();
    assert!(rendered.contains("invoke") || rendered.contains("ok"));
}
