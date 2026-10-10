//! Error raised by the regex → EBNF converter — the Rust equivalent of the C++
//! `RegexConverter::RaiseError` fatal.

/// A regex conversion failure, with the 1-based position in the regex where it occurred.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Regex parsing error at position {position}: {message}")]
pub struct RegexError {
    /// 1-based position in the regex.
    pub position: usize,
    /// Human-readable description.
    pub message: String,
}
