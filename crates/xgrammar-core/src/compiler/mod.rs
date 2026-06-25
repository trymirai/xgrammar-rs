//! Grammar compilation: optimizing a grammar against a tokenizer into a [`CompiledGrammar`]
//! that the matcher runs on, with a result cache. Ported from `cpp/grammar_compiler.cc` and
//! `cpp/compiled_grammar.cc`.
//!
//! One dedicated type per file; re-exported here.

mod compiled_grammar;
mod grammar_compiler;

pub use compiled_grammar::CompiledGrammar;
pub use grammar_compiler::GrammarCompiler;
