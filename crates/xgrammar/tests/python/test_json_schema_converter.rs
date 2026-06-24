//! Port of `xgrammar/tests/python/test_json_schema_converter.py`.
//!
//! The range-regex generators are ported here; the full `json_schema_to_ebnf` converter
//! (and the schema-driven tests that depend on it) land with the converter itself.
#![allow(clippy::approx_constant)] // float literals here are test fixtures, not π/e

use xgrammar::converter::{generate_float_range_regex, generate_range_regex};

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
