//! Port of `xgrammar/tests/python/test_structural_tag_converter.py`.
//!
//! Covers the pure grammar-string oracle (`Grammar::from_structural_tag` + `Display`) and
//! the error-oracle cases. The `_is_grammar_accept_string` acceptance halves and the
//! tokenizer/HF cases are added once the matcher (M6) and tokenizer (M7) land.

use xgrammar::grammar::Grammar;

/// Wrap a `format` object into a structural tag, convert it, and render the grammar.
fn stag(format_json: &str) -> String {
    let doc =
        format!("{{\"type\": \"structural_tag\", \"format\": {format_json}}}");
    Grammar::from_structural_tag(&doc).unwrap().to_string()
}

/// The error message from converting a structural tag with the given `format`.
fn stag_err(format_json: &str) -> String {
    let doc =
        format!("{{\"type\": \"structural_tag\", \"format\": {format_json}}}");
    Grammar::from_structural_tag(&doc).unwrap_err().to_string()
}

#[test]
fn test_const_string_format() {
    assert_eq!(
        stag(r#"{"type": "const_string", "value": "Hello!"}"#),
        "const_string ::= ((\"Hello!\"))\nroot ::= ((const_string))\n"
    );
}

#[test]
fn test_regex_format() {
    assert_eq!(
        stag(r#"{"type": "regex", "pattern": "Hello![0-9]+"}"#),
        "root_0 ::= ((\"H\" \"e\" \"l\" \"l\" \"o\" \"!\" root_1))\n\
         root_1 ::= (([0-9] root_1) | ([0-9]))\n\
         root ::= ((root_0))\n"
    );
}

#[test]
fn test_ebnf_grammar_format() {
    let format = r#"{"type": "grammar", "grammar": "root ::= \"Hello!\" number\n            number ::= [0-9] | [0-9] number"}"#;
    assert_eq!(
        stag(format),
        "root_0 ::= ((\"Hello!\" number))\n\
         number ::= (([0-9]) | ([0-9] number))\n\
         root ::= ((root_0))\n"
    );
}

#[test]
fn test_token_format_basic() {
    assert_eq!(
        stag(r#"{"type": "token", "token": 42}"#),
        "token ::= ((Token(42)))\nroot ::= ((token))\n"
    );
}

#[test]
fn test_token_format_in_tag_begin_end() {
    let format = r#"{"type": "tag", "begin": {"type": "token", "token": 10}, "content": {"type": "const_string", "value": "X"}, "end": {"type": "token", "token": 20}}"#;
    assert_eq!(
        stag(format),
        "const_string ::= ((\"X\"))\n\
         tag ::= ((Token(10) const_string Token(20)))\n\
         root ::= ((tag))\n"
    );
}

#[test]
fn test_token_format_in_tag_begin_string_end() {
    let format = r#"{"type": "tag", "begin": {"type": "token", "token": 10}, "content": {"type": "const_string", "value": "Y"}, "end": "</end>"}"#;
    assert_eq!(
        stag(format),
        "const_string ::= ((\"Y\"))\n\
         tag ::= ((Token(10) const_string \"</end>\"))\n\
         root ::= ((tag))\n"
    );
}

#[test]
fn test_exclude_token_format_no_excludes() {
    assert_eq!(
        stag(r#"{"type": "exclude_token"}"#),
        "exclude_token ::= ((ExcludeToken()))\nroot ::= ((exclude_token))\n"
    );
}

#[test]
fn test_exclude_token_format_with_excludes() {
    assert_eq!(
        stag(r#"{"type": "exclude_token", "exclude_tokens": [5, 10]}"#),
        "exclude_token ::= ((ExcludeToken(5, 10)))\nroot ::= ((exclude_token))\n"
    );
}

#[test]
fn test_exclude_token_detects_end_from_parent_tag() {
    let format = r#"{"type": "tag", "begin": {"type": "token", "token": 1}, "content": {"type": "exclude_token", "exclude_tokens": [5]}, "end": {"type": "token", "token": 99}}"#;
    assert_eq!(
        stag(format),
        "exclude_token ::= ((ExcludeToken(5, 99)))\n\
         tag ::= ((Token(1) exclude_token Token(99)))\n\
         root ::= ((tag))\n"
    );
}

#[test]
fn test_token_tag_dispatch_format_simple() {
    let format = r#"{"type": "token_dispatch", "rules": [[10, {"type": "const_string", "value": "A"}], [20, {"type": "const_string", "value": "B"}]], "loop": false}"#;
    assert_eq!(
        stag(format),
        "const_string ::= ((\"A\"))\n\
         const_string_1 ::= ((\"B\"))\n\
         token_tag_dispatch ::= ((token_tag_dispatch_1))\n\
         root ::= ((token_tag_dispatch))\n\
         token_tag_dispatch_1 ::= TokenTagDispatch(\n  (10, const_string),\n  (20, const_string_1),\n  loop_after_dispatch=false,\n  excludes=()\n)\n"
    );
}

#[test]
fn test_token_tag_dispatch_format_with_excludes() {
    let format = r#"{"type": "token_dispatch", "rules": [[10, {"type": "const_string", "value": "C"}]], "loop": false, "exclude_tokens": [50]}"#;
    assert_eq!(
        stag(format),
        "const_string ::= ((\"C\"))\n\
         token_tag_dispatch ::= ((token_tag_dispatch_1))\n\
         root ::= ((token_tag_dispatch))\n\
         token_tag_dispatch_1 ::= TokenTagDispatch(\n  (10, const_string),\n  loop_after_dispatch=false,\n  excludes=(50)\n)\n"
    );
}

#[test]
fn test_tag_dispatch_format_with_excludes() {
    let format = r#"{"type": "dispatch", "rules": [["tag1", {"type": "const_string", "value": "abcd"}], ["tag2", {"type": "const_string", "value": "efg"}]], "loop": true, "excludes": ["tag3", "ll"]}"#;
    assert_eq!(
        stag(format),
        "const_string ::= ((\"abcd\"))\n\
         const_string_1 ::= ((\"efg\"))\n\
         tag_dispatch ::= TagDispatch(\n  (\"tag1\", const_string),\n  (\"tag2\", const_string_1),\n  loop_after_dispatch=true,\n  excludes=(\"tag3\", \"ll\")\n)\n\
         root ::= ((tag_dispatch))\n"
    );
}

#[test]
fn test_structural_tag_error() {
    // A format with an unrecognized type must surface a structural-tag error.
    assert!(
        stag_err(r#"{"type": "unknown_format"}"#)
            .contains("Invalid structural tag error")
    );
}
