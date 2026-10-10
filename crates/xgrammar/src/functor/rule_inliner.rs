//! Inlines references to simple rules, in place.
//!
//! When a choice is a sequence beginning with a reference to an *inlinable* rule, that reference
//! is expanded: each of the referenced rule's choices is spliced in front of the choice's remaining
//! elements. Only choices that actually inline are rebuilt; the rest keep their original ids, and
//! the referenced rule itself is left in place for the dead code eliminator. Inlinability is judged
//! on the original grammar, so the result does not depend on the order rules are rewritten.

use super::in_place_grammar_rewriter::InPlaceGrammarRewriter;
use crate::grammar::{Grammar, GrammarBuilder, GrammarExprType};

/// Inlines leading references to simple rules in place (the `GrammarFunctor.rule_inliner` pass).
pub fn rule_inliner(grammar: &mut Grammar) {
    let mut inliner = RuleInliner {
        original_body_expr_ids: grammar.rules().iter().map(|rule| rule.body_expr_id).collect(),
        can_rule_be_inlined: (0..grammar.num_rules()).map(|rule_id| can_rule_be_inlined(grammar, rule_id)).collect(),
    };
    inliner.apply(grammar);
}

struct RuleInliner {
    original_body_expr_ids: Vec<i32>,
    can_rule_be_inlined: Vec<bool>,
}

impl RuleInliner {
    /// The rule referenced by the first element of `choice_id`, if that choice is a sequence led by
    /// a reference to an inlinable rule.
    fn leading_inlinable_rule(
        &self,
        builder: &GrammarBuilder,
        choice_id: i32,
    ) -> Option<i32> {
        let choice = builder.grammar_expr(choice_id);
        if choice.ty != GrammarExprType::Sequence || choice.is_empty() {
            return None;
        }
        let first_element = builder.grammar_expr(choice.data[0]);
        if first_element.ty != GrammarExprType::RuleRef {
            return None;
        }
        let rule_id = first_element.rule_ref_id();
        self.can_rule_be_inlined[rule_id as usize].then_some(rule_id)
    }
}

impl InPlaceGrammarRewriter for RuleInliner {
    fn visit_choices(
        &mut self,
        builder: &mut GrammarBuilder,
        memo: &mut [i32],
        expr_id: i32,
    ) -> i32 {
        let size = builder.grammar_expr(expr_id).len();
        let mut new_choice_ids: Option<Vec<i32>> = None;
        for index in 0..size {
            let choice_id = builder.grammar_expr(expr_id).data[index];
            let Some(inline_rule_id) = self.leading_inlinable_rule(builder, choice_id) else {
                let new_choice_id = self.visit_expr(builder, memo, choice_id);
                if new_choice_ids.is_none() && new_choice_id != choice_id {
                    new_choice_ids = Some(builder.grammar_expr(expr_id).data[..index].to_vec());
                }
                if let Some(ids) = &mut new_choice_ids {
                    ids.push(new_choice_id);
                }
                continue;
            };
            let ids = new_choice_ids.get_or_insert_with(|| builder.grammar_expr(expr_id).data[..index].to_vec());
            // Prepend each choice of the referenced rule to the rest of this sequence. The element ids
            // are copied and visited before anything is appended.
            let mut rest_element_ids = builder.grammar_expr(choice_id).data[1..].to_vec();
            for element_id in &mut rest_element_ids {
                *element_id = self.visit_expr(builder, memo, *element_id);
            }
            let ref_choice_ids =
                builder.grammar_expr(self.original_body_expr_ids[inline_rule_id as usize]).data.to_vec();
            for ref_choice_id in ref_choice_ids {
                let mut new_sequence = builder.grammar_expr(ref_choice_id).data.to_vec();
                for element_id in &mut new_sequence {
                    *element_id = self.visit_expr(builder, memo, *element_id);
                }
                new_sequence.extend_from_slice(&rest_element_ids);
                ids.push(builder.add_sequence(&new_sequence));
            }
        }
        match new_choice_ids {
            Some(ids) => builder.add_choices(&ids),
            None => expr_id,
        }
    }
}

/// A rule can be inlined iff it carries no per-rule runtime semantics and its body is a non-empty
/// choices of sequences that contain no rule references.
fn can_rule_be_inlined(
    grammar: &Grammar,
    rule_id: i32,
) -> bool {
    let rule = grammar.rule(rule_id);
    // Inlining a budgeted rule would erase the rule its length budget applies to. Inlining a
    // capture-relevant rule would eliminate its completion events, so its capture or hidden span
    // would never be recorded. Inlining a lazy rule would erase its committed-shortest semantics.
    // Inlining a temperature rule would erase the rule its sampling temperature applies to.
    if rule.max_tokens >= 0
        || rule.max_chars >= 0
        || !rule.capture_name.is_empty()
        || grammar.suffix_stop_info(rule_id).is_some()
        || rule.is_lazy
        || rule.temperature.is_some()
    {
        return false;
    }
    let body = grammar.expr(rule.body_expr_id);
    body.ty == GrammarExprType::Choices
        && !body.is_empty()
        && body.data.iter().all(|&choice_id| {
            let choice = grammar.expr(choice_id);
            choice.ty == GrammarExprType::Sequence
                && choice.data.iter().all(|&element_id| grammar.expr(element_id).ty != GrammarExprType::RuleRef)
        })
}
