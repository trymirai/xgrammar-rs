//! Expands bounded/unbounded repetitions into explicit rules — a port of
//! `RepetitionRangeExpander` in `cpp/grammar_functor.cc`.
//!
//! Small bounds are "unzipped" into repeated elements and a tail of optional rules; large
//! bounds keep a `Repeat` node wrapped so the matcher can count efficiently. Lookups go
//! through the builder (never the source grammar) so expanding two large repeats in one
//! rule cannot index out of bounds.

use super::mutator::{GrammarMutator, MutatorState};
use crate::grammar::{CharacterClassElement, Grammar, GrammarExprType};

const UNZIP_THRESHOLD: i64 = 128;

/// Expands repetition ranges (the `GrammarFunctor.repetition_range_expander` pass).
#[must_use]
pub fn repetition_range_expander(grammar: &Grammar) -> Grammar {
    RepetitionRangeExpander.apply(grammar)
}

struct RepetitionRangeExpander;

impl GrammarMutator for RepetitionRangeExpander {
    fn visit_repeat(
        &mut self,
        state: &mut MutatorState,
        _ty: GrammarExprType,
        data: &[i32],
    ) -> i32 {
        let ref_rule_id = data[0];
        let lower = i64::from(data[1]);
        let upper = i64::from(data[2]);
        let name = state.cur_rule_name.clone();
        handle_repetition_range(state, &name, ref_rule_id, lower, upper)
    }
}

/// Unzips a repetition into explicit sequences/choices (used for small bounds).
fn legacy_handle_repetition_range(
    state: &mut MutatorState,
    cur_rule_name: &str,
    grammar_expr_id: i32,
    lower: i64,
    upper: i64,
) -> i32 {
    // Construct expr expr ... expr (lower times).
    let mut elements: Vec<i32> = vec![grammar_expr_id; lower.max(0) as usize];

    // Case 1: {l} — expr repeated l times.
    if upper == lower {
        let seq = state.builder.add_sequence(&elements);
        let choices = state.builder.add_choices(&[seq]);
        let result_rule_id =
            state.builder.add_rule_with_hint(cur_rule_name, choices);
        return state.builder.add_rule_ref(result_rule_id);
    }

    // Case 2: {l,} — expr repeated l times, then `rest ::= "" | expr rest`.
    if upper == -1 {
        let new_rule_name = state.builder.get_new_rule_name(cur_rule_name);
        let new_rule_id = state.builder.add_empty_rule(new_rule_name);
        let ref_to_new_rule = state.builder.add_rule_ref(new_rule_id);
        let empty = state.builder.add_empty_str();
        let seq =
            state.builder.add_sequence(&[grammar_expr_id, ref_to_new_rule]);
        let body = state.builder.add_choices(&[empty, seq]);
        state.builder.update_rule_body(new_rule_id, body);
        elements.push(state.builder.add_rule_ref(new_rule_id));
        let final_seq = state.builder.add_sequence(&elements);
        let choices = state.builder.add_choices(&[final_seq]);
        let result_rule_id =
            state.builder.add_rule_with_hint(cur_rule_name, choices);
        return state.builder.add_rule_ref(result_rule_id);
    }

    // Case 3: {l, r} with r - l >= 1 — a chain of optional rules.
    let extra = (upper - lower) as usize;
    let mut rest_rule_ids = Vec::with_capacity(extra);
    for _ in 0..extra {
        let new_rule_name = state.builder.get_new_rule_name(cur_rule_name);
        rest_rule_ids.push(state.builder.add_empty_rule(new_rule_name));
    }
    for i in 0..extra.saturating_sub(1) {
        let ref_to_next_rule = state.builder.add_rule_ref(rest_rule_ids[i + 1]);
        let empty = state.builder.add_empty_str();
        let seq =
            state.builder.add_sequence(&[grammar_expr_id, ref_to_next_rule]);
        let body = state.builder.add_choices(&[empty, seq]);
        state.builder.update_rule_body(rest_rule_ids[i], body);
    }
    let empty = state.builder.add_empty_str();
    let last_seq = state.builder.add_sequence(&[grammar_expr_id]);
    let last_body = state.builder.add_choices(&[empty, last_seq]);
    let last_rule = *rest_rule_ids.last().unwrap();
    state.builder.update_rule_body(last_rule, last_body);

    elements.push(state.builder.add_rule_ref(rest_rule_ids[0]));
    let final_seq = state.builder.add_sequence(&elements);
    let choices = state.builder.add_choices(&[final_seq]);
    let result_rule_id =
        state.builder.add_rule_with_hint(cur_rule_name, choices);
    state.builder.add_rule_ref(result_rule_id)
}

/// Handles `{lower, upper}`, unzipping for small bounds and keeping a `Repeat` for large.
fn handle_repetition_range(
    state: &mut MutatorState,
    cur_rule_name: &str,
    rule_id: i32,
    mut lower: i64,
    mut upper: i64,
) -> i32 {
    // If the referred rule is a single element, use that element directly.
    let mut grammar_expr_id = state.builder.add_rule_ref(rule_id);
    let inline_element = {
        let ref_rule_body =
            state.base.expr(state.base.rule(rule_id).body_expr_id);
        if ref_rule_body.ty == GrammarExprType::Choices
            && ref_rule_body.data.len() == 1
        {
            let ref_choice = state.base.expr(ref_rule_body.data[0]);
            if ref_choice.ty == GrammarExprType::Sequence
                && ref_choice.data.len() == 1
            {
                let element = state.base.expr(ref_choice.data[0]);
                Some((element.ty, element.data.to_vec()))
            } else {
                None
            }
        } else {
            None
        }
    };
    if let Some((ty, data)) = inline_element {
        grammar_expr_id = state.builder.add_grammar_expr(ty, &data);
    }

    // Case 1: small upper, or unbounded upper with small lower — unzip.
    if (upper != -1 && upper <= UNZIP_THRESHOLD)
        || (upper == -1 && lower <= UNZIP_THRESHOLD)
    {
        return legacy_handle_repetition_range(
            state,
            cur_rule_name,
            grammar_expr_id,
            lower,
            upper,
        );
    }

    // Case 2: upper unbounded with large lower, or upper bounded but > threshold.
    let mut choices = Vec::new();
    if lower < UNZIP_THRESHOLD {
        let unzipped = legacy_handle_repetition_range(
            state,
            cur_rule_name,
            grammar_expr_id,
            lower,
            UNZIP_THRESHOLD - 1,
        );
        choices.push(state.builder.add_sequence(&[unzipped]));
        lower = UNZIP_THRESHOLD;
    }

    let mut infinite_repetition_id: Option<i32> = None;
    let mut repeated_sequence = Vec::new();
    // Case 2.2: unbounded upper becomes `{lower} {0, inf}`.
    if upper == -1 {
        let char_class = {
            let expr = state.builder.grammar_expr(grammar_expr_id);
            (expr.ty == GrammarExprType::CharacterClass)
                .then(|| expr.character_class())
        };
        if let Some((is_negative, ranges)) = char_class {
            let ranges: Vec<CharacterClassElement> = ranges;
            infinite_repetition_id = Some(
                state.builder.add_character_class_star(&ranges, is_negative),
            );
        } else {
            let unbounded_name = state
                .builder
                .get_new_rule_name(&format!("{cur_rule_name}_repeat_inf"));
            let unbounded_rule_id =
                state.builder.add_empty_rule(unbounded_name);
            let ref_unbounded = state.builder.add_rule_ref(unbounded_rule_id);
            let recursion_sequence =
                state.builder.add_sequence(&[grammar_expr_id, ref_unbounded]);
            let empty = state.builder.add_empty_str();
            let recursion_choice =
                state.builder.add_choices(&[empty, recursion_sequence]);
            state.builder.update_rule_body(unbounded_rule_id, recursion_choice);
            infinite_repetition_id =
                Some(state.builder.add_rule_ref(unbounded_rule_id));
        }
        upper = lower;
    }

    let repeat_name = format!("{cur_rule_name}_repeat_1");

    if let Some(id) = infinite_repetition_id {
        repeated_sequence.push(id);
    }

    // The repetition body, when upper is strictly above the threshold.
    if upper != UNZIP_THRESHOLD {
        let inner_seq = state.builder.add_sequence(&[grammar_expr_id]);
        let inner_choices = state.builder.add_choices(&[inner_seq]);
        let new_rule_id =
            state.builder.add_rule_with_hint(&repeat_name, inner_choices);
        let repeat = state.builder.add_repeat(
            new_rule_id,
            (lower - UNZIP_THRESHOLD) as i32,
            (upper - UNZIP_THRESHOLD) as i32,
        );
        let repeat_seq = state.builder.add_sequence(&[repeat]);
        let repeat_choices = state.builder.add_choices(&[repeat_seq]);
        let inner_name = format!("{repeat_name}_inner");
        let new_repeated_rule_id =
            state.builder.add_rule_with_hint(&inner_name, repeat_choices);
        repeated_sequence
            .push(state.builder.add_rule_ref(new_repeated_rule_id));
        let repetition_lookahead =
            vec![grammar_expr_id; UNZIP_THRESHOLD as usize];
        let lookahead = state.builder.add_sequence(&repetition_lookahead);
        state.builder.update_lookahead_assertion(new_rule_id, lookahead);
    }

    // The last `threshold` copies of the element.
    for _ in 0..UNZIP_THRESHOLD {
        repeated_sequence.push(grammar_expr_id);
    }

    choices.push(state.builder.add_sequence(&repeated_sequence));
    let result_choices = state.builder.add_choices(&choices);
    let result_rule_id =
        state.builder.add_rule_with_hint(cur_rule_name, result_choices);
    state.builder.add_rule_ref(result_rule_id)
}
