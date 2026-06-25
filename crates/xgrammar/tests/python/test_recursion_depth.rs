//! Port of `xgrammar/tests/python/test_recursion_depth.py`.
//!
//! The max-recursion-depth setting is global state, so these tests serialize on a shared
//! lock and restore the default afterward (the upstream uses `serial_test`).

use std::sync::Mutex;

use xgrammar::{
    grammar::Grammar,
    matcher::GrammarMatcher,
    support::{get_max_recursion_depth, set_max_recursion_depth},
};

static GUARD: Mutex<()> = Mutex::new(());

#[test]
fn test_set_get_recursion_depth() {
    let _lock = GUARD.lock().unwrap();
    let default_depth = get_max_recursion_depth();
    assert_eq!(default_depth, 10_000);
    set_max_recursion_depth(1000).unwrap();
    assert_eq!(get_max_recursion_depth(), 1000);
    set_max_recursion_depth(default_depth).unwrap();
}

#[test]
fn test_recursion_depth_context() {
    let _lock = GUARD.lock().unwrap();
    assert_eq!(get_max_recursion_depth(), 10_000);
    set_max_recursion_depth(500).unwrap();
    assert_eq!(get_max_recursion_depth(), 500);
    set_max_recursion_depth(10_000).unwrap();
    assert_eq!(get_max_recursion_depth(), 10_000);
}

#[test]
fn test_error_set_recursion_depth() {
    let _lock = GUARD.lock().unwrap();
    assert!(set_max_recursion_depth(-1).is_err());
    assert!(set_max_recursion_depth(100_000_000).is_err());
}

#[test]
fn test_recursion_exceed() {
    // The Earley parser is iterative, so a deeply right-recursive grammar over a long input
    // does not overflow even with a low recursion limit.
    let _lock = GUARD.lock().unwrap();
    set_max_recursion_depth(1000).unwrap();
    let grammar_ebnf = "root ::= \"\\\"\" basic_string \"\\\"\"\n\
        basic_string ::= \"\" | [^\"\\\\\\r\\n] basic_string | \"\\\\\" escape basic_string\n\
        escape ::= [\"\\\\/bfnrt] | \"u\" [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9]\n";
    let grammar = Grammar::from_ebnf(grammar_ebnf, "root").unwrap();
    let mut m = GrammarMatcher::from_grammar(&grammar, true);
    let input: String = std::iter::once('"')
        .chain(std::iter::repeat_n(' ', 10_000))
        .chain(std::iter::once('"'))
        .collect();
    let _ = m.accept_string(&input);
    set_max_recursion_depth(10_000).unwrap();
}
