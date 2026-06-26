//! Errors from [`GrammarMatcher`](super::GrammarMatcher) operations.

/// An error from matcher operations after the stop token has been accepted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "grammar matcher has terminated after accepting the stop token, but is trying to \
     find the next token mask"
)]
pub struct MatcherTerminatedError;
