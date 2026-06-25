//! The compiled grammar — a port of `CompiledGrammar` in `cpp/compiled_grammar.cc`.
//!
//! Bundles an optimized [`Grammar`] with the [`TokenizerInfo`] it was compiled against. The
//! C++ also precomputes a per-state `AdaptiveTokenMask` cache purely to speed up
//! `fill_next_token_bitmask`; that cache is a performance optimization (the masks it yields
//! are identical to the matcher's on-the-fly computation) and is deferred to the perf phase.

use crate::{grammar::Grammar, tokenizer::TokenizerInfo};

/// The preprocessing result that a [`GrammarMatcher`](crate::matcher::GrammarMatcher) runs on:
/// an optimized grammar plus its tokenizer.
#[derive(Debug, Clone)]
pub struct CompiledGrammar {
    grammar: Grammar,
    tokenizer_info: TokenizerInfo,
}

impl CompiledGrammar {
    /// Creates a compiled grammar from an (already optimized) grammar and tokenizer.
    #[must_use]
    pub fn new(
        grammar: Grammar,
        tokenizer_info: TokenizerInfo,
    ) -> Self {
        Self {
            grammar,
            tokenizer_info,
        }
    }

    /// The associated (optimized) grammar.
    #[must_use]
    pub fn grammar(&self) -> &Grammar {
        &self.grammar
    }

    /// The associated tokenizer info.
    #[must_use]
    pub fn tokenizer_info(&self) -> &TokenizerInfo {
        &self.tokenizer_info
    }

    /// An approximate memory footprint of the compiled grammar, in bytes.
    #[must_use]
    pub fn memory_size_bytes(&self) -> usize {
        let exprs = self.grammar.num_exprs() as usize * 4;
        let rules = self.grammar.num_rules() as usize * 32;
        let vocab: usize = self
            .tokenizer_info
            .decoded_vocab()
            .iter()
            .map(|t| t.len() + 16)
            .sum();
        exprs + rules + vocab
    }
}
