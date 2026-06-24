//! Error raised by the EBNF lexer — the Rust equivalent of the C++ `ReportLexerError`
//! fatal (which aborts); here it is a recoverable [`Result`] error.

/// A lexing failure, with the 1-based source position where it occurred.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("EBNF lexer error at line {line}, column {column}: {message}")]
pub struct LexerError {
    /// 1-based line of the error.
    pub line: i32,
    /// 1-based column of the error.
    pub column: i32,
    /// Human-readable description.
    pub message: String,
}
