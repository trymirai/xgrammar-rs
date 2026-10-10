//! Builds finite-state machines from grammar expressions.
//!
//! Each rule body lowers into an [`FsmWithStartEnd`] built directly into one FSM: byte strings,
//! character classes (canonical UTF-8 codepoint ranges), rule references, tokens, repeats,
//! regexes, substrings, and the tag-dispatch automata. The per-rule automata are then folded into
//! the grammar's shared complete FSM.

use std::sync::OnceLock;

use super::{
    error::FunctorError, expression_fsm_builder::ExpressionFsmBuilder,
    regular_grammar_fsm_builder::RegularGrammarFsmBuilder,
};
use crate::{
    fsm::{
        CompactFsm, CompactFsmWithStartEnd, CompactFsmWithStartEndWithSize, Fsm, FsmWithStartEnd, RegexFsmBuilder,
        TrieFsmBuilder, add_packed_utf8_range_edges, codepoint_to_packed_utf8,
    },
    grammar::{Grammar, GrammarBuilder, GrammarExpr, GrammarExprType, TagDispatch, TokenTagDispatch},
};

/// The largest Unicode codepoint.
const MAX_CODEPOINT: u32 = 0x10FFFF;

/// Stateless builder of per-expression FSMs (the C++ `GrammarFSMBuilder`).
#[derive(Debug, Clone, Copy, Default)]
pub struct GrammarFsmBuilder;

impl GrammarFsmBuilder {
    /// Compiles every rule body into an FSM, folding them into one shared complete FSM stored
    /// on the grammar. Large bounded regex repetitions add new rules, which get FSMs too.
    ///
    /// # Errors
    /// Returns [`FunctorError::FsmBuild`] if a rule body (e.g. a regex) fails to compile.
    pub fn apply(grammar: &mut Grammar) -> Result<(), FunctorError> {
        let num_original_rules = grammar.num_rules();
        let (complete_fsm, per_rule_fsms) = GrammarBuilder::edit_in_place(grammar, |builder| {
            let mut complete_fsm = Fsm::new(0);
            let mut per_rule_fsms = Vec::new();
            // New rules may be appended while building, so the rule count is re-read.
            let mut rule_id = 0;
            while rule_id < builder.num_rules() {
                let rule = builder.get_rule(rule_id);
                let (name, body_expr_id) = (rule.name.clone(), rule.body_expr_id);
                let rule_fsm = ExpressionFsmBuilder::build_expression_fsm(builder, body_expr_id, Some(name), true)?;
                per_rule_fsms.push(rule_fsm.add_to_complete_fsm(&mut complete_fsm));
                rule_id += 1;
            }
            Ok::<_, FunctorError>((complete_fsm, per_rule_fsms))
        })?;

        // The rules created while building missed the allow-empty analysis; their ids are larger
        // than all existing ones, so appending keeps the list sorted.
        let mut allow_empty_rule_ids = grammar.allow_empty_rule_ids().to_vec();
        for rule_id in num_original_rules..grammar.num_rules() {
            let body = grammar.expr(grammar.rule(rule_id).body_expr_id);
            debug_assert_eq!(body.ty, GrammarExprType::Regex);
            if RegexFsmBuilder::matches_empty(&body.regex_pattern()) == Ok(true) {
                allow_empty_rule_ids.push(rule_id);
            }
        }
        grammar.set_allow_empty_rule_ids(allow_empty_rule_ids);

        let compact_complete_fsm = CompactFsm::from_fsm(&complete_fsm);
        let compact_per_rule_fsms = per_rule_fsms
            .iter()
            .map(|fsm| {
                let with_start_end =
                    CompactFsmWithStartEnd::new(compact_complete_fsm.clone(), fsm.start(), fsm.ends().to_vec(), false);
                Some(CompactFsmWithStartEndWithSize::new(with_start_end, fsm.edge_num(), fsm.node_num()))
            })
            .collect();
        grammar.set_fsms(compact_complete_fsm, compact_per_rule_fsms);
        Ok(())
    }

    /// Builds the FSM for a `RuleRef` expression.
    #[must_use]
    pub fn rule_ref(expr: &GrammarExpr) -> FsmWithStartEnd {
        let mut fsm = Fsm::new(2);
        fsm.add_rule_edge(0, 1, expr.data[0]);
        FsmWithStartEnd::new(fsm, 0, vec![1], false)
    }

    /// Builds the FSM for a `ByteString` expression (one transition per byte).
    #[must_use]
    pub fn byte_string(expr: &GrammarExpr) -> FsmWithStartEnd {
        debug_assert_eq!(expr.ty, GrammarExprType::ByteString);
        let mut fsm = Fsm::new(1);
        let end = Self::build_byte_string(&mut fsm, expr.data, 0);
        FsmWithStartEnd::new(fsm, 0, vec![end], false)
    }

    /// Builds the FSM for a `CharacterClass` / `CharacterClassStar` expression.
    #[must_use]
    pub fn character_class(expr: &GrammarExpr) -> FsmWithStartEnd {
        let mut fsm = Fsm::new(1);
        let end = if expr.ty == GrammarExprType::CharacterClassStar {
            0
        } else {
            fsm.add_state()
        };
        Self::add_character_class_transitions(&mut fsm, expr.data, 0, end);
        FsmWithStartEnd::new(fsm, 0, vec![end], false)
    }

    /// Builds the FSM for a `Token` expression (a single token-set edge).
    #[must_use]
    pub fn token(expr: &GrammarExpr) -> FsmWithStartEnd {
        debug_assert_eq!(expr.ty, GrammarExprType::Token);
        let mut fsm = Fsm::new(2);
        fsm.add_token_edge(0, 1, expr.data);
        FsmWithStartEnd::new(fsm, 0, vec![1], false)
    }

    /// Builds the FSM for an `ExcludeToken` expression (a single exclude-token edge).
    #[must_use]
    pub fn exclude_token(expr: &GrammarExpr) -> FsmWithStartEnd {
        debug_assert_eq!(expr.ty, GrammarExprType::ExcludeToken);
        let mut fsm = Fsm::new(2);
        fsm.add_exclude_token_edge(0, 1, expr.data);
        FsmWithStartEnd::new(fsm, 0, vec![1], false)
    }

    /// Builds the dispatch FSM for a token tag-dispatch.
    #[must_use]
    pub fn token_tag_dispatch(token_tag_dispatch: &TokenTagDispatch) -> FsmWithStartEnd {
        let mut fsm = Fsm::new(1);
        let ends = Self::build_token_tag_dispatch(&mut fsm, token_tag_dispatch, 0);
        FsmWithStartEnd::new(fsm, 0, ends, false)
    }

    /// Builds the FSM for a `Sequence` expression of `grammar` (no simplification).
    ///
    /// # Errors
    /// Returns [`FunctorError::FsmBuild`] if an element fails to compile.
    pub fn sequence(
        expr: &GrammarExpr,
        grammar: &Grammar,
    ) -> Result<FsmWithStartEnd, FunctorError> {
        debug_assert_eq!(expr.ty, GrammarExprType::Sequence);
        let mut builder = GrammarBuilder::from_grammar(grammar);
        let sequence_id = builder.add_sequence(expr.data);
        ExpressionFsmBuilder::build(&mut builder, sequence_id, None, false)
    }

    /// Builds the FSM for a `Choices` expression of `grammar`, simplified and minimized.
    ///
    /// # Errors
    /// Returns [`FunctorError::FsmBuild`] if a choice fails to compile.
    pub fn choices(
        expr: &GrammarExpr,
        grammar: &Grammar,
    ) -> Result<FsmWithStartEnd, FunctorError> {
        debug_assert_eq!(expr.ty, GrammarExprType::Choices);
        let mut builder = GrammarBuilder::from_grammar(grammar);
        let choices_id = builder.add_choices(expr.data);
        ExpressionFsmBuilder::build_expression_fsm(&mut builder, choices_id, None, false)
    }

    /// Builds the dispatch FSM for a string tag-dispatch, or `None` if the trie cannot be built.
    #[must_use]
    pub fn tag_dispatch(tag_dispatch: &TagDispatch) -> Option<FsmWithStartEnd> {
        let tag_names: Vec<&[u8]> = tag_dispatch.tag_rule_pairs.iter().map(|(name, _)| name.as_slice()).collect();
        let excluded: Vec<&[u8]> = tag_dispatch.excludes.iter().map(Vec::as_slice).collect();
        let mut end_states: Vec<i32> = Vec::new();
        let trie = TrieFsmBuilder::build(&tag_names, &excluded, Some(&mut end_states), true, true)?;
        let mut trie_fsm = trie.fsm().clone();
        let start = trie.start();

        // The final end states are all states except the trie's terminal (trigger) states.
        let mut ends: Vec<i32> = (0..trie_fsm.num_states()).filter(|&state| !trie.is_end_state(state)).collect();
        // Add a rule-ref edge out of each trigger's terminal state.
        for (i, &(_, rule_id)) in tag_dispatch.tag_rule_pairs.iter().enumerate() {
            let next_state = if tag_dispatch.loop_after_dispatch {
                start
            } else {
                let state = trie_fsm.add_state();
                ends.push(state);
                state
            };
            trie_fsm.add_rule_edge(end_states[i], next_state, rule_id);
        }
        Some(FsmWithStartEnd::new(trie_fsm, start, ends, false))
    }

    /// Compiles a standalone regex into a simplified, minimized automaton. With `json_string`
    /// the characters that must be escaped in a JSON string are excluded.
    ///
    /// # Errors
    /// Returns the regex parse error.
    pub fn regex(
        regex: &[u8],
        json_string: bool,
    ) -> Result<FsmWithStartEnd, String> {
        let result = if json_string {
            RegexFsmBuilder::build_with_forbidden_chars(regex, Self::json_string_forbidden_chars(), None, "")
        } else {
            RegexFsmBuilder::build(regex, None, "")
        }?;
        Ok(result.simplify_epsilon().merge_equivalent_states())
    }

    /// Flattens a regular grammar (no non-tail recursion, no annotations) into one automaton of
    /// at most `max_num_states` intermediate states.
    ///
    /// # Errors
    /// Returns why the grammar cannot be flattened.
    pub fn from_regular_grammar(
        grammar: &Grammar,
        max_num_states: i32,
    ) -> Result<FsmWithStartEnd, String> {
        RegularGrammarFsmBuilder::build(grammar, max_num_states)
    }

    /// The characters that must be escaped in a JSON string: control characters, `"` and `\`.
    #[must_use]
    pub fn json_string_forbidden_chars() -> &'static [bool; 256] {
        static FORBIDDEN_CHARS: OnceLock<[bool; 256]> = OnceLock::new();
        FORBIDDEN_CHARS.get_or_init(|| {
            let mut chars = [false; 256];
            chars[..=0x1F].fill(true);
            chars[usize::from(b'"')] = true;
            chars[usize::from(b'\\')] = true;
            chars
        })
    }

    /// Adds a chain of byte transitions from `start`, returning the end state.
    pub fn build_byte_string(
        fsm: &mut Fsm,
        bytes: &[i32],
        start: i32,
    ) -> i32 {
        let mut current = start;
        for &byte in bytes {
            let next = fsm.add_state();
            let byte = i32::from(byte as u8);
            fsm.add_edge(current, next, byte, byte);
            current = next;
        }
        current
    }

    /// Adds the transitions of a (possibly negated) character class payload
    /// `[is_negative, lower0, upper0, ...]` from `start` to `end`.
    pub fn add_character_class_transitions(
        fsm: &mut Fsm,
        data: &[i32],
        start: i32,
        end: i32,
    ) {
        let ranges = data[1..].chunks_exact(2).map(|range| (range[0] as u32, range[1] as u32));
        if data[0] == 0 {
            for (low, high) in ranges {
                add_codepoint_range(fsm, start, end, low, high);
            }
            return;
        }
        // Complement codepoint ranges before encoding them, so negated classes reject exactly
        // the listed codepoints.
        let mut excluded: Vec<(u32, u32)> = ranges.collect();
        excluded.sort_unstable();
        let mut next = 0;
        for (low, high) in excluded {
            if low > next {
                add_codepoint_range(fsm, start, end, next, (low - 1).min(MAX_CODEPOINT));
            }
            if high >= MAX_CODEPOINT {
                return;
            }
            next = next.max(high + 1);
        }
        if next <= MAX_CODEPOINT {
            add_codepoint_range(fsm, start, end, next, MAX_CODEPOINT);
        }
    }

    /// Adds a token tag-dispatch from `start`, returning its end states.
    pub fn build_token_tag_dispatch(
        fsm: &mut Fsm,
        token_tag_dispatch: &TokenTagDispatch,
        start: i32,
    ) -> Vec<i32> {
        let dispatch_states: Vec<i32> = token_tag_dispatch.trigger_rule_pairs.iter().map(|_| fsm.add_state()).collect();
        let mut end_states = vec![start];
        let end_state = if token_tag_dispatch.loop_after_dispatch {
            start
        } else {
            let state = fsm.add_state();
            end_states.push(state);
            state
        };
        let mut excluded_token_ids: Vec<i32> =
            token_tag_dispatch.trigger_rule_pairs.iter().map(|&(token_id, _)| token_id).collect();
        excluded_token_ids.extend_from_slice(&token_tag_dispatch.excludes);
        excluded_token_ids.sort_unstable();
        excluded_token_ids.dedup();
        for (&(token_id, rule_id), &dispatch_state) in
            token_tag_dispatch.trigger_rule_pairs.iter().zip(&dispatch_states)
        {
            fsm.add_token_edge(start, dispatch_state, &[token_id]);
            fsm.add_rule_edge(dispatch_state, end_state, rule_id);
        }
        fsm.add_exclude_token_edge(start, start, &excluded_token_ids);
        end_states
    }
}

/// Adds every canonically UTF-8 encoded codepoint in `[low, high]` from `from` to `to`, split at
/// the UTF-8 width boundaries so no overlong encodings are admitted.
fn add_codepoint_range(
    fsm: &mut Fsm,
    from: i32,
    to: i32,
    mut low: u32,
    high: u32,
) {
    for width_end in [0x7F, 0x7FF, 0xFFFF, MAX_CODEPOINT] {
        if low <= high && low <= width_end {
            let end = high.min(width_end);
            add_packed_utf8_range_edges(fsm, from, to, codepoint_to_packed_utf8(low), codepoint_to_packed_utf8(end));
            low = end + 1;
        }
    }
}
