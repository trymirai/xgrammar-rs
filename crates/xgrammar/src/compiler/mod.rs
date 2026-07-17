//! Grammar compilation: preprocessing a grammar and tokenizer into a [`CompiledGrammar`]
//! for use by [`GrammarMatcher`](crate::matcher::GrammarMatcher), with a result cache.

mod adaptive_token_mask;
mod compiled_grammar;
mod grammar_compiler;
mod rule_level_cache;
mod tag_dispatch_optimization;
mod token_mask_cache_builder;

pub use adaptive_token_mask::{AdaptiveTokenMask, AdaptiveTokenMaskCache, AdaptiveTokenMaskStoreType};
pub use compiled_grammar::CompiledGrammar;
pub use grammar_compiler::GrammarCompiler;
pub use rule_level_cache::RuleLevelCache;
