//! Port of `xgrammar/tests/python/test_speculative_decoding.py` (core draft-tree cases).

use xgrammar::{
    Grammar, GrammarCompiler, GrammarMatcher, TokenizerInfo, VocabType,
    allocate_token_bitmask,
};

const VOCAB: &[&str] = &[
    "a", "b", "c", "{", "}", "\"", ":", ",", " ", "true", "false", "null",
];

fn compiled_json() -> xgrammar::CompiledGrammar {
    let grammar = Grammar::builtin_json_grammar();
    let vocab: Vec<String> = VOCAB.iter().map(|s| (*s).to_owned()).collect();
    let tokenizer =
        TokenizerInfo::new(&vocab, VocabType::RAW, None, None, false);
    let compiler = GrammarCompiler::with_defaults(tokenizer);
    compiler.compile_grammar(&grammar)
}

fn run_traverse(
    next_token: &[i64],
    next_sibling: &[i64],
    draft_tokens: &[i64],
) -> (bool, Vec<i32>) {
    let compiled = compiled_json();
    let mut matcher = GrammarMatcher::from_compiled_grammar(&compiled, false);
    let mut bitmask = allocate_token_bitmask(
        next_token.len() as i32,
        VOCAB.len() as i32,
    );
    let ok = matcher
        .traverse_draft_tree(
            next_token,
            next_sibling,
            draft_tokens,
            &mut bitmask,
            0.0,
        )
        .unwrap();
    (ok, bitmask)
}

#[test]
fn traverse_draft_tree_linear() {
    // 0 -> 1 -> 2 with draft tokens `{`, `:`, `}`
    let (ok, bitmask) = run_traverse(&[1, 2, -1], &[-1, -1, -1], &[3, 6, 4]);
    assert!(ok);
    assert!(bitmask.iter().any(|&w| w != 0));
}

#[test]
fn traverse_draft_tree_with_siblings() {
    // root with children 1 and 2; draft `{`, `"`, `}`
    let (ok, bitmask) = run_traverse(&[1, -1, -1], &[-1, 2, -1], &[3, 5, 4]);
    assert!(ok);
    assert!(bitmask.iter().any(|&w| w != 0));
}

#[test]
fn traverse_draft_tree_rejects_empty() {
    let compiled = compiled_json();
    let mut matcher = GrammarMatcher::from_compiled_grammar(&compiled, false);
    let mut bitmask = allocate_token_bitmask(1, VOCAB.len() as i32);
    let err = matcher
        .traverse_draft_tree(&[], &[], &[], &mut bitmask, 0.0)
        .unwrap_err();
    assert!(err.contains("empty"));
}
