//! Decoded form of a [`GrammarExprType::TokenTagDispatch`] expression — a port of
//! `Grammar::Impl::TokenTagDispatch` and `GetTokenTagDispatch` in `cpp/grammar_impl.h`.

use super::{grammar::Grammar, grammar_expr_type::GrammarExprType};

/// A decoded token-tag-dispatch expression: `(token id, rule id)` triggers, a looping
/// flag, and excluded token ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenTagDispatch {
    /// `(token_id, rule_id)` pairs; each trigger token dispatches into its rule.
    pub trigger_rule_pairs: Vec<(i32, i32)>,
    /// Whether dispatching loops after a tag is handled.
    pub loop_after_dispatch: bool,
    /// Excluded token ids.
    pub excludes: Vec<i32>,
}

impl Grammar {
    /// Decodes the token-tag-dispatch expression with the given id.
    ///
    /// # Panics
    /// Panics if the expression is not a [`GrammarExprType::TokenTagDispatch`] or is malformed.
    #[must_use]
    pub fn token_tag_dispatch(
        &self,
        expr_id: i32,
    ) -> TokenTagDispatch {
        let expr = self.expr(expr_id);
        assert_eq!(
            expr.ty,
            GrammarExprType::TokenTagDispatch,
            "not a token tag dispatch"
        );
        Self::decode_token_tag_dispatch_data(expr.data)
    }

    /// Decodes a token-tag-dispatch payload (the expr data without its type tag).
    pub(crate) fn decode_token_tag_dispatch_data(
        data: &[i32]
    ) -> TokenTagDispatch {
        let mut pos = 0;

        let trigger_count = data[pos] as usize;
        pos += 1;
        let mut trigger_rule_pairs = Vec::with_capacity(trigger_count);
        for _ in 0..trigger_count {
            trigger_rule_pairs.push((data[pos], data[pos + 1]));
            pos += 2;
        }

        let loop_after_dispatch = data[pos] != 0;
        pos += 1;

        let exclude_count = data[pos] as usize;
        pos += 1;
        let mut excludes = Vec::with_capacity(exclude_count);
        for _ in 0..exclude_count {
            excludes.push(data[pos]);
            pos += 1;
        }

        debug_assert_eq!(
            pos,
            data.len(),
            "token-tag-dispatch payload length mismatch"
        );
        TokenTagDispatch {
            trigger_rule_pairs,
            loop_after_dispatch,
            excludes,
        }
    }
}
