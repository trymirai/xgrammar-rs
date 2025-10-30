use serial_test::serial;
use xgrammar::{
    Grammar, GrammarCompiler, GrammarMatcher, TokenizerInfo, VocabType,
};

fn matcher_from_grammar(grammar: &Grammar) -> GrammarMatcher {
    let empty_vocab: Vec<&str> = vec![];
    let stop_ids: Option<Box<[i32]>> = None;
    let tokenizer_info =
        TokenizerInfo::new(&empty_vocab, VocabType::RAW, &stop_ids, false);
    let mut compiler = GrammarCompiler::new(&tokenizer_info, 1, false, -1);
    let compiled = compiler.compile_grammar(grammar);
    GrammarMatcher::new(&compiled, None, true, -1)
}

fn is_grammar_accept_string(
    grammar: &Grammar,
    input: &str,
) -> bool {
    let mut matcher = matcher_from_grammar(grammar);
    let accepted = matcher.accept_string(input, false);
    accepted && matcher.is_terminated()
}

#[test]
#[serial]
fn test_simple() {
    let grammar_str = r#"root ::= TagDispatch(("tag1", rule1), ("tag2", rule2))
rule1 ::= "abcd"
rule2 ::= "efg"
"#;
    let g = Grammar::from_ebnf(grammar_str, "root");
    assert!(is_grammar_accept_string(&g, "tag1abcd"));
    assert!(is_grammar_accept_string(&g, "tag1abcdtag2efg"));
    assert!(is_grammar_accept_string(&g, "tag1abcdqqqqtag2efg"));
    assert!(!is_grammar_accept_string(&g, "tag1abc"));
    assert!(!is_grammar_accept_string(&g, "tag1abce"));
    assert!(!is_grammar_accept_string(&g, "ttag1abd"));
}

#[test]
#[serial]
fn test_complex_rule() {
    let grammar_str = r#"root ::= TagDispatch(("tag1", rule1), ("tag2", rule2))
rule1 ::= "abcd" [p]*
rule2 ::= "efg" [t]*
"#;
    let g = Grammar::from_ebnf(grammar_str, "root");
    assert!(is_grammar_accept_string(&g, "tag1abcd"));
    assert!(is_grammar_accept_string(&g, "tag1abcdppppptag2efg"));
    assert!(is_grammar_accept_string(&g, "tag2efgtttttag1abc"));
    assert!(!is_grammar_accept_string(&g, "tag1efg"));
}

#[test]
#[serial]
fn test_no_loop_after_dispatch() {
    let grammar_str = r#"root ::= TagDispatch(("tag1", rule1), ("tag2", rule2), loop_after_dispatch=false)
rule1 ::= "abcd" [p]*
rule2 ::= "efg" [t]*
"#;
    let g = Grammar::from_ebnf(grammar_str, "root");
    assert!(is_grammar_accept_string(&g, "tag1abcd"));
    assert!(is_grammar_accept_string(&g, "tag2efgttt"));
    assert!(!is_grammar_accept_string(&g, "tag1abcdppppptag2"));
    assert!(!is_grammar_accept_string(&g, "tag2efgtag1"));
}

#[test]
#[serial]
fn test_stop_str() {
    let grammar_str = r#"root ::= root1 "w"
root1 ::= TagDispatch(
  ("tag1", rule1),
  ("tag2", rule2),
  stop_eos=false,
  stop_str=("tag3", "ll")
)
rule1 ::= "abcd" [p]*
rule2 ::= "efg" [t]*
"#;
    let g = Grammar::from_ebnf(grammar_str, "root");
    assert!(is_grammar_accept_string(&g, "tag1abcdllw"));
    assert!(is_grammar_accept_string(&g, "tag1abcdtag3w"));
    assert!(is_grammar_accept_string(&g, "tag1abcdqqqtag2efgtag3w"));
    // Non-terminated allowance
    assert!(matcher_from_grammar(&g).accept_string("tag1abcd", false));
    assert!(matcher_from_grammar(&g).accept_string("tag2efgttt", false));
    // But requiring termination should fail
    assert!(!is_grammar_accept_string(&g, "tag1abcd"));
    assert!(!is_grammar_accept_string(&g, "tag2efgttt"));
    assert!(!is_grammar_accept_string(&g, "tag1abce"));
    // This should not be accepted even without termination requirement
    assert!(!matcher_from_grammar(&g).accept_string("tag1abcdlltag3w", false));
}

#[test]
#[serial]
fn test_stop_str_no_loop() {
    let grammar_str = r#"root ::= root1 "w"
root1 ::= TagDispatch(
  ("tag1", rule1),
  ("tag2", rule2),
  stop_eos=false,
  stop_str=("tag3", "ll"),
  loop_after_dispatch=false
)
rule1 ::= "abcd" [p]*
rule2 ::= "efg" [t]*
"#;
    let g = Grammar::from_ebnf(grammar_str, "root");
    assert!(is_grammar_accept_string(&g, "tag1abcdllw"));
    assert!(is_grammar_accept_string(&g, "tag1abcdtag3w"));
    assert!(matcher_from_grammar(&g).accept_string("tag1abcd", false));
    assert!(matcher_from_grammar(&g).accept_string("tag2efgttt", false));
    assert!(!is_grammar_accept_string(&g, "tag1abcdqqqtag2efgtag3w"));
    assert!(!is_grammar_accept_string(&g, "tag1abcd"));
    assert!(!is_grammar_accept_string(&g, "tag2efgttt"));
    assert!(!is_grammar_accept_string(&g, "tag1abce"));
    assert!(!matcher_from_grammar(&g).accept_string("tag1abcdlltag3w", false));
}

#[test]
#[serial]
fn test_regression_multiple_tag_dispatch() {
    let grammar_str = r#"root ::= root1 "w"
root1 ::= TagDispatch(
  ("tag1", rule1),
  ("tag2", rule2),
  stop_eos=true,
  stop_str=(),
  loop_after_dispatch=false
)
rule1 ::= TagDispatch(
  ("tag1", rule2),
  ("tag2", rule3),
  stop_eos=false,
  stop_str=("tag3", "ll"),
  loop_after_dispatch=true
)
rule2 ::= "efg" [t]*
rule3 ::= "abcd" [p]*
"#;
    assert!(is_grammar_accept_string(
        &Grammar::from_ebnf(grammar_str, "root"),
        "tag1tag1efgllw"
    ));
    assert!(is_grammar_accept_string(
        &Grammar::from_ebnf(grammar_str, "root"),
        "tag1tag2abcdtag3w"
    ));
    assert!(!is_grammar_accept_string(
        &Grammar::from_ebnf(grammar_str, "root"),
        "tag1Ktag2abcdtag3tag1"
    ));
    assert!(is_grammar_accept_string(
        &Grammar::from_ebnf(grammar_str, "root"),
        "tag1tag3w"
    ));
    assert!(!is_grammar_accept_string(
        &Grammar::from_ebnf(grammar_str, "root"),
        "tag1tag3tag2abcdll"
    ));
}
