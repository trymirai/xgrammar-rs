//! Port of `external/xgrammar/tests/cpp/test_lark_converter.cc`.

use std::sync::Arc;

use xgrammar::{
    CompiledGrammar, Grammar, GrammarCompiler, TokenizerInfo, VocabType,
    converter::{LarkError, NamedGrammar},
};

fn lark(source: &str) -> String {
    Grammar::from_lark(source, None, &[]).unwrap().to_string()
}

fn lark_source(
    name: &str,
    source: &str,
) -> NamedGrammar {
    NamedGrammar::Lark {
        name: name.to_owned(),
        source: source.to_owned(),
    }
}

fn lark_grammar(
    name: &str,
    grammar: Grammar,
) -> NamedGrammar {
    NamedGrammar::Grammar {
        name: name.to_owned(),
        grammar: Box::new(grammar),
    }
}

/// Asserts that the error message contains the `.*`-separated parts of `pattern`, in order.
fn expect_error(
    result: Result<Grammar, LarkError>,
    pattern: &str,
) {
    let message = result.unwrap_err().to_string();
    let mut rest = message.as_str();
    for part in pattern.split(".*") {
        let index = rest.find(part).unwrap_or_else(|| panic!("{pattern:?} does not match {message:?}"));
        rest = &rest[index + part.len()..];
    }
}

fn same_compiled(
    lhs: &CompiledGrammar,
    rhs: &CompiledGrammar,
) -> bool {
    Arc::ptr_eq(&lhs.adaptive_token_mask_cache_arc(), &rhs.adaptive_token_mask_cache_arc())
}

#[test]
fn default_arguments() {
    let source = r#"start: "x""#;
    assert_eq!(lark(source), Grammar::from_lark(source, None, &[]).unwrap().to_string());
}

#[test]
fn core_syntax_and_imports() {
    let printed = lark(
        r#"
    %import common.INT
    %import common (WS_INLINE, CNAME)
    %ignore WS_INLINE
    start: item{2,3}
    item: CNAME ":" INT | "none"
  "#,
    );
    assert!(printed.contains("root"));
    assert!(printed.contains("lark_ignore"));
    assert!(printed.contains("{2, 3}"));
}

#[test]
fn inline_and_nested_grammar() {
    let printed = lark(
        r#"
    start: "payload=" %json {
      "type": "object",
      "properties": {"x": {"type": "integer"}},
      "required": ["x"],
      "additionalProperties": false
    } ";" %lark {
      start: "yes" | "no"
    }
  "#,
    );
    assert!(printed.contains("payload="));
    assert!(printed.contains("basic_integer"));
    assert!(printed.contains("yes"));
}

#[test]
fn named_grammars() {
    let word = Grammar::from_regex("[a-z]+").unwrap();
    let number = Grammar::from_lark("start: /[0-9]+/", None, &[]).unwrap();
    let printed = Grammar::from_lark(
        r#"
        start: "<" @word ":" @number ">" %lark {
          start: "/" @word
        }
      "#,
        None,
        &[lark_grammar("word", word), lark_grammar("number", number)],
    )
    .unwrap()
    .to_string();
    assert!(printed.contains("[a-z]"));
    assert!(printed.contains("[0-9]"));
}

#[test]
fn named_grammar_sources() {
    let printed = Grammar::from_lark(
        "start: @pair",
        None,
        &[lark_source("pair", r#"start: @item ":" @item"#), lark_source("item", "start: /[a-z]+/")],
    )
    .unwrap()
    .to_string();
    assert!(printed.contains("[a-z]"));
    assert!(printed.contains(r#"":""#));

    expect_error(
        Grammar::from_lark(
            "start: @left",
            None,
            &[lark_source("left", "start: @right"), lark_source("right", "start: @left")],
        ),
        "circular named grammar reference: @left -> @right -> @left",
    );
}

#[test]
fn grammar_compiler_caches_lark_sources() {
    let tokenizer_info = TokenizerInfo::new(&[], VocabType::RAW, None, None, false).unwrap();
    let compiler = GrammarCompiler::new(tokenizer_info.clone(), 1, true, -1);

    let first = compiler.compile_lark(r#"start: "a""#, &[]).unwrap();
    let repeated = compiler.compile_lark(r#"start: "a""#, &[]).unwrap();
    assert!(same_compiled(&first, &repeated));

    let mut named_grammars = vec![lark_source("left", r#"start: "a""#), lark_source("right", r#"start: "b""#)];
    let with_named_grammars = compiler.compile_lark("start: @left @right", &named_grammars).unwrap();
    named_grammars.reverse();
    let with_reordered_named_grammars = compiler.compile_lark("start: @left @right", &named_grammars).unwrap();
    assert!(same_compiled(&with_named_grammars, &with_reordered_named_grammars));

    let with_different_named_grammar = compiler
        .compile_lark(
            "start: @left @right",
            &[lark_source("left", r#"start: "a""#), lark_source("right", r#"start: "c""#)],
        )
        .unwrap();
    assert!(!same_compiled(&with_named_grammars, &with_different_named_grammar));

    let item = || lark_grammar("item", Grammar::from_lark(r#"start: "item""#, None, &[]).unwrap());
    let with_grammar_object = compiler.compile_lark("start: @item", &[item()]).unwrap();
    let with_equivalent_grammar_object = compiler.compile_lark("start: @item", &[item()]).unwrap();
    assert!(same_compiled(&with_grammar_object, &with_equivalent_grammar_object));

    let uncached_compiler = GrammarCompiler::new(tokenizer_info, 1, false, -1);
    let uncached_first = uncached_compiler.compile_lark(r#"start: "a""#, &[]).unwrap();
    let uncached_second = uncached_compiler.compile_lark(r#"start: "a""#, &[]).unwrap();
    assert!(!same_compiled(&uncached_first, &uncached_second));
}

#[test]
fn dynamic_tool_call_lowers_to_tag_dispatch() {
    let printed = lark(
        r#"
    start: (foo | bar)* tail
    tail: TEXT

    foo_head[lazy]: TEXT "<function"
    foo: foo_head "=foo>" /[a-z]+/ "</function>"

    bar_head[lazy]: TEXT "<function"
    bar: bar_head "=bar>" /[A-Z]+/ "</function>"

    TEXT: /(\n|.)*/
  "#,
    );
    assert!(printed.contains("TagDispatch"));
    assert!(printed.contains(r#""<function""#));
    assert!(printed.contains("loop_after_dispatch=true"));
}

#[test]
fn numeric_and_named_special_tokens() {
    let vocab = ["a", "<|tool|>", "b"].map(str::to_owned);
    let tokenizer_info = TokenizerInfo::new(&vocab, VocabType::RAW, Some(3), Some(Vec::new()), false).unwrap();
    let printed = Grammar::from_lark("start: <[0,2]> | <|tool|>", Some(&tokenizer_info), &[]).unwrap().to_string();
    assert!(printed.contains("Token(0, 2)"));
    assert!(printed.contains("Token(1)"));
}

#[test]
fn string_and_regex_flags() {
    // The i flag folds ASCII letters; non-ASCII characters in the regex are matched literally.
    let printed = lark(
        r#"
    start: "Case"i /Σ[^k].x/isu
  "#,
    );
    assert!(printed.contains("root"));

    expect_error(
        Grammar::from_lark("start: \"Żółw\"i", None, &[]),
        "case-insensitive string literals currently support ASCII characters only",
    );
}

#[test]
fn dynamic_regex_suffix_and_suffix_attribute() {
    let printed = lark(
        r#"
    start: (foo | bar)* tail
    tail: TEXT

    foo_head[lazy]: /(\n|.)*<foo>/
    foo: foo_head /[a-z]+/ "</foo>"

    bar_head[suffix="<bar>"]: TEXT
    bar: bar_head /[0-9]+/ "</bar>"

    TEXT: /.*/s
  "#,
    );
    assert!(printed.contains("TagDispatch"));
    assert!(printed.contains(r#""<foo>""#));
    assert!(printed.contains(r#""<bar>""#));
}

#[test]
fn errors_contain_source_locations() {
    let from_lark = |source: &str| Grammar::from_lark(source, None, &[]);
    expect_error(from_lark("item: \"a\""), "line 1, column 1.*no start rule");
    expect_error(from_lark("start: missing"), "line 1, column 8.*unknown name");
    expect_error(from_lark("start: FOO\nFOO: BAR\nBAR: FOO"), "circular reference in terminal");
    expect_error(from_lark("start[budget=10]: \"a\""), "attribute 'budget' is not supported");
    expect_error(from_lark("start[capture=\"a b\"]: \"a\""), "capture name must only contain letters, digits");
    expect_error(from_lark("start: A & B\nA: \"a\"\nB: \"b\""), "intersection '&' is not supported");
    expect_error(from_lark("start: /abc/m"), "regular-expression flag 'm' is not supported");
    expect_error(from_lark("start: TOKEN\nTOKEN: %json {}"), "%json cannot be used in terminals");
    expect_error(from_lark("start: TOKEN\nTOKEN: %lark { start: \"a\" }"), "nested %lark cannot be used in terminals");
    expect_error(from_lark("start: <[1-2-3]>"), "invalid numeric special-token range");
    expect_error(from_lark("start: @missing"), "line 1, column 8.*unknown named grammar '@missing'");
    let item = |source: &str| lark_grammar("item", Grammar::from_lark(source, None, &[]).unwrap());
    expect_error(
        Grammar::from_lark("start: @item", None, &[item(r#"start: "a""#), item(r#"start: "b""#)]),
        "Duplicate named grammar 'item'",
    );
}
