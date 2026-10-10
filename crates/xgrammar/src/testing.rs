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

/// The C++ `CompactFSMWithStartEnd::ToString`: only the states reachable from the start (a view
/// over the grammar's shared complete FSM) and the reachable end states are printed.
fn format_compact_fsm_with_start_end(fsm: &CompactFsmWithStartEnd) -> String {
    let mut reachable: Vec<i32> = fsm.reachable_states().into_iter().collect();
    reachable.sort_unstable();
    let end_states: Vec<String> =
        fsm.ends().iter().filter(|end| reachable.binary_search(end).is_ok()).map(i32::to_string).collect();
    format!(
        "CompactFSM(num_states={}, start={}, end=[{}], edges={})",
        fsm.num_states(),
        fsm.start(),
        end_states.join(", "),
        fsm.fsm().to_fsm().edges_to_string(Some(&reachable)),
    )
}

/// Prints each rule's compiled per-rule FSM using the C++ `ToString` edge format.
///
/// # Errors
/// Returns a message if the grammar's per-rule FSMs have not been built.
pub fn print_grammar_fsms(grammar: &Grammar) -> Result<String, String> {
    if grammar.per_rule_fsms_slice().len() != grammar.num_rules() as usize {
        return Err("The grammar has no per-rule FSMs; build them first".to_owned());
    }
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
    Ok(result)
}

/// Whether the per-rule FSM of `rule_id` accepts `input`.
///
/// # Errors
/// Returns a message if `rule_id` is out of range or the rule has no FSM.
pub fn is_rule_fsm_accept_string(
    grammar: &Grammar,
    rule_id: i32,
    input: &str,
) -> Result<bool, String> {
    if !(0..grammar.per_rule_fsms_slice().len() as i32).contains(&rule_id) {
        return Err(format!("Rule id is out of range or the grammar has no per-rule FSMs: {rule_id}"));
    }
    let fsm =
        grammar.per_rule_fsm(rule_id).ok_or_else(|| format!("Rule {rule_id} does not have a finite-state machine"))?;
    Ok(fsm.fsm().accept_string(input))
}
