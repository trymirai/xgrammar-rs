//! Port of `xgrammar/tests/python/test_grammar_compiler.py`.
//!
//! The pure `get_allow_empty_rule_ids` case is ported here. The HuggingFace-gated cases
//! (real tokenizers, compiled-grammar bitmask, cache pressure) land with the HF tokenizer.

use xgrammar::{
    compiler::GrammarCompiler,
    tokenizer::{TokenizerInfo, VocabType},
};

fn empty_compiler() -> GrammarCompiler {
    GrammarCompiler::with_defaults(TokenizerInfo::new(
        &[],
        VocabType::Raw,
        None,
        None,
        false,
    ))
}

#[test]
fn test_get_allow_empty_rule_ids() {
    let cases: &[(&str, &[i32])] = &[
        (
            "root ::= rule1 rule2 | \"abc\"\n\
             rule1 ::= \"abc\" | \"\"\n\
             rule2 ::= \"def\" rule3 | \"\"\n\
             rule3 ::= \"ghi\"\n",
            &[0, 1, 2],
        ),
        (
            "root ::= rule1 rule2 [a-z]*\n\
             rule1 ::= \"abc\" | \"\"\n\
             rule2 ::= \"def\" | \"\"\n",
            &[0, 1, 2],
        ),
        (
            "root ::= rule1 rule3\n\
             rule1 ::= \"abc\" | \"\"\n\
             rule2 ::= \"def\" | \"\"\n\
             rule3 ::= rule1 rule2\n",
            &[0, 1, 2, 3],
        ),
        ("root ::= [a]* [b]* rule1\nrule1 ::= [abc]* [def]*\n", &[0, 1]),
    ];
    let compiler = empty_compiler();
    for (grammar, expected) in cases {
        let compiled = compiler.compile_grammar_ebnf(grammar, "root");
        assert_eq!(
            compiled.grammar().allow_empty_rule_ids(),
            *expected,
            "grammar {grammar:?}"
        );
    }
}
