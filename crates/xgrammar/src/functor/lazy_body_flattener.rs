//! Rewrites lazy rule bodies into their terminal-like form where possible.
//!
//! Committed-shortest (lazy) matching needs the whole rule body to compile into a single per-rule
//! FSM. This pass unwraps the single-reference chains produced by regex conversion, and flattens the
//! right-recursive plus pattern (`x ::= cc x | cc`) and star pattern (`x ::= cc x | ""`) produced by
//! regex conversion and repetition expansion into `(cc cc*)` and `(cc*)`. Grammars without lazy
//! rules are returned unchanged.

use super::mutator::{GrammarMutator, MutatorState};
use crate::grammar::{CharacterClassElement, Grammar, GrammarBuilder, GrammarExpr, GrammarExprType};

/// Bound on the depth of reference chains and nested patterns that are resolved.
const MAX_DEPTH: i32 = 64;

const MAX_CODEPOINT: i32 = 0x10_FFFF;

/// Flattens the bodies of lazy rules (the C++ `LazyBodyFlattenerImpl`).
#[must_use]
pub fn lazy_body_flattener(grammar: &Grammar) -> Grammar {
    if !grammar.rules().iter().any(|rule| rule.is_lazy) {
        return grammar.clone();
    }
    let mut state = MutatorState {
        base: grammar,
        builder: GrammarBuilder::new(),
        cur_rule_name: String::new(),
    };
    for rule in grammar.rules() {
        state.builder.add_empty_rule(rule.name.clone());
    }
    for (rule_id, rule) in (0..).zip(grammar.rules()) {
        state.cur_rule_name = rule.name.clone();
        let new_body_expr_id = if rule.is_lazy {
            build_flattened_lazy_body(&mut state, rule.body_expr_id)
        } else {
            LazyBodyFlattener.visit_expr_id(&mut state, rule.body_expr_id)
        };
        state.builder.update_rule_body(rule_id, new_body_expr_id);
        let new_lookahead_id = LazyBodyFlattener.visit_lookahead(&mut state, rule.lookahead_assertion_id);
        state.builder.update_lookahead_assertion(rule_id, new_lookahead_id);
        state.builder.copy_rule_attributes(rule_id, grammar, rule_id, |id| id);
    }
    state.builder.into_grammar_with_root_id(grammar.root_rule_id())
}

/// The identity mutator, used to copy the parts that are not flattened.
struct LazyBodyFlattener;

impl GrammarMutator for LazyBodyFlattener {}

fn build_flattened_lazy_body(
    state: &mut MutatorState,
    body_expr_id: i32,
) -> i32 {
    let base = state.base;
    if matches!(base.expr(body_expr_id).ty, GrammarExprType::Regex | GrammarExprType::Substring) {
        return LazyBodyFlattener.visit_expr_id(state, body_expr_id);
    }
    // Unwrap chains of single rule references (r ::= (x), x ::= (y), ...) produced by regex
    // conversion, and detect the plus-desugar pattern at the top level.
    let mut cur_body_id = body_expr_id;
    let mut cur_rule_id = None;
    for _ in 0..MAX_DEPTH {
        let Some(rule_id) = sole_element(base, cur_body_id).and_then(|element_id| non_lazy_rule_ref(base, element_id))
        else {
            break;
        };
        cur_rule_id = Some(rule_id);
        cur_body_id = base.rule(rule_id).body_expr_id;
    }

    let body = base.expr(cur_body_id);
    if body.ty != GrammarExprType::Choices {
        eprintln!("The body of the lazy rule '{}' cannot be flattened into a terminal-like form", state.cur_rule_name);
        return LazyBodyFlattener.visit_expr_id(state, cur_body_id);
    }
    let mut repeat_elements = Vec::new();
    if cur_rule_id.is_some_and(|rule_id| try_emit_repeat_pattern(state, cur_body_id, rule_id, &mut repeat_elements)) {
        let sequence = state.builder.add_sequence(&repeat_elements);
        return state.builder.add_choices(&[sequence]);
    }
    let mut new_choice_ids = Vec::with_capacity(body.len());
    for &choice_id in body.data {
        if base.expr(choice_id).ty != GrammarExprType::Sequence {
            new_choice_ids.push(LazyBodyFlattener.visit_expr_id(state, choice_id));
            continue;
        }
        let mut elements = Vec::new();
        if !flatten_sequence_into(state, choice_id, &mut elements, 0) {
            // Not flattenable; copy as is and let the terminal-like validation report the error.
            eprintln!(
                "The body of the lazy rule '{}' cannot be flattened into a terminal-like form",
                state.cur_rule_name
            );
            return LazyBodyFlattener.visit_expr_id(state, cur_body_id);
        }
        new_choice_ids.push(state.builder.add_sequence(&elements));
    }
    state.builder.add_choices(&new_choice_ids)
}

/// Appends the flattened elements of the sequence, splicing rule references whose body is a single
/// terminal-like sequence or the plus-desugar pattern, and coalescing references to single-character
/// alternations into character classes. Returns false if some element cannot be flattened.
fn flatten_sequence_into(
    state: &mut MutatorState,
    sequence_id: i32,
    elements: &mut Vec<i32>,
    depth: i32,
) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    let base = state.base;
    for &element_id in base.expr(sequence_id).data {
        let element = base.expr(element_id);
        if matches!(
            element.ty,
            GrammarExprType::ByteString | GrammarExprType::CharacterClass | GrammarExprType::CharacterClassStar
        ) {
            elements.push(state.builder.add_grammar_expr(element.ty, element.data));
            continue;
        }
        let Some(ref_rule_id) = non_lazy_rule_ref(base, element_id) else {
            return false;
        };
        let ref_body_id = base.rule(ref_rule_id).body_expr_id;
        if try_emit_repeat_pattern(state, ref_body_id, ref_rule_id, elements) {
            continue;
        }
        let ref_body = base.expr(ref_body_id);
        if ref_body.ty == GrammarExprType::Choices && ref_body.len() == 1 {
            let only_choice = base.expr(ref_body.data[0]);
            if only_choice.ty == GrammarExprType::EmptyStr {
                continue;
            }
            if only_choice.ty == GrammarExprType::Sequence
                && flatten_sequence_into(state, ref_body.data[0], elements, depth + 1)
            {
                continue;
            }
        }
        let mut ranges = Vec::new();
        if collect_single_char_ranges(base, element_id, &mut ranges, 0) {
            elements.push(state.builder.add_character_class(&union_ranges(ranges), false));
            continue;
        }
        return false;
    }
    true
}

/// Resolves an expr matching exactly one character into the codepoint ranges it accepts, appending
/// them to `ranges`. Accepts character classes, single-byte strings, and references to non-lazy
/// rules that are alternations of such elements.
fn collect_single_char_ranges(
    base: &Grammar,
    expr_id: i32,
    ranges: &mut Vec<CharacterClassElement>,
    depth: i32,
) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    let expr = base.expr(expr_id);
    match expr.ty {
        GrammarExprType::CharacterClass => {
            append_positive_ranges(expr, ranges);
            return true;
        },
        GrammarExprType::ByteString if expr.len() == 1 => {
            ranges.push(CharacterClassElement::new(expr.data[0], expr.data[0]));
            return true;
        },
        _ => {},
    }
    let Some(rule_id) = non_lazy_rule_ref(base, expr_id) else {
        return false;
    };
    let body = base.expr(base.rule(rule_id).body_expr_id);
    body.ty == GrammarExprType::Choices
        && body.data.iter().all(|&choice_id| {
            let choice = base.expr(choice_id);
            choice.ty == GrammarExprType::Sequence
                && choice.len() == 1
                && collect_single_char_ranges(base, choice.data[0], ranges, depth + 1)
        })
}

/// If the body matches the plus-desugar pattern (`self ::= e self | e`) or the (generalized)
/// star-desugar pattern (`self ::= "" | e1 self | e2 self | ...`), appends the equivalent `(e e*)` or
/// `((e1|e2|...)*)` to `elements` and returns true.
fn try_emit_repeat_pattern(
    state: &mut MutatorState,
    body_id: i32,
    self_rule_id: i32,
    elements: &mut Vec<i32>,
) -> bool {
    try_emit_plus_pattern(state, body_id, self_rule_id, elements)
        || try_emit_generalized_plus_pattern(state, body_id, self_rule_id, elements)
        || try_emit_star_pattern(state, body_id, self_rule_id, elements)
}

/// If the body matches the generalized plus-desugar pattern (`self ::= e1 self | ... | e1 | ...`)
/// with single-character elements whose recursive and base unions are equal, appends the equivalent
/// `(cc cc*)` over the union to `elements` and returns true.
fn try_emit_generalized_plus_pattern(
    state: &mut MutatorState,
    body_id: i32,
    self_rule_id: i32,
    elements: &mut Vec<i32>,
) -> bool {
    let base = state.base;
    let body = base.expr(body_id);
    if body.ty != GrammarExprType::Choices {
        return false;
    }
    let mut recursive_ranges = Vec::new();
    let mut base_ranges = Vec::new();
    for &choice_id in body.data {
        let choice = base.expr(choice_id);
        if choice.ty != GrammarExprType::Sequence {
            return false;
        }
        if choice.len() == 1 {
            if !collect_single_char_ranges(base, choice.data[0], &mut base_ranges, 0) {
                return false;
            }
            continue;
        }
        if choice.len() == 2
            && is_rule_ref_to(base, choice.data[1], self_rule_id)
            && collect_single_char_ranges(base, choice.data[0], &mut recursive_ranges, 0)
        {
            continue;
        }
        return false;
    }
    let recursive_ranges = union_ranges(recursive_ranges);
    // The unions must coincide: with differing sets (e.g. self ::= a self | b, which is a*b), the
    // language is not (a|b)+.
    if recursive_ranges.is_empty() || recursive_ranges != union_ranges(base_ranges) {
        return false;
    }
    elements.push(state.builder.add_character_class(&recursive_ranges, false));
    elements.push(state.builder.add_character_class_star(&recursive_ranges, false));
    true
}

/// If the body matches the plus-desugar pattern (`self ::= e self | e`) with `e` resolving to a
/// star-expressible expr, appends the equivalent `(e e*)` (or `(e*)` when `e` itself resolves to a
/// star) to `elements` and returns true.
fn try_emit_plus_pattern(
    state: &mut MutatorState,
    body_id: i32,
    self_rule_id: i32,
    elements: &mut Vec<i32>,
) -> bool {
    let base = state.base;
    let body = base.expr(body_id);
    if body.ty != GrammarExprType::Choices || body.len() != 2 {
        return false;
    }
    for recursive_pos in 0..2 {
        let recursive = base.expr(body.data[recursive_pos]);
        let base_choice = base.expr(body.data[1 - recursive_pos]);
        if recursive.ty != GrammarExprType::Sequence
            || recursive.len() != 2
            || base_choice.ty != GrammarExprType::Sequence
            || base_choice.len() != 1
        {
            continue;
        }
        if !is_rule_ref_to(base, recursive.data[1], self_rule_id)
            || base.expr(recursive.data[0]) != base.expr(base_choice.data[0])
        {
            continue;
        }
        let Some(resolved_id) = resolve_star_expressible(base, recursive.data[0]) else {
            // Not a single terminal; e may still be a single-character alternation, giving
            // (e1|e2|...)+ = cc cc* over the union of the ranges.
            let mut ranges = Vec::new();
            if !collect_single_char_ranges(base, recursive.data[0], &mut ranges, 0) {
                continue;
            }
            let ranges = union_ranges(ranges);
            elements.push(state.builder.add_character_class(&ranges, false));
            elements.push(state.builder.add_character_class_star(&ranges, false));
            return true;
        };
        let resolved = base.expr(resolved_id);
        if resolved.ty == GrammarExprType::CharacterClassStar {
            // (e*)+ is e*.
            elements.push(state.builder.add_grammar_expr(resolved.ty, resolved.data));
            return true;
        }
        let (is_negative, character_ranges) = if resolved.ty == GrammarExprType::CharacterClass {
            resolved.character_class()
        } else {
            // A single-byte string.
            (false, vec![CharacterClassElement::new(resolved.data[0], resolved.data[0])])
        };
        elements.push(state.builder.add_grammar_expr(resolved.ty, resolved.data));
        elements.push(state.builder.add_character_class_star(&character_ranges, is_negative));
        return true;
    }
    false
}

/// If the body matches the generalized star-desugar pattern (`self ::= "" | e1 self | e2 self |
/// ...`) with each `e` resolving to character ranges, appends the equivalent single character class
/// star `((e1|e2|...)*)` to `elements` and returns true.
fn try_emit_star_pattern(
    state: &mut MutatorState,
    body_id: i32,
    self_rule_id: i32,
    elements: &mut Vec<i32>,
) -> bool {
    let base = state.base;
    let body = base.expr(body_id);
    if body.ty != GrammarExprType::Choices {
        return false;
    }
    let mut has_empty = false;
    let mut ranges = Vec::new();
    for &choice_id in body.data {
        let choice = base.expr(choice_id);
        if choice.ty == GrammarExprType::EmptyStr {
            has_empty = true;
            continue;
        }
        if choice.ty != GrammarExprType::Sequence
            || choice.len() != 2
            || !is_rule_ref_to(base, choice.data[1], self_rule_id)
            || !collect_star_ranges(base, choice.data[0], &mut ranges, 0)
        {
            return false;
        }
    }
    if !has_empty || ranges.is_empty() {
        return false;
    }
    elements.push(state.builder.add_character_class_star(&union_ranges(ranges), false));
    true
}

/// Resolves an expr repeated under an enclosing star into the codepoint ranges it repeats over,
/// appending them to `ranges`. Accepts character classes, character class stars, single-byte
/// strings, and references to non-lazy rules that are alternations of such elements, or star/plus
/// recursions over them — under an enclosing star, all of these are equivalent to the union of their
/// character ranges.
fn collect_star_ranges(
    base: &Grammar,
    expr_id: i32,
    ranges: &mut Vec<CharacterClassElement>,
    depth: i32,
) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    let expr = base.expr(expr_id);
    match expr.ty {
        GrammarExprType::CharacterClass | GrammarExprType::CharacterClassStar => {
            append_positive_ranges(expr, ranges);
            return true;
        },
        GrammarExprType::ByteString if expr.len() == 1 => {
            ranges.push(CharacterClassElement::new(expr.data[0], expr.data[0]));
            return true;
        },
        _ => {},
    }
    let Some(rule_id) = non_lazy_rule_ref(base, expr_id) else {
        return false;
    };
    let body = base.expr(base.rule(rule_id).body_expr_id);
    if body.ty != GrammarExprType::Choices {
        return false;
    }
    let mut has_empty = false;
    let mut base_elements = Vec::new();
    let mut recursive_elements = Vec::new();
    for &choice_id in body.data {
        let choice = base.expr(choice_id);
        match (choice.ty, choice.len()) {
            (GrammarExprType::EmptyStr, _) => has_empty = true,
            (GrammarExprType::Sequence, 1) => base_elements.push(choice.data[0]),
            (GrammarExprType::Sequence, 2) if is_rule_ref_to(base, choice.data[1], rule_id) => {
                recursive_elements.push(choice.data[0]);
            },
            _ => return false,
        }
    }
    // The safe shapes: an alternation (a | b | ...), a star ("" | e1 self | ...), and a plus
    // (e self | e, or single-character alternated forms with equal recursive/base unions). Mixed
    // shapes like (a self | b) are a*b, whose star is not the union, so they are rejected.
    let collect = if recursive_elements.is_empty() {
        base_elements
    } else if (base_elements.is_empty() && has_empty)
        || (recursive_elements.len() == 1
            && base_elements.len() == 1
            && !has_empty
            && base.expr(recursive_elements[0]) == base.expr(base_elements[0]))
    {
        recursive_elements
    } else if !has_empty {
        let mut recursive_ranges = Vec::new();
        let mut base_ranges = Vec::new();
        if !recursive_elements.iter().all(|&id| collect_single_char_ranges(base, id, &mut recursive_ranges, depth + 1))
            || !base_elements.iter().all(|&id| collect_single_char_ranges(base, id, &mut base_ranges, depth + 1))
        {
            return false;
        }
        let recursive_ranges = union_ranges(recursive_ranges);
        if recursive_ranges != union_ranges(base_ranges) {
            return false;
        }
        ranges.extend_from_slice(&recursive_ranges);
        return true;
    } else {
        return false;
    };
    collect.iter().all(|&id| collect_star_ranges(base, id, ranges, depth + 1))
}

/// Appends the positive codepoint ranges of a character class or character class star,
/// complementing negated classes over `[0, 0x10FFFF]`.
fn append_positive_ranges(
    expr: GrammarExpr,
    ranges: &mut Vec<CharacterClassElement>,
) {
    let (is_negative, class_ranges) = expr.character_class();
    if !is_negative {
        ranges.extend(class_ranges);
        return;
    }
    let mut next = 0;
    for range in union_ranges(class_ranges) {
        if range.lower > next {
            ranges.push(CharacterClassElement::new(next, range.lower - 1));
        }
        next = next.max(range.upper + 1);
    }
    if next <= MAX_CODEPOINT {
        ranges.push(CharacterClassElement::new(next, MAX_CODEPOINT));
    }
}

/// Sorts the ranges and merges overlapping or adjacent ones.
fn union_ranges(mut ranges: Vec<CharacterClassElement>) -> Vec<CharacterClassElement> {
    ranges.sort_by_key(|range| range.lower);
    let mut result: Vec<CharacterClassElement> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match result.last_mut() {
            Some(last) if range.lower <= last.upper + 1 => last.upper = last.upper.max(range.upper),
            _ => result.push(range),
        }
    }
    result
}

/// Resolves an expr through chains of non-lazy single-reference rules to a star-expressible expr: a
/// character class, a single-byte string, or a character class star.
fn resolve_star_expressible(
    base: &Grammar,
    expr_id: i32,
) -> Option<i32> {
    let mut cur_id = expr_id;
    for _ in 0..MAX_DEPTH {
        let cur = base.expr(cur_id);
        if matches!(cur.ty, GrammarExprType::CharacterClass | GrammarExprType::CharacterClassStar)
            || (cur.ty == GrammarExprType::ByteString && cur.len() == 1)
        {
            return Some(cur_id);
        }
        cur_id = sole_element(base, base.rule(non_lazy_rule_ref(base, cur_id)?).body_expr_id)?;
    }
    None
}

/// The only element of a body that is a choices of one sequence of one element.
fn sole_element(
    base: &Grammar,
    body_id: i32,
) -> Option<i32> {
    let body = base.expr(body_id);
    if body.ty != GrammarExprType::Choices || body.len() != 1 {
        return None;
    }
    let choice = base.expr(body.data[0]);
    (choice.ty == GrammarExprType::Sequence && choice.len() == 1).then(|| choice.data[0])
}

/// The referenced rule, if the expr is a reference to a non-lazy rule.
fn non_lazy_rule_ref(
    base: &Grammar,
    expr_id: i32,
) -> Option<i32> {
    let expr = base.expr(expr_id);
    (expr.ty == GrammarExprType::RuleRef && !base.rule(expr.rule_ref_id()).is_lazy).then(|| expr.rule_ref_id())
}

/// Whether the expr is a reference to `rule_id`.
fn is_rule_ref_to(
    base: &Grammar,
    expr_id: i32,
    rule_id: i32,
) -> bool {
    let expr = base.expr(expr_id);
    expr.ty == GrammarExprType::RuleRef && expr.rule_ref_id() == rule_id
}
