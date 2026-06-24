//! EBNF lexing and parsing into the BNF AST. Ported from `cpp/grammar_parser.cc`.
//!
//! One dedicated type per file; re-exported here.

mod ebnf_lexer;
mod lexer_error;
mod token;
mod token_type;

pub use ebnf_lexer::tokenize;
pub use lexer_error::LexerError;
pub use token::{Token, TokenValue};
pub use token_type::TokenType;
