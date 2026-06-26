//! Test and debug helpers re-exported for bindings and integration tests.

use crate::{fsm::CompactFsmWithStartEnd, grammar::Grammar};

fn format_compact_fsm_with_start_end(fsm: &CompactFsmWithStartEnd) -> String {
    let end_states: Vec<String> = fsm
        .ends()
        .iter()
        .enumerate()
        .filter_map(|(state, &accepting)| {
            accepting.then_some(state.to_string())
        })
        .collect();
    format!(
        "FSM(num_states={}, start={}, end=[{}], edges={})",
        fsm.num_states(),
        fsm.start(),
        end_states.join(", "),
        fsm.fsm().to_fsm().edges_to_string(None),
    )
}

/// Prints each rule's compiled per-rule FSM using the C++ `ToString` edge format.
#[must_use]
pub fn print_grammar_fsms(grammar: &Grammar) -> String {
    let mut result = String::new();
    for rule_id in 0..grammar.num_rules() {
        let rule = grammar.rule(rule_id);
        result.push_str(&format!("Rule {rule_id}: {}, FSM: ", rule.name));
        if let Some(per_rule) = grammar.per_rule_fsm(rule_id) {
            result.push_str(&format_compact_fsm_with_start_end(per_rule.fsm()));
        } else {
            result.push_str("None");
        }
        result.push('\n');
    }
    result
}
