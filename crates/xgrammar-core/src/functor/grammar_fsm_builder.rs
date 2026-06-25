//! Builds finite-state machines from grammar expressions — a port of `GrammarFSMBuilder`
//! in `cpp/grammar_functor.cc`.
//!
//! Each rule body lowers into an [`FsmWithStartEnd`]: byte strings, character classes
//! (with full UTF-8 multibyte range encoding), rule references, tokens, repeats, and the
//! tag-dispatch automata. Sequences concatenate and choices union (then simplify).

use crate::{
    fsm::{
        CompactFsm, CompactFsmWithStartEnd, CompactFsmWithStartEndWithSize,
        Fsm, FsmWithStartEnd, TrieFsmBuilder,
    },
    grammar::{
        Grammar, GrammarExpr, GrammarExprType, TagDispatch, TokenTagDispatch,
    },
};

// Packed-UTF-8 range boundaries (a codepoint's UTF-8 bytes packed big-endian into a u32).
const MAX_1_BYTE: u32 = 0x7F;
const MIN_2_BYTES: u32 = 0xC080;
const MAX_2_BYTES: u32 = 0xDFBF;
const MIN_3_BYTES: u32 = 0x00E0_8080;
const MAX_3_BYTES: u32 = 0x00EF_BFBF;
const MIN_4_BYTES: u32 = 0xF080_8080;
const MAX_4_BYTES: u32 = 0xF7BF_BFBF;

/// Stateless builder of per-expression FSMs.
#[derive(Debug, Clone, Copy, Default)]
pub struct GrammarFsmBuilder;

impl GrammarFsmBuilder {
    /// Compiles every rule body into an FSM, folding them into one shared complete FSM stored
    /// on the grammar (the C++ `GrammarFSMBuilder::Apply`).
    ///
    /// # Panics
    /// Panics if any rule body fails to build an FSM (a malformed optimized grammar).
    pub fn apply(grammar: &mut Grammar) {
        let mut complete = Fsm::new(0);
        let num_rules = grammar.num_rules();
        let mut per_rule_meta: Vec<crate::fsm::FsmWithStartEndWithSize> =
            Vec::with_capacity(num_rules as usize);
        for i in 0..num_rules {
            let body_id = grammar.rule(i).body_expr_id;
            let body = grammar.expr(body_id);
            let rule_fsm = match body.ty {
                GrammarExprType::TagDispatch => {
                    Self::tag_dispatch(&grammar.tag_dispatch(body_id))
                },
                GrammarExprType::TokenTagDispatch => Self::token_tag_dispatch(
                    &grammar.token_tag_dispatch(body_id),
                ),
                _ => Self::choices(&body, grammar),
            }
            .expect("rule body must build an FSM after optimization");
            per_rule_meta.push(rule_fsm.add_to_complete_fsm(&mut complete));
        }

        let compact_complete = CompactFsm::from_fsm(&complete);
        let per_rule_fsms: Vec<Option<CompactFsmWithStartEndWithSize>> =
            per_rule_meta
                .iter()
                .map(|s| {
                    let wse = CompactFsmWithStartEnd::new(
                        compact_complete.clone(),
                        s.start(),
                        s.ends().to_vec(),
                        false,
                    );
                    Some(CompactFsmWithStartEndWithSize::new(
                        wse,
                        s.edge_num(),
                        s.node_num(),
                    ))
                })
                .collect();
        grammar.set_fsms(compact_complete, per_rule_fsms);
    }

    /// Builds the FSM for a `RuleRef` expression.
    #[must_use]
    pub fn rule_ref(expr: &GrammarExpr) -> FsmWithStartEnd {
        let mut fsm = empty_fsm();
        fsm.add_state();
        fsm.add_state();
        fsm.set_start_state(0);
        fsm.add_end_state(1);
        fsm.fsm_mut().add_rule_edge(0, 1, expr.data[0]);
        fsm
    }

    /// Builds the FSM for a `ByteString` expression (one transition per byte).
    #[must_use]
    pub fn byte_string(expr: &GrammarExpr) -> FsmWithStartEnd {
        debug_assert_eq!(expr.ty, GrammarExprType::ByteString);
        let mut fsm = empty_fsm();
        let mut current = fsm.add_state();
        fsm.set_start_state(current);
        for &byte in expr.data {
            let next = fsm.add_state();
            let b = i32::from(byte as u8);
            fsm.fsm_mut().add_edge(current, next, b, b);
            current = next;
        }
        fsm.add_end_state(current);
        fsm
    }

    /// Builds the FSM for a `CharacterClass` / `CharacterClassStar` expression.
    #[must_use]
    pub fn character_class(expr: &GrammarExpr) -> FsmWithStartEnd {
        let is_negative = expr.data[0] != 0;
        if is_negative {
            return Self::build_negative_character_class(expr);
        }
        let mut fsm = empty_fsm();
        let start = fsm.add_state();
        fsm.set_start_state(start);
        let is_star = expr.ty == GrammarExprType::CharacterClassStar;
        let end = if is_star {
            start
        } else {
            fsm.add_state()
        };
        fsm.add_end_state(end);
        let mut i = 1;
        while i < expr.data.len() {
            let codepoint_min = expr.data[i] as u32;
            let codepoint_max = expr.data[i + 1] as u32;
            let packed_min = codepoint_to_packed_utf8(codepoint_min);
            let packed_max = codepoint_to_packed_utf8(codepoint_max);
            add_character_range(&mut fsm, start, end, packed_min, packed_max);
            i += 2;
        }
        fsm
    }

    /// Builds the FSM for a `Repeat` expression (a single repeat-reference edge).
    #[must_use]
    pub fn repeat(expr: &GrammarExpr) -> FsmWithStartEnd {
        let (rule_id, lower, upper) =
            (expr.data[0], expr.data[1], expr.data[2]);
        let mut fsm = empty_fsm();
        fsm.add_state();
        fsm.add_state();
        fsm.set_start_state(0);
        fsm.add_end_state(1);
        fsm.fsm_mut().add_repeat_edge(0, 1, rule_id, lower, upper);
        fsm
    }

    /// Builds the FSM for a `Token` expression (a single token-set edge).
    #[must_use]
    pub fn token(expr: &GrammarExpr) -> FsmWithStartEnd {
        debug_assert_eq!(expr.ty, GrammarExprType::Token);
        let mut fsm = Fsm::new(2);
        fsm.add_token_edge(0, 1, expr.data);
        FsmWithStartEnd::new(fsm, 0, vec![false, true], false)
    }

    /// Builds the FSM for an `ExcludeToken` expression (a single exclude-token edge).
    #[must_use]
    pub fn exclude_token(expr: &GrammarExpr) -> FsmWithStartEnd {
        debug_assert_eq!(expr.ty, GrammarExprType::ExcludeToken);
        let mut fsm = Fsm::new(2);
        fsm.add_exclude_token_edge(0, 1, expr.data);
        FsmWithStartEnd::new(fsm, 0, vec![false, true], false)
    }

    /// Builds the dispatch FSM for a token tag-dispatch.
    #[must_use]
    pub fn token_tag_dispatch(
        ttd: &TokenTagDispatch
    ) -> Option<FsmWithStartEnd> {
        let num_triggers = ttd.trigger_rule_pairs.len() as i32;
        let loop_after = ttd.loop_after_dispatch;
        let num_states = 1 + num_triggers + i32::from(!loop_after);
        let mut fsm = Fsm::new(num_states as usize);
        let mut ends = vec![false; num_states as usize];
        let start = 0;
        ends[start as usize] = true;
        let end_state = if loop_after {
            -1
        } else {
            let e = num_states - 1;
            ends[e as usize] = true;
            e
        };

        let mut self_loop_exclude: Vec<i32> = ttd
            .trigger_rule_pairs
            .iter()
            .map(|&(token_id, _)| token_id)
            .collect();
        self_loop_exclude.extend_from_slice(&ttd.excludes);
        self_loop_exclude.sort_unstable();
        self_loop_exclude.dedup();

        for (i, &(token_id, rule_id)) in
            ttd.trigger_rule_pairs.iter().enumerate()
        {
            let dispatch_state = 1 + i as i32;
            fsm.add_token_edge(start, dispatch_state, &[token_id]);
            let target = if loop_after {
                start
            } else {
                end_state
            };
            fsm.add_rule_edge(dispatch_state, target, rule_id);
        }
        fsm.add_exclude_token_edge(start, start, &self_loop_exclude);
        Some(FsmWithStartEnd::new(fsm, start, ends, false))
    }

    /// Builds the FSM for a `Sequence` expression (concatenation of its elements).
    #[must_use]
    pub fn sequence(
        expr: &GrammarExpr,
        grammar: &Grammar,
    ) -> Option<FsmWithStartEnd> {
        let mut fsm_list = Vec::with_capacity(expr.data.len());
        for &seq_id in expr.data {
            let seq_expr = grammar.expr(seq_id);
            let sub = match seq_expr.ty {
                GrammarExprType::ByteString => Self::byte_string(&seq_expr),
                GrammarExprType::RuleRef => Self::rule_ref(&seq_expr),
                GrammarExprType::CharacterClass
                | GrammarExprType::CharacterClassStar => {
                    Self::character_class(&seq_expr)
                },
                GrammarExprType::Repeat => Self::repeat(&seq_expr),
                GrammarExprType::Token => Self::token(&seq_expr),
                GrammarExprType::ExcludeToken => Self::exclude_token(&seq_expr),
                _ => return None,
            };
            fsm_list.push(sub);
        }
        if fsm_list.is_empty() {
            return Some(single_state_fsm());
        }
        Some(FsmWithStartEnd::concat(&fsm_list))
    }

    /// Builds the FSM for a `Choices` expression (union of its sequences, then simplified).
    #[must_use]
    pub fn choices(
        expr: &GrammarExpr,
        grammar: &Grammar,
    ) -> Option<FsmWithStartEnd> {
        debug_assert_eq!(expr.ty, GrammarExprType::Choices);
        let mut fsm_list = Vec::new();
        let mut nullable = false;
        for &choice_id in expr.data {
            let choice_expr = grammar.expr(choice_id);
            if choice_expr.ty == GrammarExprType::EmptyStr {
                nullable = true;
                continue;
            }
            debug_assert_eq!(choice_expr.ty, GrammarExprType::Sequence);
            fsm_list.push(Self::sequence(&choice_expr, grammar)?);
        }
        if fsm_list.is_empty() {
            return Some(single_state_fsm());
        }
        if nullable {
            fsm_list.push(single_state_fsm());
        }
        let result = FsmWithStartEnd::union(&fsm_list);
        let result = result.simplify_epsilon();
        Some(result.merge_equivalent_states())
    }

    /// Builds the dispatch FSM for a string tag-dispatch.
    #[must_use]
    pub fn tag_dispatch(tag_dispatch: &TagDispatch) -> Option<FsmWithStartEnd> {
        Self::build_tag_dispatch(
            &tag_dispatch.tag_rule_pairs,
            tag_dispatch.loop_after_dispatch,
            &tag_dispatch.excludes,
        )
    }

    fn build_tag_dispatch(
        string_trigger_rules: &[(Vec<u8>, i32)],
        loop_after_dispatch: bool,
        excluded_strings: &[Vec<u8>],
    ) -> Option<FsmWithStartEnd> {
        let tag_names: Vec<&[u8]> = string_trigger_rules
            .iter()
            .map(|(name, _)| name.as_slice())
            .collect();
        let excluded: Vec<&[u8]> =
            excluded_strings.iter().map(Vec::as_slice).collect();
        let mut end_states: Vec<i32> = Vec::new();
        let trie = TrieFsmBuilder::build(
            &tag_names,
            &excluded,
            Some(&mut end_states),
            true,
            true,
        )?;
        let mut trie_fsm = trie.fsm().clone();
        let start = trie.start();

        // The final end states are all states except the trie's terminal (trigger) states.
        let mut ends = vec![false; trie_fsm.num_states() as usize];
        for (i, slot) in ends.iter_mut().enumerate() {
            *slot = !trie.is_end_state(i as i32);
        }

        // Add a rule-ref edge out of each trigger's terminal state.
        for (i, (_, rule_id)) in string_trigger_rules.iter().enumerate() {
            let next_state = if loop_after_dispatch {
                start
            } else {
                let s = trie_fsm.add_state();
                ends.push(true);
                s
            };
            trie_fsm.add_rule_edge(end_states[i], next_state, *rule_id);
        }
        Some(FsmWithStartEnd::new(trie_fsm, start, ends, false))
    }

    fn build_negative_character_class(expr: &GrammarExpr) -> FsmWithStartEnd {
        debug_assert!(matches!(
            expr.ty,
            GrammarExprType::CharacterClass
                | GrammarExprType::CharacterClassStar
        ));
        debug_assert!(expr.data[0] != 0);
        let mut char_set = [false; 128];
        let mut i = 1;
        while i < expr.data.len() {
            let byte_min = expr.data[i] as u8;
            let mut byte_max = expr.data[i + 1] as u8;
            if byte_max > 128 {
                byte_max = 127;
            }
            for j in byte_min..=byte_max {
                char_set[j as usize] = true;
            }
            i += 2;
        }

        let mut fsm = empty_fsm();
        let start = fsm.add_state();
        fsm.set_start_state(start);
        let is_star = expr.ty == GrammarExprType::CharacterClassStar;
        let end = if is_star {
            start
        } else {
            fsm.add_state()
        };
        fsm.add_end_state(end);
        let mut i = 0;
        while i < 128 {
            if !char_set[i] {
                let left = i;
                let mut right = i + 1;
                while right < 128 && !char_set[right] {
                    right += 1;
                }
                fsm.fsm_mut().add_edge(
                    start,
                    end,
                    left as i32,
                    (right - 1) as i32,
                );
                i = right;
            } else {
                i += 1;
            }
        }
        add_character_range(&mut fsm, start, end, MIN_2_BYTES, MAX_4_BYTES);
        fsm
    }
}

/// An empty FSM with no states (start 0, no ends) — the C++ default `FSMWithStartEnd`.
fn empty_fsm() -> FsmWithStartEnd {
    FsmWithStartEnd::new(Fsm::new(0), 0, Vec::new(), false)
}

/// A one-state FSM whose single state is both start and accepting (the empty language `""`).
fn single_state_fsm() -> FsmWithStartEnd {
    let mut fsm = empty_fsm();
    fsm.add_state();
    fsm.set_start_state(0);
    fsm.add_end_state(0);
    fsm
}

/// Converts a Unicode codepoint to its UTF-8 bytes packed big-endian into a `u32`.
fn codepoint_to_packed_utf8(codepoint: u32) -> u32 {
    if codepoint <= 0x7F {
        codepoint
    } else if codepoint <= 0x7FF {
        let byte0 = 0xC0 | ((codepoint >> 6) & 0x1F);
        let byte1 = 0x80 | (codepoint & 0x3F);
        (byte0 << 8) | byte1
    } else if codepoint <= 0xFFFF {
        let byte0 = 0xE0 | ((codepoint >> 12) & 0x0F);
        let byte1 = 0x80 | ((codepoint >> 6) & 0x3F);
        let byte2 = 0x80 | (codepoint & 0x3F);
        (byte0 << 16) | (byte1 << 8) | byte2
    } else {
        let byte0 = 0xF0 | ((codepoint >> 18) & 0x07);
        let byte1 = 0x80 | ((codepoint >> 12) & 0x3F);
        let byte2 = 0x80 | ((codepoint >> 6) & 0x3F);
        let byte3 = 0x80 | (codepoint & 0x3F);
        (byte0 << 24) | (byte1 << 16) | (byte2 << 8) | byte3
    }
}

/// The four bytes of a packed-UTF-8 value, little end (`byte[0]` = least significant) first.
fn packed_bytes(v: u32) -> [i32; 4] {
    [
        (v & 0xFF) as i32,
        ((v >> 8) & 0xFF) as i32,
        ((v >> 16) & 0xFF) as i32,
        ((v >> 24) & 0xFF) as i32,
    ]
}

/// Adds a range `[min, max]` of equal-byte-length packed-UTF-8 characters between `from` and
/// `to`, allocating intermediate states for the continuation bytes.
fn add_same_length_character_range(
    fsm: &mut FsmWithStartEnd,
    from: i32,
    to: i32,
    mut min: u32,
    mut max: u32,
) {
    let mut byte_min = packed_bytes(min);
    let mut byte_max = packed_bytes(max);

    // ASCII (single byte).
    if byte_max[1] == 0 {
        fsm.fsm_mut().add_edge(from, to, byte_min[0], byte_max[0]);
        return;
    }

    if byte_max[3] != 0 {
        // 4-byte sequence.
        if byte_max[3] == byte_min[3] {
            let tmp = fsm.add_state();
            fsm.fsm_mut().add_edge(from, tmp, byte_min[3], byte_max[3]);
            min &= 0x00FF_FFFF;
            max &= 0x00FF_FFFF;
            add_same_length_character_range(fsm, tmp, to, min, max);
            return;
        }
        if (min & 0x00FF_FFFF) != 0x0080_8080 {
            let tmp_min = fsm.add_state();
            fsm.fsm_mut().add_edge(from, tmp_min, byte_min[3], byte_min[3]);
            add_same_length_character_range(
                fsm,
                tmp_min,
                to,
                min & 0x00FF_FFFF,
                0x00BF_BFBF,
            );
        } else {
            byte_min[3] -= 1;
        }
        if (max & 0x00FF_FFFF) != 0x00BF_BFBF {
            let tmp_max = fsm.add_state();
            fsm.fsm_mut().add_edge(from, tmp_max, byte_max[3], byte_max[3]);
            add_same_length_character_range(
                fsm,
                tmp_max,
                to,
                0x0080_8080,
                max & 0x00FF_FFFF,
            );
        } else {
            byte_max[3] += 1;
        }
        if byte_max[3] - byte_min[3] > 1 {
            let mid = fsm.add_state();
            fsm.fsm_mut().add_edge(from, mid, byte_min[3] + 1, byte_max[3] - 1);
            let mid2 = fsm.add_state();
            fsm.fsm_mut().add_edge(mid, mid2, 0x80, 0xBF);
            let mid3 = fsm.add_state();
            fsm.fsm_mut().add_edge(mid2, mid3, 0x80, 0xBF);
            fsm.fsm_mut().add_edge(mid3, to, 0x80, 0xBF);
        }
        return;
    }

    if byte_max[2] != 0 {
        // 3-byte sequence.
        if byte_max[2] == byte_min[2] {
            let tmp = fsm.add_state();
            fsm.fsm_mut().add_edge(from, tmp, byte_min[2], byte_max[2]);
            min &= 0x00FFFF;
            max &= 0x00FFFF;
            add_same_length_character_range(fsm, tmp, to, min, max);
            return;
        }
        if (min & 0x00FFFF) != 0x8080 {
            let tmp_min = fsm.add_state();
            fsm.fsm_mut().add_edge(from, tmp_min, byte_min[2], byte_min[2]);
            add_same_length_character_range(
                fsm,
                tmp_min,
                to,
                min & 0x00FFFF,
                0x00BFBF,
            );
        } else {
            byte_min[2] -= 1;
        }
        if (max & 0x00FFFF) != 0xBFBF {
            let tmp_max = fsm.add_state();
            fsm.fsm_mut().add_edge(from, tmp_max, byte_max[2], byte_max[2]);
            add_same_length_character_range(
                fsm,
                tmp_max,
                to,
                0x0080,
                max & 0x00FFFF,
            );
        } else {
            byte_max[2] += 1;
        }
        if byte_max[2] - byte_min[2] > 1 {
            let mid = fsm.add_state();
            fsm.fsm_mut().add_edge(from, mid, byte_min[2] + 1, byte_max[2] - 1);
            let mid2 = fsm.add_state();
            fsm.fsm_mut().add_edge(mid, mid2, 0x80, 0xBF);
            fsm.fsm_mut().add_edge(mid2, to, 0x80, 0xBF);
        }
        return;
    }

    // 2-byte sequence.
    if byte_max[1] == byte_min[1] {
        let tmp = fsm.add_state();
        fsm.fsm_mut().add_edge(from, tmp, byte_min[1], byte_max[1]);
        min &= 0x00FF;
        max &= 0x00FF;
        add_same_length_character_range(fsm, tmp, to, min, max);
        return;
    }
    if (min & 0x00FF) != 0x80 {
        let tmp_min = fsm.add_state();
        fsm.fsm_mut().add_edge(from, tmp_min, byte_min[1], byte_min[1]);
        add_same_length_character_range(fsm, tmp_min, to, min & 0x00FF, 0x00BF);
    } else {
        byte_min[1] -= 1;
    }
    if (max & 0x00FF) != 0xBF {
        let tmp_max = fsm.add_state();
        fsm.fsm_mut().add_edge(from, tmp_max, byte_max[1], byte_max[1]);
        add_same_length_character_range(fsm, tmp_max, to, 0x0080, max & 0x00FF);
    } else {
        byte_max[1] += 1;
    }
    if byte_max[1] - byte_min[1] > 1 {
        let mid = fsm.add_state();
        fsm.fsm_mut().add_edge(from, mid, byte_min[1] + 1, byte_max[1] - 1);
        fsm.fsm_mut().add_edge(mid, to, 0x80, 0xBF);
    }
}

/// Adds a range `[min, max]` of packed-UTF-8 characters, clamping to valid UTF-8 and
/// splitting across byte-length boundaries.
fn add_character_range(
    fsm: &mut FsmWithStartEnd,
    from: i32,
    to: i32,
    mut min: u32,
    mut max: u32,
) {
    debug_assert!(min <= max, "invalid character range: min > max");
    // Clamp max to a valid packed-UTF-8 value.
    if max > MAX_4_BYTES {
        max = MAX_4_BYTES;
    } else if max > MAX_3_BYTES {
        if max < MIN_4_BYTES {
            max = MAX_3_BYTES;
        }
    } else if max > MAX_2_BYTES {
        if max < MIN_3_BYTES {
            max = MAX_2_BYTES;
        }
    } else if max < MIN_2_BYTES && max > MAX_1_BYTE {
        max = MAX_1_BYTE;
    }

    if min > MAX_4_BYTES {
        min = MAX_4_BYTES;
    } else if min > MAX_3_BYTES {
        if min < MIN_4_BYTES {
            min = MIN_4_BYTES;
        }
    } else if min > MAX_2_BYTES {
        if min < MIN_3_BYTES {
            min = MIN_3_BYTES;
        }
    } else if min < MIN_2_BYTES && min > MAX_1_BYTE {
        min = MIN_2_BYTES;
    }

    // Divide the range into same-byte-length sub-ranges.
    if max <= MAX_1_BYTE {
        add_same_length_character_range(fsm, from, to, min, max);
    } else if max <= MAX_2_BYTES {
        if min >= MIN_2_BYTES {
            add_same_length_character_range(fsm, from, to, min, max);
        } else {
            add_same_length_character_range(fsm, from, to, min, MAX_1_BYTE);
            add_same_length_character_range(fsm, from, to, MIN_2_BYTES, max);
        }
    } else if max <= MAX_3_BYTES {
        if min >= MIN_3_BYTES {
            add_same_length_character_range(fsm, from, to, min, max);
        } else if min >= MIN_2_BYTES {
            add_same_length_character_range(fsm, from, to, min, MAX_2_BYTES);
            add_same_length_character_range(fsm, from, to, MIN_3_BYTES, max);
        } else {
            add_same_length_character_range(fsm, from, to, min, MAX_1_BYTE);
            add_same_length_character_range(
                fsm,
                from,
                to,
                MIN_2_BYTES,
                MAX_2_BYTES,
            );
            add_same_length_character_range(fsm, from, to, MIN_3_BYTES, max);
        }
    } else {
        debug_assert!(max <= MAX_4_BYTES);
        if min >= MIN_4_BYTES {
            add_same_length_character_range(fsm, from, to, min, max);
        } else if min >= MIN_3_BYTES {
            add_same_length_character_range(fsm, from, to, min, MAX_3_BYTES);
            add_same_length_character_range(fsm, from, to, MIN_4_BYTES, max);
        } else if min >= MIN_2_BYTES {
            add_same_length_character_range(fsm, from, to, min, MAX_2_BYTES);
            add_same_length_character_range(
                fsm,
                from,
                to,
                MIN_3_BYTES,
                MAX_3_BYTES,
            );
            add_same_length_character_range(fsm, from, to, MIN_4_BYTES, max);
        } else {
            add_same_length_character_range(fsm, from, to, min, MAX_1_BYTE);
            add_same_length_character_range(
                fsm,
                from,
                to,
                MIN_2_BYTES,
                MAX_2_BYTES,
            );
            add_same_length_character_range(
                fsm,
                from,
                to,
                MIN_3_BYTES,
                MAX_3_BYTES,
            );
            add_same_length_character_range(fsm, from, to, MIN_4_BYTES, max);
        }
    }
}
