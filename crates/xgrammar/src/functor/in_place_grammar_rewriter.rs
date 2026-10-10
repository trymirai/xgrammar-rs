//! Base for grammar passes that rewrite a grammar in place.
//!
//! The rewriter binds a [`GrammarBuilder`] directly to the target grammar (no copy) and walks every
//! rule body and lookahead assertion. An expr whose subtree is unchanged keeps its original id; a
//! new expr is appended only where a rewrite actually happens, so a pass with nothing to do writes
//! nothing. Changed ids propagate upward: a container is rebuilt when any child changed, and a rule
//! body or lookahead is updated when its expr changed. Stale exprs left behind are removed later by
//! the dead code eliminator.

use crate::grammar::{Grammar, GrammarBuilder, GrammarExprType, NO_EXPR};

/// A pass that rewrites a grammar in place. Override [`Self::visit_sequence`] or
/// [`Self::visit_choices`] to add rewriting.
pub trait InPlaceGrammarRewriter {
    /// Rewrites every rule body and lookahead assertion of `grammar` in place.
    fn apply(
        &mut self,
        grammar: &mut Grammar,
    ) {
        GrammarBuilder::edit_in_place(grammar, |builder| {
            // Expr ids are dense, so the memo is a plain array over the original exprs. Appended exprs
            // are never visited again, so they need no memo slots.
            let mut memo = vec![NO_EXPR; builder.num_grammar_exprs() as usize];
            for rule_id in 0..builder.num_rules() {
                let body_expr_id = builder.get_rule(rule_id).body_expr_id;
                let new_body_expr_id = self.visit_expr(builder, &mut memo, body_expr_id);
                if new_body_expr_id != body_expr_id {
                    builder.update_rule_body(rule_id, new_body_expr_id);
                }
                let lookahead_id = builder.get_rule(rule_id).lookahead_assertion_id;
                if lookahead_id != NO_EXPR {
                    let new_lookahead_id = self.visit_expr(builder, &mut memo, lookahead_id);
                    if new_lookahead_id != lookahead_id {
                        builder.update_lookahead_assertion(rule_id, new_lookahead_id);
                    }
                }
            }
        });
    }

    /// Visits an original expr, returning its rewritten id, or the original id when nothing changed.
    /// `memo` maps each original expr id to its result ([`NO_EXPR`] while unvisited).
    fn visit_expr(
        &mut self,
        builder: &mut GrammarBuilder,
        memo: &mut [i32],
        expr_id: i32,
    ) -> i32 {
        if memo[expr_id as usize] != NO_EXPR {
            return memo[expr_id as usize];
        }
        let result = match builder.grammar_expr(expr_id).ty {
            GrammarExprType::Sequence => self.visit_sequence(builder, memo, expr_id),
            GrammarExprType::Choices => self.visit_choices(builder, memo, expr_id),
            _ => expr_id,
        };
        memo[expr_id as usize] = result;
        result
    }

    /// Rebuilds a sequence when any element changed.
    fn visit_sequence(
        &mut self,
        builder: &mut GrammarBuilder,
        memo: &mut [i32],
        expr_id: i32,
    ) -> i32 {
        match self.visit_children(builder, memo, expr_id) {
            Some(new_element_ids) => builder.add_sequence(&new_element_ids),
            None => expr_id,
        }
    }

    /// Rebuilds a choices when any choice changed.
    fn visit_choices(
        &mut self,
        builder: &mut GrammarBuilder,
        memo: &mut [i32],
        expr_id: i32,
    ) -> i32 {
        match self.visit_children(builder, memo, expr_id) {
            Some(new_choice_ids) => builder.add_choices(&new_choice_ids),
            None => expr_id,
        }
    }

    /// Visits each child of a sequence or choices, returning the new child ids if any changed. No
    /// memory is allocated when no child changes.
    fn visit_children(
        &mut self,
        builder: &mut GrammarBuilder,
        memo: &mut [i32],
        expr_id: i32,
    ) -> Option<Vec<i32>> {
        let size = builder.grammar_expr(expr_id).len();
        let mut new_child_ids: Option<Vec<i32>> = None;
        for index in 0..size {
            // A visit may append exprs, so the parent is re-read by id rather than held as a view.
            let child_id = builder.grammar_expr(expr_id).data[index];
            let new_child_id = self.visit_expr(builder, memo, child_id);
            if new_child_ids.is_none() && new_child_id != child_id {
                // Materialize the already scanned, unchanged prefix on the first change.
                new_child_ids = Some(builder.grammar_expr(expr_id).data[..index].to_vec());
            }
            if let Some(ids) = &mut new_child_ids {
                ids.push(new_child_id);
            }
        }
        new_child_ids
    }
}
