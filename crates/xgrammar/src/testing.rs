//! Test and debug helpers re-exported for bindings and integration tests.

use crate::{fsm::CompactFsmWithStartEnd, grammar::Grammar, support::escape_bytes, tokenizer::TokenizerInfo};

/// Formats token ids for debugging — the C++ `PrintTokenByIds`.
#[must_use]
pub fn print_token_by_ids(
    token_ids: &[i32],
    tokenizer_info: &TokenizerInfo,
    max_print_num: i32,
) -> String {
    let vocab = tokenizer_info.decoded_vocab();
    let print_num = (token_ids.len() as i32).min(max_print_num).max(0) as usize;
    let mut out = String::from("[");
    for (i, &token_id) in token_ids.iter().take(print_num).enumerate() {
        let escaped = if token_id >= 0 && (token_id as usize) < vocab.len() {
            escape_bytes(&vocab[token_id as usize])
        } else {
            String::new()
        };
        out.push('#');
        out.push_str(&token_id.to_string());
        out.push_str(" <");
        out.push_str(&escaped);
        out.push('>');
        if i + 1 < print_num {
            out.push_str(", ");
        }
    }
    if token_ids.len() > print_num {
        out.push_str(", ...");
    }
    out.push(']');
    out
}

fn format_compact_fsm_with_start_end(fsm: &CompactFsmWithStartEnd) -> String {
    let end_states: Vec<String> = fsm
        .ends()
        .iter()
        .enumerate()
        .filter_map(|(state, &accepting)| accepting.then_some(state.to_string()))
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
