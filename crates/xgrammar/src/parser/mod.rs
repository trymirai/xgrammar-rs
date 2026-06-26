//! EBNF lexing and parsing into the BNF AST. Ported from `cpp/grammar_parser.cc`.
//!
//! One dedicated type per file; re-exported here.

mod earley_parser;
mod ebnf_error;
mod ebnf_lexer;
mod ebnf_parser;
mod lexer_error;
mod macro_ir;
mod parse_error;
mod parser_state;
mod repeat_detector;
mod token;
mod token_type;

pub use earley_parser::EarleyParser;
pub use ebnf_error::EbnfError;
pub use ebnf_lexer::tokenize;
pub use ebnf_parser::ebnf_to_grammar_no_normalization;
pub use lexer_error::LexerError;
pub use macro_ir::{MacroArguments, MacroValue};
pub use parse_error::ParserError;
pub use parser_state::ParserState;
pub use repeat_detector::RepeatDetector;
pub use token::{Token, TokenValue};
pub use token_type::TokenType;
