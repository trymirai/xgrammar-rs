//! Port of `xgrammar/tests/python/test_json_schema_converter.py`.
//!
//! The range-regex generators are ported here; the full `json_schema_to_ebnf` converter
//! (and the schema-driven tests that depend on it) land with the converter itself.
#![allow(clippy::approx_constant)] // float literals here are test fixtures, not π/e

use xgrammar::converter::{
    generate_float_range_regex, generate_range_regex, json_schema_to_ebnf,
};

const BASIC_JSON_RULES_EBNF: &str = r#"basic_escape ::= ["\\/bfnrt] | "u" [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9]
basic_string_sub ::= ("\"" | [^\0-\x1f\"\\\r\n] basic_string_sub | "\\" basic_escape basic_string_sub) (= [ \n\t]* [,}\]:])
basic_any ::= basic_number | basic_string | basic_boolean | basic_null | basic_array | basic_object
basic_integer ::= ("0" | "-"? [1-9] [0-9]*)
basic_number ::= "-"? ("0" | [1-9] [0-9]*) ("." [0-9]+)? ([eE] [+-]? [0-9]+)?
basic_string ::= ["] basic_string_sub
basic_boolean ::= "true" | "false"
basic_null ::= "null"
basic_array ::= (("[" [ \n\t]* basic_any ([ \n\t]* "," [ \n\t]* basic_any)* [ \n\t]* "]") | ("[" [ \n\t]* "]"))
basic_object ::= ("{" [ \n\t]* basic_string [ \n\t]* ":" [ \n\t]* basic_any ([ \n\t]* "," [ \n\t]* basic_string [ \n\t]* ":" [ \n\t]* basic_any)* [ \n\t]* "}") | "{" [ \n\t]* "}"
"#;

const BASIC_JSON_RULES_EBNF_NO_SPACE: &str = r#"basic_escape ::= ["\\/bfnrt] | "u" [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9]
basic_string_sub ::= ("\"" | [^\0-\x1f\"\\\r\n] basic_string_sub | "\\" basic_escape basic_string_sub) (= [ \n\t]* [,}\]:])
basic_any ::= basic_number | basic_string | basic_boolean | basic_null | basic_array | basic_object
basic_integer ::= ("0" | "-"? [1-9] [0-9]*)
basic_number ::= "-"? ("0" | [1-9] [0-9]*) ("." [0-9]+)? ([eE] [+-]? [0-9]+)?
basic_string ::= ["] basic_string_sub
basic_boolean ::= "true" | "false"
basic_null ::= "null"
basic_array ::= (("[" "" basic_any (", " basic_any)* "" "]") | ("[" "" "]"))
basic_object ::= ("{" "" basic_string ": " basic_any (", " basic_string ": " basic_any)* "" "}") | "{" "}"
"#;

/// Convert with the Python `check_schema_with_grammar` defaults (any_whitespace, strict).
fn check(
    schema: &str,
    expected: &str,
) {
    let ebnf =
        json_schema_to_ebnf(schema, true, None, None, true, None).unwrap();
    assert_eq!(ebnf, expected);
}

/// Convert with explicit whitespace/strict settings.
fn check_full(
    schema: &str,
    expected: &str,
    any_whitespace: bool,
    strict_mode: bool,
) {
    let ebnf = json_schema_to_ebnf(
        schema,
        any_whitespace,
        None,
        None,
        strict_mode,
        None,
    )
    .unwrap();
    assert_eq!(ebnf, expected);
}

#[test]
fn test_basic() {
    let schema = r#"{"properties": {"integer_field": {"title": "Integer Field", "type": "integer"}, "number_field": {"title": "Number Field", "type": "number"}, "boolean_field": {"title": "Boolean Field", "type": "boolean"}, "any_array_field": {"items": {}, "title": "Any Array Field", "type": "array"}, "array_field": {"items": {"type": "string"}, "title": "Array Field", "type": "array"}, "tuple_field": {"maxItems": 3, "minItems": 3, "prefixItems": [{"type": "string"}, {"type": "integer"}, {"items": {"type": "string"}, "type": "array"}], "title": "Tuple Field", "type": "array"}, "object_field": {"additionalProperties": {"type": "integer"}, "title": "Object Field", "type": "object"}, "nested_object_field": {"additionalProperties": {"additionalProperties": {"type": "integer"}, "type": "object"}, "title": "Nested Object Field", "type": "object"}}, "required": ["integer_field", "number_field", "boolean_field", "any_array_field", "array_field", "tuple_field", "object_field", "nested_object_field"], "title": "MainModel", "type": "object"}"#;
    let expected = format!(
        "{BASIC_JSON_RULES_EBNF_NO_SPACE}{}",
        r#"root_prop_3 ::= (("[" "" basic_any (", " basic_any)* "" "]") | ("[" "" "]"))
root_prop_4 ::= (("[" "" basic_string (", " basic_string)* "" "]") | ("[" "" "]"))
root_prop_5_item_2 ::= (("[" "" basic_string (", " basic_string)* "" "]") | ("[" "" "]"))
root_prop_5 ::= ("[" "" (basic_string ", " basic_integer ", " root_prop_5_item_2) "" "]")
root_prop_6 ::= ("{" "" basic_string ": " basic_integer (", " basic_string ": " basic_integer)* "" "}") | "{" "}"
root_prop_7_addl ::= ("{" "" basic_string ": " basic_integer (", " basic_string ": " basic_integer)* "" "}") | "{" "}"
root_prop_7 ::= ("{" "" basic_string ": " root_prop_7_addl (", " basic_string ": " root_prop_7_addl)* "" "}") | "{" "}"
root_part_6 ::= ", " "\"nested_object_field\"" ": " root_prop_7 ""
root_part_5 ::= ", " "\"object_field\"" ": " root_prop_6 root_part_6
root_part_4 ::= ", " "\"tuple_field\"" ": " root_prop_5 root_part_5
root_part_3 ::= ", " "\"array_field\"" ": " root_prop_4 root_part_4
root_part_2 ::= ", " "\"any_array_field\"" ": " root_prop_3 root_part_3
root_part_1 ::= ", " "\"boolean_field\"" ": " basic_boolean root_part_2
root_part_0 ::= ", " "\"number_field\"" ": " basic_number root_part_1
root ::= "{" "" (("\"integer_field\"" ": " basic_integer root_part_0)) "" "}"
"#
    );
    check_full(schema, &expected, false, true);
}

#[test]
fn test_min_max_length() {
    let schema = r#"{"type": "string", "minLength": 1, "maxLength": 10}"#;
    let expected = format!(
        "{BASIC_JSON_RULES_EBNF}root ::= \"\\\"\" [^\"\\\\\\r\\n]{{1,10}} \"\\\"\"\n"
    );
    check(schema, &expected);
}

#[test]
fn test_type_array() {
    let schema = r#"{"type": ["integer", "string"], "minLength": 1, "maxLength": 10, "minimum": 1, "maximum": 10}"#;
    let expected = format!(
        "{BASIC_JSON_RULES_EBNF}root_type_0 ::= ( ( [1-9] | \"1\" \"0\" ) )\n\
         root_type_1 ::= \"\\\"\" [^\"\\\\\\r\\n]{{1,10}} \"\\\"\"\n\
         root ::= root_type_0 | root_type_1\n"
    );
    check(schema, &expected);
}

#[test]
fn test_type_array_empty() {
    let expected = format!("{BASIC_JSON_RULES_EBNF}root ::= basic_any\n");
    check(r#"{"type": []}"#, &expected);
}

#[test]
fn test_empty_array() {
    let schema = r#"{"items": {"type": "string"}, "type": "array"}"#;
    let expected = format!(
        "{BASIC_JSON_RULES_EBNF}{}",
        "root ::= ((\"[\" [ \\n\\t]* basic_string ([ \\n\\t]* \",\" [ \\n\\t]* basic_string)* \
         [ \\n\\t]* \"]\") | (\"[\" [ \\n\\t]* \"]\"))\n"
    );
    check(schema, &expected);
}

#[test]
fn test_empty_object() {
    let schema =
        r#"{"properties": {"name": {"type": "string"}}, "type": "object"}"#;
    let expected = format!(
        "{BASIC_JSON_RULES_EBNF}{}",
        "root ::= (\"{\" [ \\n\\t]* ((\"\\\"name\\\"\" [ \\n\\t]* \":\" [ \\n\\t]* basic_string \
         \"\")) [ \\n\\t]* \"}\") | \"{\" [ \\n\\t]* \"}\"\n"
    );
    check(schema, &expected);
}

#[test]
fn test_primitive_type_string() {
    let expected = format!("{BASIC_JSON_RULES_EBNF}root ::= basic_string\n");
    check(r#"{"type": "string"}"#, &expected);
}

#[test]
fn test_primitive_type_object() {
    let expected = format!("{BASIC_JSON_RULES_EBNF}root ::= basic_object\n");
    check(r#"{"type": "object"}"#, &expected);
}

#[test]
fn test_email_format() {
    let schema = r#"{"type": "string", "format": "email"}"#;
    let root = r##"root ::= "\"" ( ( [a-zA-Z0-9_!#$%&'*+/=?^`{|}~-]+ ( "." [a-zA-Z0-9_!#$%&'*+/=?^`{|}~-]+ )* ) | "\\" "\"" ( "\\" [ -~] | [ !#-[\]-~] )* "\\" "\"" ) "@" ( [A-Za-z0-9] ( [\-A-Za-z0-9]* [A-Za-z0-9] )? ) ( ( "." [A-Za-z0-9] [\-A-Za-z0-9]* [A-Za-z0-9] )* ) "\""
"##;
    let expected = format!("{BASIC_JSON_RULES_EBNF}{root}");
    check(schema, &expected);
}

#[test]
fn test_generate_range_regex() {
    // Basic range tests
    assert_eq!(generate_range_regex(Some(12), Some(16)), r"^((1[2-6]))$");
    assert_eq!(generate_range_regex(Some(1), Some(10)), r"^(([1-9]|10))$");
    assert_eq!(
        generate_range_regex(Some(2134), Some(3459)),
        r"^((2[2-9]\d{2}|2[2-9]\d{2}|21[4-9]\d{1}|213[5-9]|2134|3[0-3]\d{2}|3[0-3]\d{2}|34[0-4]\d{1}|345[0-8]|3459))$"
    );

    // Negative to positive range
    assert_eq!(
        generate_range_regex(Some(-5), Some(10)),
        r"^(-([1-5])|0|([1-9]|10))$"
    );

    // Pure negative range
    assert_eq!(generate_range_regex(Some(-15), Some(-10)), r"^(-(1[0-5]))$");

    // Large ranges
    assert_eq!(
        generate_range_regex(Some(-1999), Some(-100)),
        r"^(-([1-9]\d{2}|1[0-8]\d{2}|19[0-8]\d{1}|199[0-8]|1999))$"
    );
    assert_eq!(
        generate_range_regex(Some(1), Some(9999)),
        r"^(([1-9]|[1-9]\d{1}|[1-9]\d{2}|[1-9]\d{3}))$"
    );
}

#[test]
fn test_generate_float_regex() {
    assert_eq!(
        generate_float_range_regex(Some(1.0), Some(5.0)),
        r"^(1|5|(([2-4]))(\.\d{1,6})?|1\.\d{1,6}|5\.\d{1,6})$"
    );
    assert_eq!(
        generate_float_range_regex(Some(1.5), Some(5.75)),
        r"^(1\.5|5\.75|(([2-4]))(\.\d{1,6})?|1\.6\d{0,5}|1\.7\d{0,5}|1\.8\d{0,5}|1\.9\d{0,5}|5\.0\d{0,5}|5\.1\d{0,5}|5\.2\d{0,5}|5\.3\d{0,5}|5\.4\d{0,5}|5\.5\d{0,5}|5\.6\d{0,5}|5\.70\d{0,4}|5\.71\d{0,4}|5\.72\d{0,4}|5\.73\d{0,4}|5\.74\d{0,4})$"
    );
    assert_eq!(
        generate_float_range_regex(Some(-3.14), Some(2.71828)),
        r"^(-3\.14|2\.71828|(-([1-3])|0|(1))(\.\d{1,6})?|-3\.0\d{0,5}|-3\.10\d{0,4}|-3\.11\d{0,4}|-3\.12\d{0,4}|-3\.13\d{0,4}|2\.0\d{0,5}|2\.1\d{0,5}|2\.2\d{0,5}|2\.3\d{0,5}|2\.4\d{0,5}|2\.5\d{0,5}|2\.6\d{0,5}|2\.70\d{0,4}|2\.710\d{0,3}|2\.711\d{0,3}|2\.712\d{0,3}|2\.713\d{0,3}|2\.714\d{0,3}|2\.715\d{0,3}|2\.716\d{0,3}|2\.717\d{0,3}|2\.7180\d{0,2}|2\.7181\d{0,2}|2\.71820\d{0,1}|2\.71821\d{0,1}|2\.71822\d{0,1}|2\.71823\d{0,1}|2\.71824\d{0,1}|2\.71825\d{0,1}|2\.71826\d{0,1}|2\.71827\d{0,1})$"
    );
    assert_eq!(
        generate_float_range_regex(Some(0.5), None),
        r"^(0\.5|0\.6\d{0,5}|0\.7\d{0,5}|0\.8\d{0,5}|0\.9\d{0,5}|([1-9]|[1-9]\d*)(\.\d{1,6})?)$"
    );
    assert_eq!(
        generate_float_range_regex(None, Some(-1.5)),
        r"^(-1\.5|-1\.6\d{0,5}|-1\.7\d{0,5}|-1\.8\d{0,5}|-1\.9\d{0,5}|(-[3-9]|-[1-9]\d*)(\.\d{1,6})?)$"
    );
    assert_eq!(generate_float_range_regex(None, None), r"^-?\d+(\.\d{1,6})?$");
    assert_eq!(
        generate_float_range_regex(Some(3.14159), Some(3.14159)),
        r"^(3\.14159)$"
    );
    assert_eq!(generate_float_range_regex(Some(10.5), Some(2.5)), r"^()$");
    assert_eq!(
        generate_float_range_regex(Some(5.123456), Some(5.123457)),
        r"^(5\.123456|5\.123457)$"
    );
    assert_eq!(
        generate_float_range_regex(Some(-0.000001), Some(0.000001)),
        r"^(-0\.000001|0\.000001|-0\.000000\d{0,0}|0\.000000\d{0,0})$"
    );
}
