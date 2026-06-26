//! A lexical token and its processed value — a port of `EBNFLexer::Token` in
//! `cpp/grammar_parser.h` (the C++ `std::any` value becomes a typed [`TokenValue`]).

use super::token_type::TokenType;

/// The processed payload attached to a token (the C++ `std::any`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenValue {
    /// No payload (punctuation, EOF, …).
    None,
    /// An integer literal value.
    Int(i64),
    /// A boolean literal value.
    Bool(bool),
    /// A decoded string: the value of a string literal, or an identifier's text.
    Str(String),
    /// A codepoint, for a literal character in a character class.
    Codepoint(i32),
}

impl TokenValue {
    /// The integer value, if this is [`TokenValue::Int`].
    #[must_use]
    pub fn as_int(&self) -> Option<i64> {
        match self {
            TokenValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    /// The boolean value, if this is [`TokenValue::Bool`].
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            TokenValue::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// The string value, if this is [`TokenValue::Str`].
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            TokenValue::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The codepoint value, if this is [`TokenValue::Codepoint`].
    #[must_use]
    pub fn as_codepoint(&self) -> Option<i32> {
        match self {
            TokenValue::Codepoint(c) => Some(*c),
            _ => None,
        }
    }
}

/// A single lexical token: its kind, the original source text, a processed value, and the
/// 1-based source position where it began.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// The token kind.
    pub ty: TokenType,
    /// The original source text of the token.
    pub lexeme: String,
    /// The processed value.
    pub value: TokenValue,
    /// 1-based line where the token starts.
    pub line: i32,
    /// 1-based column where the token starts.
    pub column: i32,
}
