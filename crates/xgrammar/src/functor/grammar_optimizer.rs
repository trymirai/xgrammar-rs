//! The grammar optimization pipeline run before parsing.
//!
//! Runs the byte-string fuser and rule inliner (both in place), the repetition-range expander, the
//! lazy body flattener, dead-code eliminator, lookahead analyzer, empty-rule analysis, lazy rule
//! validation, repetition normalizer, and finally the grammar FSM builder, leaving the grammar
//! marked optimized and ready for the Earley parser.

use super::{
    allow_empty_rule_analyzer::allow_empty_rule_ids, byte_string_fuser::byte_string_fuser,
    dead_code_eliminator::dead_code_eliminator, error::FunctorError, grammar_fsm_builder::GrammarFsmBuilder,
    lazy_body_flattener::lazy_body_flattener, lookahead_assertion_analyzer::lookahead_assertion_analyzer,
    repetition_normalizer::repetition_normalizer, repetition_range_expander::repetition_range_expander,
    rule_inliner::rule_inliner,
};
use crate::grammar::{Grammar, GrammarExprType};

/// Optimizes `grammar`, returning a new grammar with per-rule FSMs built.
///
/// # Errors
/// Returns [`FunctorError`] if a repetition range is invalid, a lazy rule body is not
/// terminal-like, or a rule body (e.g. a regex) fails to compile into an automaton.
pub fn grammar_optimizer(grammar: &Grammar) -> Result<Grammar, FunctorError> {
    // The fuser and inliner rewrite in place, so they work on a private copy of the input.
    let mut result = grammar.clone();
    byte_string_fuser(&mut result);
    rule_inliner(&mut result);
    result = repetition_range_expander(&result)?;
    result = lazy_body_flattener(&result);
    result = dead_code_eliminator(&result);
    result = lookahead_assertion_analyzer(&result);
    let ids = allow_empty_rule_ids(&result);
    result.set_allow_empty_rule_ids(ids);
    validate_lazy_rules(&result)?;
    repetition_normalizer(&mut result);
    GrammarFsmBuilder::apply(&mut result)?;
    result.set_optimized(true);
    Ok(result)
}

/// Committed-shortest (lazy) matching requires the whole rule body to compile into a single
/// per-rule FSM without rule references, so that the states of one occurrence are exactly the
/// states with the rule's id.
fn validate_lazy_rules(grammar: &Grammar) -> Result<(), FunctorError> {
    for rule in grammar.rules().iter().filter(|rule| rule.is_lazy) {
        let body = grammar.expr(rule.body_expr_id);
        match body.ty {
            GrammarExprType::Regex | GrammarExprType::Substring => continue,
            GrammarExprType::Choices => {},
            _ => {
                return Err(FunctorError::LazyRuleBodyNotChoices {
                    rule_name: rule.name.clone(),
                });
            },
        }
        for &choice_id in body.data {
            let choice = grammar.expr(choice_id);
            if choice.ty == GrammarExprType::EmptyStr {
                continue;
            }
            let is_terminal = |&element_id: &i32| {
                matches!(
                    grammar.expr(element_id).ty,
                    GrammarExprType::ByteString | GrammarExprType::CharacterClass | GrammarExprType::CharacterClassStar
                )
            };
            if !choice.data.iter().all(is_terminal) {
                return Err(FunctorError::LazyRuleElementNotTerminal {
                    rule_name: rule.name.clone(),
                });
            }
        }
    }
    Ok(())
}
