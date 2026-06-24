//! Inlines references to simple rules — a port of `RuleInliner` in
//! `cpp/grammar_functor.cc`.
//!
//! When a choice is a sequence beginning with a reference to an *inlinable* rule (one whose
//! body is a non-empty choices of sequences with no empty string and no rule references),
//! that reference is expanded: each of the referenced rule's choices is spliced in front of
//! the choice's remaining elements. The referenced rule itself is left in place.

use std::collections::HashMap;

use super::mutator::{GrammarMutator, MutatorState};
use crate::grammar::{Grammar, GrammarExprType};

/// Inlines leading references to simple rules (the `GrammarFunctor.rule_inliner` pass).
#[must_use]
pub fn rule_inliner(grammar: &Grammar) -> Grammar {
    RuleInliner {
        cache: HashMap::new(),
    }
    .apply(grammar)
}

struct RuleInliner {
    cache: HashMap<i32, bool>,
}

impl RuleInliner {
    fn can_inline(&mut self, base: &Grammar, rule_id: i32) -> bool {
        if let Some(&cached) = self.cache.get(&rule_id) {
            return cached;
        }
        let result = check_can_inline(base, rule_id);
        self.cache.insert(rule_id, result);
        result
    }
}

impl GrammarMutator for RuleInliner {
    fn visit_choices(&mut self, state: &mut MutatorState, data: &[i32]) -> i32 {
        let mut new_choice_ids = Vec::new();
        for &choice_id in data {
            let (choice_ty, choice_data) = {
                let expr = state.base.expr(choice_id);
                (expr.ty, expr.data.to_vec())
            };

            // Keep empty strings, empty sequences, and sequences not led by a rule ref.
            if choice_ty != GrammarExprType::Sequence || choice_data.is_empty() {
                new_choice_ids.push(self.visit_expr_id(state, choice_id));
                continue;
            }
            let (first_ty, first_data) = {
                let expr = state.base.expr(choice_data[0]);
                (expr.ty, expr.data.to_vec())
            };
            if first_ty != GrammarExprType::RuleRef {
                new_choice_ids.push(self.visit_expr_id(state, choice_id));
                continue;
            }
            let rule_ref_id = first_data[0];
            if !self.can_inline(state.base, rule_ref_id) {
                new_choice_ids.push(self.visit_expr_id(state, choice_id));
                continue;
            }

            // Inline: splice each of the referenced rule's choices in front of the rest of
            // this sequence's elements.
            let other_elements: Vec<i32> = choice_data[1..]
                .iter()
                .map(|&e| self.visit_expr_id(state, e))
                .collect();
            let ref_body = state.base.rule(rule_ref_id).body_expr_id;
            let ref_choice_ids = state.base.expr(ref_body).data.to_vec();
            for ref_choice_id in ref_choice_ids {
                let ref_element_ids = state.base.expr(ref_choice_id).data.to_vec();
                let mut new_sequence = Vec::with_capacity(ref_element_ids.len() + other_elements.len());
                for &re in &ref_element_ids {
                    new_sequence.push(self.visit_expr_id(state, re));
                }
                new_sequence.extend_from_slice(&other_elements);
                new_choice_ids.push(state.builder.add_sequence(&new_sequence));
            }
        }
        state.builder.add_choices(&new_choice_ids)
    }
}

/// A rule is inlinable if its body is a non-empty choices of sequences, with no empty-string
/// choice and no rule-reference element.
fn check_can_inline(base: &Grammar, rule_id: i32) -> bool {
    let body = base.rule(rule_id).body_expr_id;
    let (body_ty, choice_ids) = {
        let expr = base.expr(body);
        (expr.ty, expr.data.to_vec())
    };
    if body_ty != GrammarExprType::Choices || choice_ids.is_empty() {
        return false;
    }
    for choice_id in choice_ids {
        let (choice_ty, element_ids) = {
            let expr = base.expr(choice_id);
            (expr.ty, expr.data.to_vec())
        };
        if choice_ty == GrammarExprType::EmptyStr {
            return false;
        }
        for element_id in element_ids {
            if base.expr(element_id).ty == GrammarExprType::RuleRef {
                return false;
            }
        }
    }
    true
}
