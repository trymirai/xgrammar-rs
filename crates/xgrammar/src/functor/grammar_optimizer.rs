//! The grammar optimization pipeline run before parsing — a port of `GrammarOptimizer` in
//! `cpp/grammar_functor.cc`.
//!
//! Runs the byte-string fuser, rule inliner, repetition-range expander, dead-code eliminator,
//! lookahead analyzer, empty-rule analysis, repetition normalizer, and finally the grammar
//! FSM builder, leaving the grammar marked optimized and ready for the Earley parser.

use super::{
    allow_empty_rule_analyzer::allow_empty_rule_ids,
    byte_string_fuser::byte_string_fuser,
    dead_code_eliminator::dead_code_eliminator,
    grammar_fsm_builder::GrammarFsmBuilder,
    lookahead_assertion_analyzer::lookahead_assertion_analyzer,
    repetition_normalizer::repetition_normalizer,
    repetition_range_expander::repetition_range_expander,
    rule_inliner::rule_inliner,
};
use crate::grammar::Grammar;

/// Optimizes `grammar`, returning a new grammar with per-rule FSMs built.
#[must_use]
pub fn grammar_optimizer(grammar: &Grammar) -> Grammar {
    let mut result = byte_string_fuser(grammar);
    result = rule_inliner(&result);
    result = repetition_range_expander(&result);
    result = dead_code_eliminator(&result);
    result = lookahead_assertion_analyzer(&result);
    let ids = allow_empty_rule_ids(&result);
    result.set_allow_empty_rule_ids(ids);
    repetition_normalizer(&mut result);
    GrammarFsmBuilder::apply(&mut result);
    result.set_optimized(true);
    result
}
