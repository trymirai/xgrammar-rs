//! Error raised by the EBNF parser — the Rust equivalent of the C++ `ReportParseError`
//! fatal.

/// A parsing failure, with the 1-based source position where it occurred.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("EBNF parser error at line {line}, column {column}: {message}")]
pub struct ParserError {
    /// 1-based line of the error.
    pub line: i32,
    /// 1-based column of the error.
    pub column: i32,
    /// Human-readable description.
    pub message: String,
}
