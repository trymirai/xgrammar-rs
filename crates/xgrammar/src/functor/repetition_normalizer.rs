//! Normalizes `kRepeat` expressions in place — a port of `RepetitionNormalizer` in
//! `cpp/grammar_functor.cc`.
//!
//! For every repeat, the repeated rule is marked exact-lookahead; if that rule is nullable
//! the repeat's minimum count is lowered to 0 (reducing parser uncertainty). Must run after
//! [`allow_empty_rule_ids`](super::allow_empty_rule_ids) has populated the grammar.

use crate::grammar::{Grammar, GrammarExprType};

/// Applies the repetition normalization in place.
pub fn repetition_normalizer(grammar: &mut Grammar) {
    let allow_empty = grammar.allow_empty_rule_ids().to_vec();
    for expr_id in 0..grammar.num_exprs() {
        let expr = grammar.expr(expr_id);
        if expr.ty != GrammarExprType::Repeat {
            continue;
        }
        let repeat_rule_id = expr.data[0];
        grammar.rule_mut(repeat_rule_id).is_exact_lookahead = true;
        if allow_empty.binary_search(&repeat_rule_id).is_ok() {
            // The repeated rule is nullable: set its minimum count to 0.
            grammar.set_expr_data(expr_id, 1, 0);
        }
    }
}
