//! Errors from [`GrammarMatcher`](super::GrammarMatcher) operations.

/// An error from matcher construction or a matcher operation (the C++ `XGRAMMAR_CHECK`s).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MatcherError {
    /// The matcher already accepted the stop token.
    #[error("GrammarMatcher has terminated after accepting the stop token, but is trying to {action}")]
    Terminated {
        /// The attempted operation.
        action: &'static str,
    },
    /// `override_stop_tokens` was empty.
    #[error("The override_stop_tokens should not be empty")]
    EmptyOverrideStopTokens,
    /// An override stop token id is outside the vocabulary.
    #[error("The override stop token id {token_id} is out of the vocabulary range [0, {vocab_size})")]
    StopTokenOutOfRange {
        /// The offending token id.
        token_id: i32,
        /// The vocabulary size.
        vocab_size: i32,
    },
    /// The default temperature is negative or not finite.
    #[error("The default_temperature must be a finite non-negative number")]
    InvalidDefaultTemperature,
    /// More tokens were rolled back than recorded.
    #[error("Intended to rollback {requested} tokens, but only the last {available} steps of history are saved")]
    RollbackTooFar {
        /// The requested number of tokens.
        requested: i32,
        /// The number of recorded steps.
        available: usize,
    },
    /// A bitmask, temperature, or draft-tree argument has an invalid shape, dtype, or index.
    #[error("{0}")]
    InvalidArgument(String),
}
