//! Port of `xgrammar/tests/python/test_json_schema_converter.py`.
//!
//! The range-regex generators are ported here; the full `json_schema_to_ebnf` converter
//! (and the schema-driven tests that depend on it) land with the converter itself.
#![allow(clippy::approx_constant)] // float literals here are test fixtures, not π/e

use xgrammar::{
    converter::{
        generate_float_range_regex, generate_range_regex, json_schema_to_ebnf,
    },
    grammar::Grammar,
    matcher::GrammarMatcher,
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

/// `check_schema_with_instance`: build the schema grammar, then assert the matcher accepts
/// (and terminates on) the instance iff `is_accepted`.
fn check_schema_with_instance(
    schema: &str,
    instance: &str,
    is_accepted: bool,
) {
    let grammar =
        Grammar::from_json_schema(schema, true, None, None, true, None)
            .unwrap();
    let mut m = GrammarMatcher::from_grammar(&grammar, true);
    let accepted = m.accept_string(instance) && m.is_terminated();
    assert_eq!(accepted, is_accepted, "instance {instance:?}");
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
        r"^(1.5|5.75|(([2-4]))(\.\d{1,6})?|1\.6\d{0,5}|1\.7\d{0,5}|1\.8\d{0,5}|1\.9\d{0,5}|5\.0\d{0,5}|5\.1\d{0,5}|5\.2\d{0,5}|5\.3\d{0,5}|5\.4\d{0,5}|5\.5\d{0,5}|5\.6\d{0,5}|5\.70\d{0,4}|5\.71\d{0,4}|5\.72\d{0,4}|5\.73\d{0,4}|5\.74\d{0,4})$"
    );
    assert_eq!(
        generate_float_range_regex(Some(-3.14), Some(2.71828)),
        r"^(-3.14|2.71828|(-([1-3])|0|(1))(\.\d{1,6})?|-3\.0\d{0,5}|-3\.10\d{0,4}|-3\.11\d{0,4}|-3\.12\d{0,4}|-3\.13\d{0,4}|2\.0\d{0,5}|2\.1\d{0,5}|2\.2\d{0,5}|2\.3\d{0,5}|2\.4\d{0,5}|2\.5\d{0,5}|2\.6\d{0,5}|2\.70\d{0,4}|2\.710\d{0,3}|2\.711\d{0,3}|2\.712\d{0,3}|2\.713\d{0,3}|2\.714\d{0,3}|2\.715\d{0,3}|2\.716\d{0,3}|2\.717\d{0,3}|2\.7180\d{0,2}|2\.7181\d{0,2}|2\.71820\d{0,1}|2\.71821\d{0,1}|2\.71822\d{0,1}|2\.71823\d{0,1}|2\.71824\d{0,1}|2\.71825\d{0,1}|2\.71826\d{0,1}|2\.71827\d{0,1})$"
    );
    assert_eq!(
        generate_float_range_regex(Some(0.5), None),
        r"^(0.5|0\.6\d{0,5}|0\.7\d{0,5}|0\.8\d{0,5}|0\.9\d{0,5}|([1-9]|[1-9]\d*)(\.\d{1,6})?)$"
    );
    assert_eq!(
        generate_float_range_regex(None, Some(-1.5)),
        r"^(-1.5|-1\.6\d{0,5}|-1\.7\d{0,5}|-1\.8\d{0,5}|-1\.9\d{0,5}|(-[3-9]|-[1-9]\d*)(\.\d{1,6})?)$"
    );
    assert_eq!(generate_float_range_regex(None, None), r"^-?\d+(\.\d{1,6})?$");
    assert_eq!(
        generate_float_range_regex(Some(3.14159), Some(3.14159)),
        r"^(3.14159)$"
    );
    assert_eq!(generate_float_range_regex(Some(10.5), Some(2.5)), r"^()$");
    assert_eq!(
        generate_float_range_regex(Some(5.123456), Some(5.123457)),
        r"^(5.123456|5.123457)$"
    );
    assert_eq!(
        generate_float_range_regex(Some(-0.000001), Some(0.000001)),
        r"^(-0.000001|0.000001|-0\.000000\d{0,0}|0\.000000\d{0,0})$"
    );
}

#[test]
fn test_email_format() {
    let schema = r#"{"type": "string", "format": "email"}"#;
    let root = r###"root ::= "\"" ( ( [a-zA-Z0-9_!#$%&'*+/=?^`{|}~-]+ ( "." [a-zA-Z0-9_!#$%&'*+/=?^`{|}~-]+ )* ) | "\\" "\"" ( "\\" [ -~] | [ !#-[\]-~] )* "\\" "\"" ) "@" ( [A-Za-z0-9] ( [\-A-Za-z0-9]* [A-Za-z0-9] )? ) ( ( "." [A-Za-z0-9] [\-A-Za-z0-9]* [A-Za-z0-9] )* ) "\""
"###;
    check(schema, &format!("{BASIC_JSON_RULES_EBNF}{root}"));
    let cases: &[(&str, bool)] = &[
        ("simple@example.com", true),
        ("very.common@example.com", true),
        ("FirstName.LastName@EasierReading.org", true),
        ("x@example.com", true),
        ("long.email-address-with-hyphens@and.subdomains.example.com", true),
        ("user.name+tag+sorting@example.com", true),
        ("name/surname@example.com", true),
        ("admin@example", true),
        ("example@s.example", true),
        ("\\\" \\\"@example.org", true),
        ("\\\"john..doe\\\"@example.org", true),
        ("mailhost!username@example.org", true),
        (
            "\\\"very.(),:;<>[]\\\\\\\".VERY.\\\\\\\"very@\\\\\\\\ \\\\\\\"very\\\\\\\".unusual\\\"@strange.example.com",
            true,
        ),
        ("user%example.com@example.org", true),
        ("user-@example.org", true),
        ("abc.example.com", false),
        ("a@b@c@example.com", false),
        ("a\"b(c)d,e:f;g<h>i[j\\k]l@example.com", false),
        ("just\"not\"right@example.com", false),
        ("this is\"not\\allowed@example.com", false),
        ("this\\ still\\\"not\\\\allowed@example.com", false),
        ("i.like.underscores@but_they_are_not_allowed_in_this_part", false),
    ];
    for (instance, accepted) in cases {
        check_schema_with_instance(
            schema,
            &format!("\"{instance}\""),
            *accepted,
        );
    }
}

#[test]
fn test_date_format() {
    let schema = r#"{"type": "string", "format": "date"}"#;
    let root = r###"root ::= "\"" ( [0-9]{4} "-" ( "0" [1-9] | "1" [0-2] ) "-" ( "0" [1-9] | [1-2] [0-9] | "3" [01] ) ) "\""
"###;
    check(schema, &format!("{BASIC_JSON_RULES_EBNF}{root}"));
    let cases: &[(&str, bool)] = &[
        ("0000-01-01", true),
        ("9999-12-31", true),
        ("10-01-01", false),
        ("2025-00-01", false),
        ("2025-13-01", false),
        ("2025-01-00", false),
        ("2025-01-32", false),
    ];
    for (instance, accepted) in cases {
        check_schema_with_instance(
            schema,
            &format!("\"{instance}\""),
            *accepted,
        );
    }
}

#[test]
fn test_time_format() {
    let schema = r#"{"type": "string", "format": "time"}"#;
    let root = r###"root ::= "\"" ( [01] [0-9] | "2" [0-3] ) ":" [0-5] [0-9] ":" ( [0-5] [0-9] | "6" "0" ) ( "." [0-9]+ )? ( "Z" | [+-] ( [01] [0-9] | "2" [0-3] ) ":" [0-5] [0-9] ) "\""
"###;
    check(schema, &format!("{BASIC_JSON_RULES_EBNF}{root}"));
    let cases: &[(&str, bool)] = &[
        ("00:00:00Z", true),
        ("23:59:60Z", true),
        ("12:34:56Z", true),
        ("12:34:56+07:08", true),
        ("12:34:56-07:08", true),
        ("12:34:56.7Z", true),
        ("12:34:56.7+08:09", true),
        ("12:34:56.7-08:09", true),
        ("00:00:00", false),
        ("23:59:60", false),
        ("12:34:56.7", false),
        ("12:34:56.7890", false),
        ("24:00:00", false),
        ("00:60:00", false),
        ("00:00:61", false),
        ("00:00:00.", false),
        ("12:34:56+07:", false),
        ("12:34:56-07:", false),
        ("12:34:56.7+-08:09", false),
    ];
    for (instance, accepted) in cases {
        check_schema_with_instance(
            schema,
            &format!("\"{instance}\""),
            *accepted,
        );
    }
}

#[test]
fn test_date_time_format() {
    let schema = r#"{"type": "string", "format": "date-time"}"#;
    let root = r###"root ::= "\"" ( [0-9]{4} "-" ( "0" [1-9] | "1" [0-2] ) "-" ( "0" [1-9] | [1-2] [0-9] | "3" [01] ) ) "T" ( [01] [0-9] | "2" [0-3] ) ":" [0-5] [0-9] ":" ( [0-5] [0-9] | "6" "0" ) ( "." [0-9]+ )? ( "Z" | [+-] ( [01] [0-9] | "2" [0-3] ) ":" [0-5] [0-9] ) "\""
"###;
    check(schema, &format!("{BASIC_JSON_RULES_EBNF}{root}"));
    let cases: &[(&str, bool)] = &[
        ("2024-05-19T14:23:45Z", true),
        ("2019-11-30T08:15:27+05:30", true),
        ("2030-02-01T22:59:59-07:00", true),
        ("2021-07-04T00:00:00.123456Z", true),
        ("2022-12-31T23:45:12-03:00", true),
        ("2024-12-31T23:45:60.123456Z", true),
        ("2024-12-31T23:60:12.123456+05:30", false),
        ("2024-13-15T14:30:00Z", false),
        ("2023-02-2010:59:59Z", false),
        ("2021-11-05T24:00:00+05:30", false),
        ("2022-08-20T12:61:10-03:00", false),
    ];
    for (instance, accepted) in cases {
        check_schema_with_instance(
            schema,
            &format!("\"{instance}\""),
            *accepted,
        );
    }
}

#[test]
fn test_duration_format() {
    let schema = r#"{"type": "string", "format": "duration"}"#;
    let root = r###"root ::= "\"" "P" ( ( [0-9]+ "D" | [0-9]+ "M" ( [0-9]+ "D" )? | [0-9]+ "Y" ( [0-9]+ "M" ( [0-9]+ "D" )? )? ) ( "T" ( [0-9]+ "S" | [0-9]+ "M" ( [0-9]+ "S" )? | [0-9]+ "H" ( [0-9]+ "M" ( [0-9]+ "S" )? )? ) )? | "T" ( [0-9]+ "S" | [0-9]+ "M" ( [0-9]+ "S" )? | [0-9]+ "H" ( [0-9]+ "M" ( [0-9]+ "S" )? )? ) | [0-9]+ "W" ) "\""
"###;
    check(schema, &format!("{BASIC_JSON_RULES_EBNF}{root}"));
    let cases: &[(&str, bool)] = &[
        ("P0Y", true),
        ("P12M", true),
        ("P345D", true),
        ("P6789W", true),
        ("P01234D", true),
        ("PT9H", true),
        ("PT87M", true),
        ("PT654S", true),
        ("P1Y23M456D", true),
        ("P23M456D", true),
        ("P1Y0M456D", true),
        ("P1Y23M", true),
        ("PT9H87M654S", true),
        ("PT87M654S", true),
        ("PT9H0M654S", true),
        ("PT9H87M", true),
        ("P1Y23M456DT9H87M654S", true),
        ("P", false),
        ("PD", false),
        ("P1", false),
        ("PT", false),
        ("P1Y456D", false),
        ("PT9H654S", false),
    ];
    for (instance, accepted) in cases {
        check_schema_with_instance(
            schema,
            &format!("\"{instance}\""),
            *accepted,
        );
    }
}

#[test]
fn test_ipv6_format() {
    let schema = r#"{"type": "string", "format": "ipv6"}"#;
    let root = r###"root ::= "\"" ( ( [0-9a-fA-F]{1,4} ":" ){7,7} [0-9a-fA-F]{1,4} | ( [0-9a-fA-F]{1,4} ":" ){1,7} ":" | ( [0-9a-fA-F]{1,4} ":" ){1,6} ":" [0-9a-fA-F]{1,4} | ( [0-9a-fA-F]{1,4} ":" ){1,5} ( ":" [0-9a-fA-F]{1,4} ){1,2} | ( [0-9a-fA-F]{1,4} ":" ){1,4} ( ":" [0-9a-fA-F]{1,4} ){1,3} | ( [0-9a-fA-F]{1,4} ":" ){1,3} ( ":" [0-9a-fA-F]{1,4} ){1,4} | ( [0-9a-fA-F]{1,4} ":" ){1,2} ( ":" [0-9a-fA-F]{1,4} ){1,5} | [0-9a-fA-F]{1,4} ":" ( ( ":" [0-9a-fA-F]{1,4} ){1,6} ) | ":" ( ( ":" [0-9a-fA-F]{1,4} ){1,7} | ":" ) | ":" ":" ( "f" "f" "f" "f" ( ":" "0"{1,4} ){0,1} ":" ){0,1} ( ( "2" "5" [0-5] | ( "2" [0-4] | "1"{0,1} [0-9] ){0,1} [0-9] ) "." ){3,3} ( "2" "5" [0-5] | ( "2" [0-4] | "1"{0,1} [0-9] ){0,1} [0-9] ) | ( [0-9a-fA-F]{1,4} ":" ){1,4} ":" ( ( "2" "5" [0-5] | ( "2" [0-4] | "1"{0,1} [0-9] ){0,1} [0-9] ) "." ){3,3} ( "2" "5" [0-5] | ( "2" [0-4] | "1"{0,1} [0-9] ){0,1} [0-9] ) ) "\""
"###;
    check(schema, &format!("{BASIC_JSON_RULES_EBNF}{root}"));
    let cases: &[(&str, bool)] = &[
        ("0123:4567:890a:bced:fABC:DEF0:1234:5678", true),
        ("::6666:6666:6666:6666:6666:6666", true),
        ("::6666:6666:6666:6666:6666", true),
        ("::6666:6666:6666:6666", true),
        ("::6666:6666:6666", true),
        ("::6666:6666", true),
        ("::6666", true),
        ("::", true),
        ("8888:8888:8888:8888:8888:8888::", true),
        ("8888:8888:8888:8888:8888::", true),
        ("8888:8888:8888:8888::", true),
        ("8888:8888:8888::", true),
        ("8888:8888::", true),
        ("8888::", true),
        ("1111::2222", true),
        ("1111:1111::2222", true),
        ("1111::2222:2222", true),
        ("1111:1111:1111::2222", true),
        ("1111:1111::2222:2222", true),
        ("1111::2222:2222:2222", true),
        ("1111:1111:1111:1111::2222", true),
        ("1111:1111:1111::2222:2222", true),
        ("1111:1111::2222:2222:2222", true),
        ("1111::2222:2222:2222:2222", true),
        ("1111:1111:1111:1111:1111::2222", true),
        ("1111:1111:1111:1111::2222:2222", true),
        ("1111:1111:1111::2222:2222:2222", true),
        ("1111:1111::2222:2222:2222:2222", true),
        ("1111::2222:2222:2222:2222:2222", true),
        ("1111:1111:1111:1111:1111:1111::2222", true),
        ("1111:1111:1111:1111:1111::2222:2222", true),
        ("1111:1111:1111:1111::2222:2222:2222", true),
        ("1111:1111:1111::2222:2222:2222:2222", true),
        ("1111:1111::2222:2222:2222:2222:2222", true),
        ("1111::2222:2222:2222:2222:2222:2222", true),
        ("2001:db8:3:4::192.0.2.33", true),
        ("64:ff9b::192.0.2.33", true),
        ("::ffff:0:255.255.255.255", true),
        ("::111.111.222.222", true),
        (":", false),
        (":::", false),
        ("::5555:5555:5555:5555:5555:5555:5555:5555", false),
        ("5555::5555:5555:5555:5555:5555:5555:5555", false),
        ("5555:5555::5555:5555:5555:5555:5555:5555", false),
        ("5555:5555:5555::5555:5555:5555:5555:5555", false),
        ("5555:5555:5555:5555::5555:5555:5555:5555", false),
        ("5555:5555:5555:5555:5555::5555:5555:5555", false),
        ("5555:5555:5555:5555:5555:5555::5555:5555", false),
        ("5555:5555:5555:5555:5555:5555:5555::5555", false),
        ("5555:5555:5555:5555:5555:5555:5555:5555::", false),
    ];
    for (instance, accepted) in cases {
        check_schema_with_instance(
            schema,
            &format!("\"{instance}\""),
            *accepted,
        );
    }
}

#[test]
fn test_ipv4_format() {
    let schema = r#"{"type": "string", "format": "ipv4"}"#;
    let root = r###"root ::= "\"" ( ( "2" "5" [0-5] | "2" [0-4] [0-9] | [0-1]? [0-9]? [0-9] ) "." ){3} ( "2" "5" [0-5] | "2" [0-4] [0-9] | [0-1]? [0-9]? [0-9] ) "\""
"###;
    check(schema, &format!("{BASIC_JSON_RULES_EBNF}{root}"));
    let cases: &[(&str, bool)] = &[
        ("00.00.00.00", true),
        ("000.000.000.000", true),
        ("255.255.255.255", true),
        ("1", false),
        ("1.", false),
        ("1.1", false),
        ("1.1.", false),
        ("1.1.1", false),
        ("1.1.1.", false),
        ("0001.0001.0001.0001", false),
        ("256.256.256.256", false),
    ];
    for (instance, accepted) in cases {
        check_schema_with_instance(
            schema,
            &format!("\"{instance}\""),
            *accepted,
        );
    }
}

#[test]
fn test_hostname_format() {
    let schema = r#"{"type": "string", "format": "hostname"}"#;
    let root = r###"root ::= "\"" ( [a-z0-9] ( [a-z0-9-]* [a-z0-9] )? ) ( "." [a-z0-9] ( [a-z0-9-]* [a-z0-9] )? )* "\""
"###;
    check(schema, &format!("{BASIC_JSON_RULES_EBNF}{root}"));
    let cases: &[(&str, bool)] = &[
        ("0", true),
        ("9", true),
        ("a", true),
        ("z", true),
        ("www.github.com", true),
        ("w-w-w.g-i-t-h-u-b.c-o-m", true),
        ("ww-w.gi-th-ub.co-m", true),
        ("w--ww.git---hub.co----m", true),
        (".", false),
        ("-", false),
        ("-.", false),
        (".-", false),
        ("_", false),
        ("a.", false),
        ("-b", false),
        ("c-", false),
        ("d.-", false),
        ("e-.", false),
        ("-f.", false),
        ("g-.h", false),
        ("-i.j", false),
    ];
    for (instance, accepted) in cases {
        check_schema_with_instance(
            schema,
            &format!("\"{instance}\""),
            *accepted,
        );
    }
}

#[test]
fn test_uuid_format() {
    let schema = r#"{"type": "string", "format": "uuid"}"#;
    let root = r###"root ::= "\"" [0-9A-Fa-f]{8} "-" [0-9A-Fa-f]{4} "-" [0-9A-Fa-f]{4} "-" [0-9A-Fa-f]{4} "-" [0-9A-Fa-f]{12} "\""
"###;
    check(schema, &format!("{BASIC_JSON_RULES_EBNF}{root}"));
    let cases: &[(&str, bool)] = &[
        ("00000000-0000-0000-0000-000000000000", true),
        ("FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF", true),
        ("01234567-89AB-CDEF-abcd-ef0123456789", true),
        ("-", false),
        ("----", false),
        ("AAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA", false),
        ("BBBBBBBB-BBB-BBBB-BBBB-BBBBBBBBBBBB", false),
        ("CCCCCCCC-CCCC-CCC-CCCC-CCCCCCCCCCCC", false),
        ("DDDDDDDD-DDDD-DDDD-DDD-DDDDDDDDDDDD", false),
        ("EEEEEEEE-EEEE-EEEE-EEEE-EEEEEEEEEEE", false),
        ("AAAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA", false),
        ("BBBBBBBB-BBBBB-BBBB-BBBB-BBBBBBBBBBBB", false),
        ("CCCCCCCC-CCCC-CCCCC-CCCC-CCCCCCCCCCCC", false),
        ("DDDDDDDD-DDDD-DDDD-DDDDD-DDDDDDDDDDDD", false),
        ("EEEEEEEE-EEEE-EEEE-EEEE-EEEEEEEEEEEEE", false),
    ];
    for (instance, accepted) in cases {
        check_schema_with_instance(
            schema,
            &format!("\"{instance}\""),
            *accepted,
        );
    }
}

#[test]
fn test_uri_format() {
    let schema = r#"{"type": "string", "format": "uri"}"#;
    let root = r###"root ::= "\"" [a-zA-Z] [a-zA-Z+.-]* ":" ( "/" "/" ( ( [a-zA-Z0-9_.~!$&'()*+,;=:-] | "%" [0-9A-Fa-f] [0-9A-Fa-f] )* "@" )? ( [a-zA-Z0-9_.~!$&'()*+,;=-] | "%" [0-9A-Fa-f] [0-9A-Fa-f] )* ( ":" [0-9]* )? ( "/" ( [a-zA-Z0-9_.~!$&'()*+,;=:@-] | "%" [0-9A-Fa-f] [0-9A-Fa-f] )* )* | "/"? ( ( [a-zA-Z0-9_.~!$&'()*+,;=:@-] | "%" [0-9A-Fa-f] [0-9A-Fa-f] )+ ( "/" ( [a-zA-Z0-9_.~!$&'()*+,;=:@-] | "%" [0-9A-Fa-f] [0-9A-Fa-f] )* )* )? ) ( "\?" ( [a-zA-Z0-9_.~!$&'()*+,;=:@/\?-] | "%" [0-9A-Fa-f] [0-9A-Fa-f] )* )? ( "#" ( [a-zA-Z0-9_.~!$&'()*+,;=:@/\?-] | "%" [0-9A-Fa-f] [0-9A-Fa-f] )* )? "\""
"###;
    check(schema, &format!("{BASIC_JSON_RULES_EBNF}{root}"));
    let cases: &[(&str, bool)] = &[
        ("aaa:?azAZ09-._~%Ff!$&'()*+,;=:@#azAZ09-._~%Aa!$&'()*+,;=:@", true),
        ("z+.-:", true),
        ("abc:", true),
        ("abc:a", true),
        ("abc:/", true),
        ("abc:/a", true),
        ("abc://", true),
        ("abc://///////", true),
        ("abc://azAZ09-._~%Ff!$&'()*+,;=:@", true),
        ("abc://:", true),
        ("abc://:0123", true),
        ("abc://azAZ09-._~%Ff!$&'()*+,;=", true),
        ("xyz:/a", true),
        ("xyz:/azAZ09-._~%Ff!$&'()*+,;=:@", true),
        ("aaa:?[#]", false),
        ("abc://@@", false),
        ("abc://::", false),
        ("abc:/[]", false),
    ];
    for (instance, accepted) in cases {
        check_schema_with_instance(
            schema,
            &format!("\"{instance}\""),
            *accepted,
        );
    }
}

#[test]
fn test_uri_reference_format() {
    let schema = r#"{"type": "string", "format": "uri-reference"}"#;
    let root = r###"root ::= "\"" ( "/" "/" ( ( [a-zA-Z0-9_.~!$&'()*+,;=:-] | "%" [0-9A-Fa-f] [0-9A-Fa-f] )* "@" )? ( [a-zA-Z0-9_.~!$&'()*+,;=-] | "%" [0-9A-Fa-f] [0-9A-Fa-f] )* ( ":" [0-9]* )? ( "/" ( [a-zA-Z0-9_.~!$&'()*+,;=:@-] | "%" [0-9A-Fa-f] [0-9A-Fa-f] )* )* | "/" ( ( [a-zA-Z0-9_.~!$&'()*+,;=:@-] | "%" [0-9A-Fa-f] [0-9A-Fa-f] )+ ( "/" ( [a-zA-Z0-9_.~!$&'()*+,;=:@-] | "%" [0-9A-Fa-f] [0-9A-Fa-f] )* )* )? | ( [a-zA-Z0-9_.~!$&'()*+,;=@-] | "%" [0-9A-Fa-f] [0-9A-Fa-f] )+ ( "/" ( [a-zA-Z0-9_.~!$&'()*+,;=:@-] | "%" [0-9A-Fa-f] [0-9A-Fa-f] )* )* )? ( "\?" ( [a-zA-Z0-9_.~!$&'()*+,;=:@/\?-] | "%" [0-9A-Fa-f] [0-9A-Fa-f] )* )? ( "#" ( [a-zA-Z0-9_.~!$&'()*+,;=:@/\?-] | "%" [0-9A-Fa-f] [0-9A-Fa-f] )* )? "\""
"###;
    check(schema, &format!("{BASIC_JSON_RULES_EBNF}{root}"));
    let cases: &[(&str, bool)] = &[
        ("?azAZ09-._~%Ff!$&'()*+,;=:@#azAZ09-._~%Aa!$&'()*+,;=:@", true),
        ("", true),
        ("a", true),
        ("/", true),
        ("/a", true),
        ("//", true),
        ("/////////", true),
        ("//azAZ09-._~%Ff!$&'()*+,;=:@", true),
        ("//:", true),
        ("//:0123", true),
        ("//azAZ09-._~%Ff!$&'()*+,;=", true),
        ("/a", true),
        ("/azAZ09-._~%Ff!$&'()*+,;=:@", true),
        ("?[#]", false),
        ("//@@", false),
        ("//::", false),
        ("/[]", false),
        (":", false),
    ];
    for (instance, accepted) in cases {
        check_schema_with_instance(
            schema,
            &format!("\"{instance}\""),
            *accepted,
        );
    }
}

#[test]
fn test_uri_template_format() {
    let schema = r#"{"type": "string", "format": "uri-template"}"#;
    let root = r###"root ::= "\"" ( ( [!#-$&(-;=\?-[\]_a-z~] | "%" [0-9A-Fa-f] [0-9A-Fa-f] ) | "{" ( [+#./;\?&=,!@|] )? ( [a-zA-Z0-9_] | "%" [0-9A-Fa-f] [0-9A-Fa-f] ) ( "."? ( [a-zA-Z0-9_] | "%" [0-9A-Fa-f] [0-9A-Fa-f] ) )* ( ":" [1-9] [0-9]? [0-9]? [0-9]? | "*" )? ( "," ( [a-zA-Z0-9_] | "%" [0-9A-Fa-f] [0-9A-Fa-f] ) ( "."? ( [a-zA-Z0-9_] | "%" [0-9A-Fa-f] [0-9A-Fa-f] ) )* ( ":" [1-9] [0-9]? [0-9]? [0-9]? | "*" )? )* "}" )* "\""
"###;
    check(schema, &format!("{BASIC_JSON_RULES_EBNF}{root}"));
    let cases: &[(&str, bool)] = &[
        ("", true),
        ("!#$&()*+,-./09:;=?@AZ[]_az~%Ff", true),
        ("{+a}{#a}{.a}{/a}{;a}{?a}{&a}{=a}{,a}{!a}{@a}{|a}", true),
        ("{%Ff}", true),
        ("{i.j.k}", true),
        ("{a_b_c:1234}", true),
        ("{x_y_z*}", true),
        ("\"", false),
        ("'", false),
        ("%", false),
        ("<", false),
        (">", false),
        ("\\\\\\\\", false),
        ("^", false),
        ("`", false),
        ("{", false),
        ("|", false),
        ("}", false),
        ("{n.}", false),
        ("{m:100001}", false),
        ("%1", false),
        ("%Gg", false),
    ];
    for (instance, accepted) in cases {
        check_schema_with_instance(
            schema,
            &format!("\"{instance}\""),
            *accepted,
        );
    }
}

#[test]
fn test_json_pointer_format() {
    let schema = r#"{"type": "string", "format": "json-pointer"}"#;
    let root = r###"root ::= "\"" ( "/" ( [\0-.] | [0-}] | [\x7f-\U0010ffff] | "~" [01] )* )* "\""
"###;
    check(schema, &format!("{BASIC_JSON_RULES_EBNF}{root}"));
    let cases: &[(&str, bool)] = &[
        ("/", true),
        ("//", true),
        ("/a/bc/def/ghij", true),
        ("/~0/~1/", true),
        ("abc", false),
        ("/~", false),
        ("/~2", false),
    ];
    for (instance, accepted) in cases {
        check_schema_with_instance(
            schema,
            &format!("\"{instance}\""),
            *accepted,
        );
    }
}

#[test]
fn test_relative_json_pointer_format() {
    let schema = r#"{"type": "string", "format": "relative-json-pointer"}"#;
    let root = r###"root ::= "\"" ( "0" | [1-9] [0-9]* ) ( "#" | ( "/" ( [\0-.] | [0-}] | [\x7f-\U0010ffff] | "~" [01] )* )* ) "\""
"###;
    check(schema, &format!("{BASIC_JSON_RULES_EBNF}{root}"));
    let cases: &[(&str, bool)] = &[
        ("0/", true),
        ("123/a/bc/def/ghij", true),
        ("45/~0/~1/", true),
        ("6789#", true),
        ("#", false),
        ("abc", false),
        ("/", false),
        ("9/~2", false),
    ];
    for (instance, accepted) in cases {
        check_schema_with_instance(
            schema,
            &format!("\"{instance}\""),
            *accepted,
        );
    }
}
