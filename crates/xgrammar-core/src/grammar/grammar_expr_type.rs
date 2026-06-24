//! The discriminant for a grammar expression — a port of `GrammarExprType` in
//! `cpp/grammar_impl.h`.

use serde::{Deserialize, Serialize};

/// The kind of a grammar expression.
///
/// Each variant documents the layout of the `i32` data array it owns inside the grammar's
/// flat CSR buffer. The discriminants match the C++ `enum class GrammarExprType : int32_t`
/// exactly, because the type tag is stored as the first `i32` of every expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum GrammarExprType {
    /// `[byte0, byte1, ...]` — a literal byte string.
    ByteString = 0,
    /// `[is_negative, lower0, upper0, lower1, upper1, ...]` — a character class.
    CharacterClass = 1,
    /// Like [`CharacterClass`](Self::CharacterClass) but matched zero or more times.
    CharacterClassStar = 2,
    /// `[]` — the empty string.
    EmptyStr = 3,
    /// `[rule_id]` — a reference to another rule.
    RuleRef = 4,
    /// `[grammar_expr_id0, grammar_expr_id1, ...]` — a sequence of expressions.
    Sequence = 5,
    /// `[grammar_expr_id0, grammar_expr_id1, ...]` — an alternation of expressions.
    Choices = 6,
    /// `[tag_expr0, rule_id0, ..., loop_after_dispatch, excluded_str_expr_id]` — tag dispatch.
    TagDispatch = 7,
    /// `[rule_id, min_repeat_count, max_repeat_count]` — bounded/unbounded repetition.
    Repeat = 8,
    /// `[token_id_0, token_id_1, ...]` — an explicit set of allowed tokens.
    Token = 9,
    /// `[token_id_0, token_id_1, ...]` — an explicit set of excluded tokens.
    ExcludeToken = 10,
    /// `[trigger_cnt, (token_id, rule_id) × N, loop_after_dispatch, exclude_cnt, token_id × M]`.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discriminants_match_layout_order() {
        assert_eq!(GrammarExprType::ByteString.as_i32(), 0);
        assert_eq!(GrammarExprType::TagDispatch.as_i32(), 7);
        assert_eq!(GrammarExprType::TokenTagDispatch.as_i32(), 11);
    }

    #[test]
    fn round_trips_through_i32() {
        for tag in 0..=11 {
            let ty = GrammarExprType::try_from(tag).unwrap();
            assert_eq!(ty.as_i32(), tag);
        }
    }

    #[test]
    fn rejects_unknown_tag() {
        assert_eq!(GrammarExprType::try_from(12), Err(UnknownGrammarExprType(12)));
        assert_eq!(GrammarExprType::try_from(-1), Err(UnknownGrammarExprType(-1)));
    }
}
