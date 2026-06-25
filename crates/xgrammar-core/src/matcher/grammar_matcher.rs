//! The grammar matcher — the string/token-accepting front end over the Earley parser.
//!
//! This is the byte/string-accepting core of `GrammarMatcher` in `cpp/grammar_matcher.cc`
//! (token-level masking and the tokenizer-dependent paths land with the compiler milestone).

use std::sync::Arc;

use crate::{
    functor::grammar_optimizer,
    grammar::Grammar,
    parser::{EarleyParser, ParserState},
};

/// Matches input against a grammar by driving an [`EarleyParser`].
#[derive(Debug)]
pub struct GrammarMatcher {
    parser: EarleyParser,
    terminate_without_stop_token: bool,
    /// Lengths of accepted strings/tokens, for rollback.
    token_length_history: Vec<i32>,
}

impl GrammarMatcher {
    /// Creates a matcher over `grammar`, optimizing it first if needed. With
    /// `terminate_without_stop_token` the matcher is considered terminated once the grammar
    /// is completed (no stop token required) — the mode used for string-acceptance testing.
    #[must_use]
    pub fn from_grammar(
        grammar: &Grammar,
        terminate_without_stop_token: bool,
    ) -> Self {
        let optimized = if grammar.is_optimized() {
            grammar.clone()
        } else {
            grammar_optimizer(grammar)
        };
        let parser = EarleyParser::new(
            Arc::new(optimized),
            ParserState::invalid(),
            true,
        );
        Self {
            parser,
            terminate_without_stop_token,
            token_length_history: Vec::new(),
        }
    }

    /// Accepts `input` byte by byte (see [`Self::accept_bytes`]).
    pub fn accept_string(
        &mut self,
        input: &str,
    ) -> bool {
        self.accept_bytes(input.as_bytes())
    }

    /// Accepts `input` byte by byte. On rejection the parser is rolled back to its prior state
    /// and `false` is returned (the acceptance is transactional).
    pub fn accept_bytes(
        &mut self,
        input: &[u8],
    ) -> bool {
        if self.is_stop_token_accepted() {
            return false;
        }
        for (accepted_cnt, &byte) in input.iter().enumerate() {
            if !self.parser.advance(byte) {
                self.parser.pop_last_states(accepted_cnt as i32);
                return false;
            }
        }
        self.token_length_history.push(input.len() as i32);
        true
    }

    /// Accepts the stop token if the grammar is currently completed.
    pub fn accept_stop_token(&mut self) -> bool {
        if self.terminate_without_stop_token || !self.parser.is_completed() {
            return false;
        }
        self.token_length_history.push(0);
        self.parser.set_stop_token_accepted(true);
        true
    }

    /// Whether the matcher has terminated.
    #[must_use]
    pub fn is_terminated(&self) -> bool {
        if self.terminate_without_stop_token {
            return self.parser.is_completed();
        }
        self.is_stop_token_accepted()
    }

    /// Whether the grammar is currently in a completed (acceptable-stop) state.
    #[must_use]
    pub fn is_completed(&self) -> bool {
        self.parser.is_completed()
    }

    /// Whether the stop token has been accepted.
    #[must_use]
    pub fn is_stop_token_accepted(&self) -> bool {
        self.parser.is_stop_token_accepted()
    }

    /// Resets the matcher to the initial state.
    pub fn reset(&mut self) {
        self.parser.reset();
        self.token_length_history.clear();
    }

    /// The underlying Earley parser.
    #[must_use]
    pub fn parser(&self) -> &EarleyParser {
        &self.parser
    }
}
