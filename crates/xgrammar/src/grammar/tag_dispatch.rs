//! Decoded form of a [`GrammarExprType::TagDispatch`] expression.

use super::{grammar::Grammar, grammar_expr::GrammarExpr, grammar_expr_type::GrammarExprType};

/// The number of trailing payload elements after the tag/rule pairs:
/// `loop_after_dispatch` and the excluded-strings expression id.
pub const EXTRA_PARAMETERS: usize = 2;

/// A decoded tag-dispatch expression: a list of `(tag bytes, rule id)` pairs, a looping
/// flag, and a set of excluded byte strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagDispatch {
    /// `(tag, rule_id)` pairs; each tag triggers a dispatch into its rule.
    pub tag_rule_pairs: Vec<(Vec<u8>, i32)>,
    /// Whether dispatching loops after a tag is handled.
    pub loop_after_dispatch: bool,
    /// Byte strings excluded from free-text matching.
    pub excludes: Vec<Vec<u8>>,
}

impl Grammar {
    /// Decodes the tag-dispatch expression with the given id.
    ///
    /// # Panics
    /// Panics if the expression is not a [`GrammarExprType::TagDispatch`] or is malformed.
    #[must_use]
    pub fn tag_dispatch(
        &self,
        expr_id: i32,
    ) -> TagDispatch {
        let expr = self.expr(expr_id);
        assert_eq!(expr.ty, GrammarExprType::TagDispatch, "not a tag dispatch");
        self.decode_tag_dispatch_data(expr.data)
    }

    /// Decodes a tag-dispatch payload (the expr data without its type tag), resolving the
    /// byte-string and excludes expression ids against this grammar.
    pub(crate) fn decode_tag_dispatch_data(
        &self,
        data: &[i32],
    ) -> TagDispatch {
        TagDispatch::decode(data, |expr_id| self.expr(expr_id))
    }
}

impl TagDispatch {
    /// Decodes a tag-dispatch payload (the expr data without its type tag), resolving the
    /// byte-string and excludes expression ids through `expr`.
    ///
    /// # Panics
    /// Panics if the excludes expression is not a choices expr.
    pub fn decode<'a>(
        data: &[i32],
        expr: impl Fn(i32) -> GrammarExpr<'a>,
    ) -> TagDispatch {
        let body_len = data.len() - EXTRA_PARAMETERS;
        let tag_rule_pairs =
            data[..body_len].chunks_exact(2).map(|pair| (expr(pair[0]).byte_string(), pair[1])).collect();
        let loop_after_dispatch = data[data.len() - 2] != 0;
        let exclude_expr = expr(data[data.len() - 1]);
        assert_eq!(exclude_expr.ty, GrammarExprType::Choices, "tag-dispatch excludes must be a choices expr");
        let excludes = exclude_expr.data.iter().map(|&child| expr(child).byte_string()).collect();
        TagDispatch {
            tag_rule_pairs,
            loop_after_dispatch,
            excludes,
        }
    }
}
