//! The discriminant for a grammar expression and the data layout of each kind.
//!
//! See [`GrammarExprType`] for the format of each type of [`GrammarExpr`](super::GrammarExpr).

use serde::{Deserialize, Serialize};

/// The kind of a grammar expression.
///
/// Each variant documents the layout of the `i32` data array it owns inside the grammar's
/// flat CSR buffer. The type tag is stored as the first `i32` of every expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum GrammarExprType {
    /// A string of bytes (0–255). Supports UTF-8 strings.
    ///
    /// Data: `[byte0, byte1, ...]`.
    ByteString = 0,
    /// A range of characters (each character is a Unicode codepoint), e.g. `[a-z]`, `[ac-z]`.
    /// Can be negated: `[^a-z]`, `[^ac-z]`. Only ASCII characters are allowed inside `[]`, but
    /// this expression can accept or reject Unicode characters.
    ///
    /// Data: `[is_negative, lower0, upper0, lower1, upper1, ...]`.
    CharacterClass = 1,
    /// A star quantifier of a character class, e.g. `[a-z]*`, `[^a-z]*`.
    ///
    /// Added for efficient matching of character sequences without recursing into rules. Should
    /// be used as `rule2 ::= character_class_star(id_of_a_character_class_grammar_expr)`.
    ///
    /// Data: same layout as [`CharacterClass`](Self::CharacterClass).
    CharacterClassStar = 2,
    /// The empty string, i.e. `""`.
    ///
    /// Data: `[]`.
    EmptyStr = 3,
    /// A reference to another rule.
    ///
    /// Data: `[rule_id]`.
    RuleRef = 4,
    /// A sequence of grammar expressions, e.g. `("a" "b")`. These expressions are concatenated
    /// together.
    ///
    /// Data: `[grammar_expr_id0, grammar_expr_id1, ...]`.
    Sequence = 5,
    /// A choice of grammar expressions, e.g. `("a" "b") | "c"`. Each expression can be matched.
    ///
    /// Data: `[grammar_expr_id0, grammar_expr_id1, ...]`.
    Choices = 6,
    /// Tag dispatch (internal optimization construct).
    ///
    /// Data: `[tag_expr0, rule_id0, ..., loop_after_dispatch, excluded_str_expr_id]`.
    TagDispatch = 7,
    /// Bounded or unbounded repetition.
    ///
    /// Data: `[rule_id, min_repeat_count, max_repeat_count]`.
    Repeat = 8,
    /// An explicit set of allowed tokens.
    ///
    /// Data: `[token_id_0, token_id_1, ...]`.
    Token = 9,
    /// An explicit set of excluded tokens.
    ///
    /// Data: `[token_id_0, token_id_1, ...]`.
    ExcludeToken = 10,
    /// Token-triggered tag dispatch (internal optimization construct).
    ///
    /// Data: `[trigger_cnt, (token_id, rule_id) × N, loop_after_dispatch, exclude_cnt, token_id × M]`.
    TokenTagDispatch = 11,
}

impl GrammarExprType {
    /// The raw `i32` tag stored in the flat grammar buffer.
    #[must_use]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }
}

/// Error when an `i32` does not correspond to a known [`GrammarExprType`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("unknown grammar expr type tag: {0}")]
pub struct UnknownGrammarExprType(pub i32);

impl TryFrom<i32> for GrammarExprType {
    type Error = UnknownGrammarExprType;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::ByteString,
            1 => Self::CharacterClass,
            2 => Self::CharacterClassStar,
            3 => Self::EmptyStr,
            4 => Self::RuleRef,
            5 => Self::Sequence,
            6 => Self::Choices,
            7 => Self::TagDispatch,
            8 => Self::Repeat,
            9 => Self::Token,
            10 => Self::ExcludeToken,
            11 => Self::TokenTagDispatch,
            other => return Err(UnknownGrammarExprType(other)),
        })
    }
}
