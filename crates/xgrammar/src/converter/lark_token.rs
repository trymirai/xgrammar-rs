//! A token produced by the Lark lexer.

use super::{lark_location::LarkLocation, lark_token_type::LarkTokenType};

/// A Lark token: its kind, source text (or re-serialized JSON value), and regex flags.
#[derive(Debug, Clone)]
pub struct LarkToken {
    /// The token kind.
    pub ty: LarkTokenType,
    /// The token text; the pattern for regexes and the serialized value for JSON directives.
    pub text: String,
    /// The flags following a regular expression.
    pub flags: String,
    /// Where the token starts.
    pub location: LarkLocation,
}
