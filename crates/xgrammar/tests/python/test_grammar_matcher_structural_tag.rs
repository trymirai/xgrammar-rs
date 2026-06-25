//! Port of `xgrammar/tests/python/test_grammar_matcher_structural_tag.py`.
//!
//! Covers the CPU-only structural-tag grammar tests: the deprecated
//! `from_structural_tag(tags, triggers)` legacy form (built here exactly as the Python wrapper
//! `StructuralTag.from_legacy_structural_tag` does — a `triggered_tags` format whose tag
//! contents are JSON-schema grammars), the compiled-grammar form, and a bare `TagDispatch`.
//! The `hf_token_required` mask-generation tests are skipped (they need a HuggingFace
//! tokenizer).

use serde_json::{Value, json};
use xgrammar::{
    compiler::GrammarCompiler,
    grammar::Grammar,
    matcher::GrammarMatcher,
    tokenizer::{TokenizerInfo, VocabType},
};

/// Builds the structural-tag JSON for the legacy `(tags, triggers)` form, mirroring
/// `StructuralTag.from_legacy_structural_tag(...).model_dump_json()`.
fn legacy_structural_tag_json(
    tags: &[(&str, Value, &str)],
    triggers: &[&str],
) -> String {
    let tag_values: Vec<Value> = tags
        .iter()
        .map(|(begin, schema, end)| {
            json!({
                "type": "tag",
                "begin": begin,
                "content": {
                    "type": "json_schema",
                    "json_schema": schema,
                    "style": "json",
                },
                "end": end,
            })
        })
        .collect();
    json!({
        "type": "structural_tag",
        "format": {
            "type": "triggered_tags",
            "triggers": triggers,
            "tags": tag_values,
            "at_least_one": false,
            "stop_after_first": false,
            "excludes": [],
        },
    })
    .to_string()
}

/// Pydantic's `model_json_schema()` for `class Schema(BaseModel): arg1: str; arg2: int`.
fn schema_str_int() -> Value {
    json!({
        "properties": {
            "arg1": {"title": "Arg1", "type": "string"},
            "arg2": {"title": "Arg2", "type": "integer"},
        },
        "required": ["arg1", "arg2"],
        "title": "Schema1",
        "type": "object",
    })
}

/// Pydantic's `model_json_schema()` for `class Schema(BaseModel): arg3: float; arg4: List[str]`.
fn schema_float_list() -> Value {
    json!({
        "properties": {
            "arg3": {"title": "Arg3", "type": "number"},
            "arg4": {"items": {"type": "string"}, "title": "Arg4", "type": "array"},
        },
        "required": ["arg3", "arg4"],
        "title": "Schema2",
        "type": "object",
    })
}

/// `_is_grammar_accept_string`: accept the whole string, then require termination
/// (the matcher built with `terminate_without_stop_token=true`, empty tokenizer).
fn is_grammar_accept_string(
    grammar: &Grammar,
    input: &str,
) -> bool {
    let mut m = GrammarMatcher::from_grammar(grammar, true);
    m.accept_string(input) && m.is_terminated()
}

#[test]
fn test_utf8() {
    let schema = schema_str_int();
    let tags = [
        ("，，", schema.clone(), "。"),
        ("，！", schema.clone(), "。。"),
        ("，，？", schema.clone(), "。。。"),
        ("｜｜？", schema, "｜？｜"),
    ];
    let triggers = ["，", "｜｜"];
    let json = legacy_structural_tag_json(&tags, &triggers);
    let grammar = Grammar::from_structural_tag(&json).unwrap();

    let accepted_inputs = [
        "这是无用的内容，，{\"arg1\": \"你好，世界！\", \"arg2\": 0}。这是无用的内容",
        "这是无用的内容，！{\"arg1\": \"こんにちは！\", \"arg2\": 1}。。这是无用的内容",
        "这是无用的内容，，？{\"arg1\": \"안녕하세요！\", \"arg2\": 2}。。。这是无用的内容，！{\"arg1\": \"안녕하세요！\", \"arg2\": 3}。。",
        "这是无用的内容｜｜？{\"arg1\": \"။စ်န, ်ပြ！\", \"arg2\": 0}｜？｜｜｜？{\"arg1\": \"။စ်န, ်ပြ\", \"arg2\": 0}｜？｜",
    ];
    for input in accepted_inputs {
        assert!(is_grammar_accept_string(&grammar, input));
    }
}

const EXPECTED_BEFORE_OPTIMIZATION: &str = r#"basic_escape ::= (([\"\\/bfnrt]) | ("u" [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9]))
basic_string_sub ::= (("\"") | ([^\0-\x1f\"\\\r\n] basic_string_sub) | ("\\" basic_escape basic_string_sub)) (=([ \n\t]* [,}\]:]))
basic_any ::= ((basic_number) | (basic_string) | (basic_boolean) | (basic_null) | (basic_array) | (basic_object))
basic_integer ::= (("0") | (basic_integer_1 [1-9] [0-9]*))
basic_number ::= ((basic_number_1 basic_number_7 basic_number_3 basic_number_6))
basic_string ::= (("\"" basic_string_sub))
basic_boolean ::= (("true") | ("false"))
basic_null ::= (("null"))
basic_array ::= (("[" [ \n\t]* basic_any basic_array_1 [ \n\t]* "]") | ("[" [ \n\t]* "]"))
basic_object ::= (("{" [ \n\t]* basic_string [ \n\t]* ":" [ \n\t]* basic_any basic_object_1 [ \n\t]* "}") | ("{" [ \n\t]* "}"))
root_part_0 ::= (([ \n\t]* "," [ \n\t]* "\"arg2\"" [ \n\t]* ":" [ \n\t]* basic_integer))
root_0 ::= (("{" [ \n\t]* "\"arg1\"" [ \n\t]* ":" [ \n\t]* basic_string root_part_0 [ \n\t]* "}"))
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
basic_escape_1 ::= (([\"\\/bfnrt]) | ("u" [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9]))
basic_string_sub_1 ::= (("\"") | ([^\0-\x1f\"\\\r\n] basic_string_sub_1) | ("\\" basic_escape_1 basic_string_sub_1)) (=([ \n\t]* [,}\]:]))
basic_any_1 ::= ((basic_number_8) | (basic_string_1) | (basic_boolean_1) | (basic_null_1) | (basic_array_2) | (basic_object_2))
basic_integer_2 ::= (("0") | (basic_integer_1_1 [1-9] [0-9]*))
basic_number_8 ::= ((basic_number_1_1 basic_number_7_1 basic_number_3_1 basic_number_6_1))
basic_string_1 ::= (("\"" basic_string_sub_1))
basic_boolean_1 ::= (("true") | ("false"))
basic_null_1 ::= (("null"))
basic_array_2 ::= (("[" [ \n\t]* basic_any_1 basic_array_1_1 [ \n\t]* "]") | ("[" [ \n\t]* "]"))
basic_object_2 ::= (("{" [ \n\t]* basic_string_1 [ \n\t]* ":" [ \n\t]* basic_any_1 basic_object_1_1 [ \n\t]* "}") | ("{" [ \n\t]* "}"))
root_prop_1 ::= (("[" [ \n\t]* basic_string_1 root_prop_1_1 [ \n\t]* "]") | ("[" [ \n\t]* "]"))
root_part_0_1 ::= (([ \n\t]* "," [ \n\t]* "\"arg4\"" [ \n\t]* ":" [ \n\t]* root_prop_1))
root_1 ::= (("{" [ \n\t]* "\"arg3\"" [ \n\t]* ":" [ \n\t]* basic_number_8 root_part_0_1 [ \n\t]* "}"))
basic_integer_1_1 ::= ("" | ("-"))
basic_number_1_1 ::= ("" | ("-"))
basic_number_2_1 ::= (([0-9] basic_number_2_1) | ([0-9]))
basic_number_3_1 ::= ("" | ("." basic_number_2_1))
basic_number_4_1 ::= ("" | ([+\-]))
basic_number_5_1 ::= (([0-9] basic_number_5_1) | ([0-9]))
basic_number_6_1 ::= ("" | ([eE] basic_number_4_1 basic_number_5_1))
basic_array_1_1 ::= ("" | ([ \n\t]* "," [ \n\t]* basic_any_1 basic_array_1_1))
basic_object_1_1 ::= ("" | ([ \n\t]* "," [ \n\t]* basic_string_1 [ \n\t]* ":" [ \n\t]* basic_any_1 basic_object_1_1))
root_prop_1_1 ::= ("" | ([ \n\t]* "," [ \n\t]* basic_string_1 root_prop_1_1))
basic_number_7_1 ::= (("0") | ([1-9] [0-9]*))
triggered_tags_group ::= (("1>" root_0 "</function>") | ("2>" root_0 "</function>"))
triggered_tags_group_1 ::= ((">" root_1 "</function>"))
triggered_tags ::= TagDispatch(
  ("<function=f", triggered_tags_group),
  ("<function=g", triggered_tags_group_1),
  loop_after_dispatch=true,
  excludes=()
)
root ::= ((triggered_tags))
"#;

const EXPECTED_AFTER_OPTIMIZATION: &str = r#"basic_escape ::= (([\"\\/bfnrt]) | ("u" [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9])) (=(basic_string_sub))
basic_string_sub ::= (("\"") | ([^\0-\x1f\"\\\r\n] basic_string_sub) | ("\\" basic_escape basic_string_sub)) (=([ \n\t]* [,}\]:]))
basic_integer ::= (("0") | (basic_integer_1 [1-9] [0-9]*))
basic_string ::= (("\"" basic_string_sub)) (=(root_part_0 [ \n\t]* "}"))
root_part_0 ::= (([ \n\t]* "," [ \n\t]* "\"arg2\"" [ \n\t]* ":" [ \n\t]* basic_integer)) (=([ \n\t]* "}"))
root_0 ::= (("{" [ \n\t]* "\"arg1\"" [ \n\t]* ":" [ \n\t]* basic_string root_part_0 [ \n\t]* "}"))
basic_integer_1 ::= ("" | ("-")) (=([1-9] [0-9]*))
basic_escape_1 ::= (([\"\\/bfnrt]) | ("u" [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9])) (=(basic_string_sub_1))
basic_string_sub_1 ::= (("\"") | ([^\0-\x1f\"\\\r\n] basic_string_sub_1) | ("\\" basic_escape_1 basic_string_sub_1)) (=([ \n\t]* [,}\]:]))
basic_number_8 ::= ((basic_number_1_1 basic_number_7_1 basic_number_3_1 basic_number_6_1)) (=(root_part_0_1 [ \n\t]* "}"))
basic_string_1 ::= (("\"" basic_string_sub_1))
root_prop_1 ::= (("[" [ \n\t]* basic_string_1 root_prop_1_1 [ \n\t]* "]") | ("[" [ \n\t]* "]"))
root_part_0_1 ::= (([ \n\t]* "," [ \n\t]* "\"arg4\"" [ \n\t]* ":" [ \n\t]* root_prop_1)) (=([ \n\t]* "}"))
root_1 ::= (("{" [ \n\t]* "\"arg3\"" [ \n\t]* ":" [ \n\t]* basic_number_8 root_part_0_1 [ \n\t]* "}")) (=("</function>"))
basic_number_1_1 ::= ("" | ("-")) (=(basic_number_7_1 basic_number_3_1 basic_number_6_1))
basic_number_2_1 ::= (([0-9] basic_number_2_1) | ([0-9]))
basic_number_3_1 ::= ("" | ("." basic_number_2_1)) (=(basic_number_6_1))
basic_number_4_1 ::= ("" | ([+\-])) (=(basic_number_5_1))
basic_number_5_1 ::= (([0-9] basic_number_5_1) | ([0-9]))
basic_number_6_1 ::= ("" | ([eE] basic_number_4_1 basic_number_5_1))
root_prop_1_1 ::= ("" | ([ \n\t]* "," [ \n\t]* basic_string_1 root_prop_1_1)) (=([ \n\t]* "]"))
basic_number_7_1 ::= (("0") | ([1-9] [0-9]*)) (=(basic_number_3_1 basic_number_6_1))
triggered_tags_group ::= (("1>" root_0 "</function>") | ("2>" root_0 "</function>"))
triggered_tags_group_1 ::= ((">" root_1 "</function>"))
triggered_tags ::= TagDispatch(
  ("<function=f", triggered_tags_group),
  ("<function=g", triggered_tags_group_1),
  loop_after_dispatch=true,
  excludes=()
)
root ::= ((triggered_tags))
"#;

fn function_call_tags() -> [(&'static str, Value, &'static str); 3] {
    [
        ("<function=f1>", schema_str_int(), "</function>"),
        ("<function=f2>", schema_str_int(), "</function>"),
        ("<function=g>", schema_float_list(), "</function>"),
    ]
}

#[test]
fn test_structural_tag() {
    let tags = function_call_tags();
    // Two triggers dispatching to the f-tags and the g-tag respectively.
    let triggers = ["<function=f", "<function=g"];
    let json = legacy_structural_tag_json(&tags, &triggers);
    let grammar = Grammar::from_structural_tag(&json).unwrap();
    assert_eq!(grammar.to_string(), EXPECTED_BEFORE_OPTIMIZATION);

    let accepted_inputs = [
        "<function=f1>{\"arg1\": \"abc\", \"arg2\": 1}</function>",
        "<function=g>{\"arg3\": 1.23, \"arg4\": [\"a\", \"b\", \"c\"]}</function>",
        "<function=f2>{\"arg1\": \"abc\", \"arg2\": 1}</function><function=g>{\"arg3\": 1.23, \"arg4\": [\"a\", \"b\", \"c\"]}</function>",
        "hhhh<function=g>{\"arg3\": 1.23, \"arg4\": [\"a\", \"b\", \"c\"]}</function>haha<function=f1>{\"arg1\": \"abc\", \"arg2\": 1}</function>123",
    ];
    for input in accepted_inputs {
        assert!(is_grammar_accept_string(&grammar, input));
    }
}

#[test]
fn test_structural_tag_compiler() {
    let tags = function_call_tags();
    let triggers = ["<function=f", "<function=g"];
    let json = legacy_structural_tag_json(&tags, &triggers);

    let tokenizer_info =
        TokenizerInfo::new(&[], VocabType::Raw, None, None, false);
    let compiler = GrammarCompiler::with_defaults(tokenizer_info);
    let compiled = compiler.compile_structural_tag(&json).unwrap();
    assert_eq!(compiled.grammar().to_string(), EXPECTED_AFTER_OPTIMIZATION);
}

#[test]
fn test_empty_tag_dispatch() {
    let grammar_str = "root ::= TagDispatch(\n  loop_after_dispatch=true\n)\n";
    let grammar = Grammar::from_ebnf(grammar_str, "root").unwrap();
    assert!(is_grammar_accept_string(&grammar, "any string"));
    assert!(is_grammar_accept_string(&grammar, ""));
    assert!(is_grammar_accept_string(&grammar, "好"));

    let grammar_with_excludes_str = "root ::= TagDispatch(\n  excludes=(\"end\"),\n  loop_after_dispatch=true\n)\n";
    let grammar_with_excludes =
        Grammar::from_ebnf(grammar_with_excludes_str, "root").unwrap();
    assert!(is_grammar_accept_string(&grammar_with_excludes, "any string"));
    assert!(is_grammar_accept_string(&grammar_with_excludes, "好"));
    assert!(!is_grammar_accept_string(&grammar_with_excludes, "any stringend"));
    assert!(!is_grammar_accept_string(&grammar_with_excludes, "endaaa"));
}
