//! EBNF lexing and parsing into the BNF AST. Ported from `cpp/grammar_parser.cc`.
//!
//! One dedicated type per file; re-exported here.

mod ebnf_error;
mod ebnf_lexer;
mod ebnf_parser;
mod lexer_error;
mod parse_error;
mod token;
mod token_type;

pub use ebnf_error::EbnfError;
pub use ebnf_lexer::tokenize;
pub use ebnf_parser::ebnf_to_grammar_no_normalization;
pub use lexer_error::LexerError;
pub use parse_error::ParserError;
pub use token::{Token, TokenValue};
pub use token_type::TokenType;
