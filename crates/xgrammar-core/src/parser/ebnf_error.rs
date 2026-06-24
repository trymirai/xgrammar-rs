//! Unified error for turning an EBNF string into a grammar (lexing or parsing).

use super::lexer_error::LexerError;
use super::parse_error::ParserError;

/// A failure while converting EBNF text to a grammar.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EbnfError {
    /// The lexer rejected the input.
    #[error(transparent)]
    Lexer(#[from] LexerError),
    /// The parser rejected the token stream.
    #[error(transparent)]
    Parser(#[from] ParserError),
}

impl EbnfError {
    /// The human-readable message of the underlying error.
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            EbnfError::Lexer(e) => &e.message,
            EbnfError::Parser(e) => &e.message,
        }
    }
}
