//! The Earley parser — a port of `EarleyParser` in `cpp/earley_parser.cc`.
//!
//! The parser walks the per-rule FSMs of an optimized [`Grammar`]: every rule body is
//! compiled to an FSM (so every live [`ParserState`] has `rule_id >= 0`), and the parser
//! scans byte/token edges, predicts referenced rules, and completes finished rules in the
//! classic Earley three-phase loop. The IR-based path of the C++ (`rule_id == -1`) is dead
//! once a grammar is optimized and is therefore not ported.

use std::{collections::VecDeque, sync::Arc};

use super::{parser_state::ParserState, repeat_detector::RepeatDetector};
use crate::{fsm::FsmEdge, grammar::Grammar, support::Compact2dArray};

/// An incremental Earley parser over an optimized grammar's per-rule FSMs.
#[derive(Debug)]
pub struct EarleyParser {
    grammar: Arc<Grammar>,
    /// Whether the stop token can be accepted in this advance round.
    tmp_accept_stop_token: bool,
    /// `is_completed[i]` = whether the root rule is completed after accepting `i` inputs.
    is_completed: Vec<bool>,
    /// Per input position: `(referenced_rule_id, parent_state)` pairs awaiting completion.
    rule_id_to_completable_states: Compact2dArray<(i32, ParserState)>,
    /// Per input position: the scanable states after accepting that input.
    scanable_state_history: Compact2dArray<ParserState>,
    /// Scratch: states to add to the next scanable-history row.
    tmp_states_to_be_added: Vec<ParserState>,
    /// Scratch: the processing queue.
    tmp_process_state_queue: VecDeque<ParserState>,
    /// Scratch: states already enqueued this round.
    tmp_states_visited_in_queue: RepeatDetector,
    /// Whether the stop token has been accepted.
    stop_token_is_accepted: bool,
}

impl EarleyParser {
    /// Creates a parser over `grammar`, seeded with `initial_state` (or the root rule if it is
    /// invalid). When `need_expand` is false the state is only recorded, not expanded.
    ///
    /// # Panics
    /// Panics if the grammar has not been optimized.
    #[must_use]
    pub fn new(
        grammar: Arc<Grammar>,
        initial_state: ParserState,
        need_expand: bool,
    ) -> Self {
        assert!(
            grammar.is_optimized(),
            "the grammar is not optimized; optimize it before parsing"
        );
        let init = if initial_state.is_invalid() {
            ParserState::new(
                grammar.root_rule_id(),
                ParserState::UNEXPANDED_RULE_START_SEQUENCE_ID,
                0,
                ParserState::NO_PREV_INPUT_POS,
                0,
            )
        } else {
            initial_state
        };
        let mut parser = Self {
            grammar,
            tmp_accept_stop_token: false,
            is_completed: Vec::new(),
            rule_id_to_completable_states: Compact2dArray::new(),
            scanable_state_history: Compact2dArray::new(),
            tmp_states_to_be_added: Vec::new(),
            tmp_process_state_queue: VecDeque::new(),
            tmp_states_visited_in_queue: RepeatDetector::new(),
            stop_token_is_accepted: false,
        };
        if !need_expand {
            parser.rule_id_to_completable_states.push_row(&[]);
            parser.is_completed.push(false);
            parser.scanable_state_history.push_row(&[init]);
            return parser;
        }
        parser.push_state_and_expand(init);
        parser
    }

    /// Whether the root rule is currently completed (the stop token is acceptable).
    #[must_use]
    pub fn is_completed(&self) -> bool {
        *self
            .is_completed
            .last()
            .expect("parser always has at least one position")
    }

    /// Advances the parser by one input byte. Returns false (leaving state unchanged) if the
    /// byte is not accepted.
    pub fn advance(
        &mut self,
        ch: u8,
    ) -> bool {
        self.tmp_states_visited_in_queue.clear();
        self.tmp_states_to_be_added.clear();
        self.tmp_accept_stop_token = false;
        let latest: Vec<ParserState> =
            self.scanable_state_history.back().to_vec();
        for state in latest {
            self.advance_fsm(state, ch);
        }
        if self.tmp_process_state_queue.is_empty()
            && self.tmp_states_to_be_added.is_empty()
        {
            return false;
        }
        self.process_queue();
        true
    }

    /// Advances the parser by accepting a whole token via token/exclude-token edges. Returns
    /// false (state unchanged) if no state advances.
    pub fn advance_atomic_token(
        &mut self,
        token_id: i32,
    ) -> bool {
        self.tmp_states_visited_in_queue.clear();
        self.tmp_states_to_be_added.clear();
        self.tmp_accept_stop_token = false;
        let latest: Vec<ParserState> =
            self.scanable_state_history.back().to_vec();
        for state in latest {
            self.scan_atomic_token(state, token_id);
        }
        if self.tmp_process_state_queue.is_empty()
            && self.tmp_states_to_be_added.is_empty()
        {
            return false;
        }
        self.process_queue();
        true
    }

    /// Runs the predict/complete loop over the queue and records the resulting position.
    fn process_queue(&mut self) {
        self.rule_id_to_completable_states.push_row(&[]);
        while let Some(state) = self.tmp_process_state_queue.pop_front() {
            let (scanable, completable) = self.predict(state);
            if completable {
                self.complete(state);
            }
            if scanable {
                self.tmp_states_to_be_added.push(state);
            }
        }
        self.is_completed.push(self.tmp_accept_stop_token);
        let added = std::mem::take(&mut self.tmp_states_to_be_added);
        self.scanable_state_history.push_row(&added);
    }

    /// Removes the last `count` recorded positions.
    pub fn pop_last_states(
        &mut self,
        count: i32,
    ) {
        self.stop_token_is_accepted = false;
        let count = count as usize;
        assert!(
            count < self.rule_id_to_completable_states.len(),
            "cannot pop more states than exist"
        );
        self.rule_id_to_completable_states.pop_back(count);
        let new_len = self.is_completed.len() - count;
        self.is_completed.truncate(new_len);
        self.scanable_state_history.pop_back(count);
    }

    /// Pushes `state` and expands it through the predict/complete loop.
    pub fn push_state_and_expand(
        &mut self,
        state: ParserState,
    ) {
        self.tmp_states_visited_in_queue.clear();
        self.tmp_accept_stop_token = false;
        self.tmp_states_to_be_added.clear();
        if !self.expand_and_enqueue_unexpanded_state(state) {
            self.enqueue(state);
        }
        self.process_queue();
    }

    /// Resets the parser to the freshly-expanded root rule.
    pub fn reset(&mut self) {
        let rows = self.rule_id_to_completable_states.len();
        self.rule_id_to_completable_states.pop_back(rows);
        let hist = self.scanable_state_history.len();
        self.scanable_state_history.pop_back(hist);
        self.is_completed.clear();
        self.stop_token_is_accepted = false;
        self.push_state_and_expand(ParserState::new(
            self.grammar.root_rule_id(),
            ParserState::UNEXPANDED_RULE_START_SEQUENCE_ID,
            0,
            ParserState::NO_PREV_INPUT_POS,
            0,
        ));
    }

    /// The scanable states at the latest position.
    #[must_use]
    pub fn latest_scanable_states(&self) -> Vec<ParserState> {
        self.scanable_state_history.back().to_vec()
    }

    /// Appends one state as a new position to check (without expanding it).
    pub fn push_one_state_to_check(
        &mut self,
        state: ParserState,
    ) {
        self.rule_id_to_completable_states.push_row(&[]);
        let last = *self.is_completed.last().expect("non-empty");
        self.is_completed.push(last);
        self.scanable_state_history.push_row(&[state]);
    }

    /// The grammar being parsed.
    #[must_use]
    pub fn grammar(&self) -> &Grammar {
        &self.grammar
    }

    /// Whether the stop token has been accepted.
    #[must_use]
    pub fn is_stop_token_accepted(&self) -> bool {
        self.stop_token_is_accepted
    }

    /// Sets whether the stop token has been accepted (used by the matcher).
    pub fn set_stop_token_accepted(
        &mut self,
        value: bool,
    ) {
        self.stop_token_is_accepted = value;
    }

    fn enqueue(
        &mut self,
        state: ParserState,
    ) {
        if !self.tmp_states_visited_in_queue.is_visited(&state) {
            self.tmp_process_state_queue.push_back(state);
            self.tmp_states_visited_in_queue.insert(state);
        }
    }

    fn enqueue_without_processing(
        &mut self,
        state: ParserState,
    ) {
        if !self.tmp_states_visited_in_queue.is_visited(&state) {
            self.tmp_states_visited_in_queue.insert(state);
            self.tmp_states_to_be_added.push(state);
        }
    }

    fn expand_and_enqueue_unexpanded_state(
        &mut self,
        state: ParserState,
    ) -> bool {
        if state.sequence_id != ParserState::UNEXPANDED_RULE_START_SEQUENCE_ID {
            return false;
        }
        let body_id = self.grammar.rule(state.rule_id).body_expr_id;
        let start = self
            .grammar
            .per_rule_fsm(state.rule_id)
            .expect("optimized grammar has a per-rule FSM")
            .fsm()
            .start();
        self.enqueue(ParserState::new(
            state.rule_id,
            body_id,
            start,
            ParserState::NO_PREV_INPUT_POS,
            0,
        ));
        true
    }

    fn predict(
        &mut self,
        state: ParserState,
    ) -> (bool, bool) {
        self.expand_next_rule_ref_element_on_fsm(state);
        let fsm = self
            .grammar
            .per_rule_fsm(state.rule_id)
            .expect("per-rule FSM")
            .fsm();
        (
            fsm.is_scanable_state(state.element_id),
            fsm.is_end_state(state.element_id),
        )
    }

    #[allow(clippy::too_many_lines)]
    fn expand_next_rule_ref_element_on_fsm(
        &mut self,
        state: ParserState,
    ) {
        let grammar = Arc::clone(&self.grammar);
        let fsm = grammar.per_rule_fsm(state.rule_id).expect("per-rule FSM");
        let edges: Vec<FsmEdge> =
            fsm.fsm().fsm().state_edges(state.element_id).to_vec();
        for edge in edges {
            if edge.is_epsilon() {
                self.enqueue(ParserState::new(
                    state.rule_id,
                    state.sequence_id,
                    edge.target,
                    state.rule_start_pos,
                    0,
                ));
                continue;
            }

            let target = edge.target;
            let ref_rule_id;
            let is_repeat;
            if edge.is_rule_ref() {
                ref_rule_id = edge.ref_rule_id();
                is_repeat = false;
            } else if edge.is_repeat_ref() {
                is_repeat = true;
                let info =
                    grammar.complete_fsm().repeat_edge_info(edge.aux_index());
                ref_rule_id = info.rule_id();
                if state.repeat_count >= info.lower() {
                    self.enqueue(ParserState::with_repeat(
                        state.rule_id,
                        state.sequence_id,
                        target,
                        state.rule_start_pos,
                        0,
                        0,
                    ));
                }
                if state.repeat_count >= info.upper() {
                    continue;
                }
            } else {
                continue;
            }

            let mut right_recursion_to_root = false;
            let cur_pos = self.rule_id_to_completable_states.len() as i32 - 1;
            let target_is_leaf_end = !is_repeat
                && fsm.fsm().fsm().state_edges(target).is_empty()
                && fsm.fsm().is_end_state(target);
            if target_is_leaf_end && state.rule_start_pos != cur_pos {
                if state.rule_start_pos == ParserState::NO_PREV_INPUT_POS {
                    right_recursion_to_root = true;
                } else {
                    let back: Vec<(i32, ParserState)> =
                        self.rule_id_to_completable_states.back().to_vec();
                    let parents: Vec<(i32, ParserState)> = self
                        .rule_id_to_completable_states
                        .row(state.rule_start_pos as usize)
                        .to_vec();
                    let mut to_add: Vec<(i32, ParserState)> = Vec::new();
                    for (pid, parent_state) in &parents {
                        if *pid != state.rule_id {
                            continue;
                        }
                        let in_back = back.iter().any(|(rid, s)| {
                            *s == *parent_state && *rid == ref_rule_id
                        });
                        if !in_back {
                            to_add.push((ref_rule_id, *parent_state));
                        }
                    }
                    for item in to_add {
                        self.rule_id_to_completable_states
                            .push_in_latest_row(item);
                    }
                }
            } else if is_repeat {
                self.rule_id_to_completable_states.push_in_latest_row((
                    ref_rule_id,
                    ParserState::with_repeat(
                        state.rule_id,
                        state.sequence_id,
                        state.element_id,
                        state.rule_start_pos,
                        0,
                        state.repeat_count,
                    ),
                ));
            } else {
                self.rule_id_to_completable_states.push_in_latest_row((
                    ref_rule_id,
                    ParserState::new(
                        state.rule_id,
                        state.sequence_id,
                        target,
                        state.rule_start_pos,
                        0,
                    ),
                ));
            }

            if !is_repeat
                && grammar
                    .allow_empty_rule_ids()
                    .binary_search(&ref_rule_id)
                    .is_ok()
            {
                self.enqueue(ParserState::new(
                    state.rule_id,
                    state.sequence_id,
                    target,
                    state.rule_start_pos,
                    0,
                ));
            }

            let ref_body_id = grammar.rule(ref_rule_id).body_expr_id;
            let ref_start = grammar
                .per_rule_fsm(ref_rule_id)
                .expect("per-rule FSM")
                .fsm()
                .start();
            let new_start_pos = if right_recursion_to_root {
                ParserState::NO_PREV_INPUT_POS
            } else {
                self.rule_id_to_completable_states.len() as i32 - 1
            };
            self.enqueue(ParserState::new(
                ref_rule_id,
                ref_body_id,
                ref_start,
                new_start_pos,
                0,
            ));
        }
    }

    fn complete(
        &mut self,
        state: ParserState,
    ) {
        if state.rule_start_pos == ParserState::NO_PREV_INPUT_POS {
            self.tmp_accept_stop_token = true;
            return;
        }
        let grammar = Arc::clone(&self.grammar);
        let parents: Vec<(i32, ParserState)> = self
            .rule_id_to_completable_states
            .row(state.rule_start_pos as usize)
            .to_vec();
        for (ref_id, parent_state) in parents {
            if ref_id != state.rule_id {
                continue;
            }
            let parent_fsm = grammar
                .per_rule_fsm(parent_state.rule_id)
                .expect("per-rule FSM");
            let mut handled_as_repeat = false;
            for edge in
                parent_fsm.fsm().fsm().state_edges(parent_state.element_id)
            {
                if !edge.is_repeat_ref() {
                    continue;
                }
                let info =
                    grammar.complete_fsm().repeat_edge_info(edge.aux_index());
                if info.rule_id() != ref_id {
                    continue;
                }
                handled_as_repeat = true;
                let new_count = parent_state.repeat_count + 1;
                if new_count >= info.lower() {
                    self.enqueue(ParserState::with_repeat(
                        parent_state.rule_id,
                        parent_state.sequence_id,
                        edge.target,
                        parent_state.rule_start_pos,
                        0,
                        0,
                    ));
                }
                if new_count < info.upper() {
                    self.enqueue(ParserState::with_repeat(
                        parent_state.rule_id,
                        parent_state.sequence_id,
                        parent_state.element_id,
                        parent_state.rule_start_pos,
                        0,
                        new_count,
                    ));
                }
                break;
            }
            if !handled_as_repeat {
                self.enqueue(parent_state);
            }
        }
    }

    fn advance_fsm(
        &mut self,
        state: ParserState,
        ch: u8,
    ) {
        let grammar = Arc::clone(&self.grammar);
        let current =
            grammar.per_rule_fsm(state.rule_id).expect("per-rule FSM");
        let edges: Vec<FsmEdge> =
            current.fsm().fsm().state_edges(state.element_id).to_vec();
        let value = i32::from(ch);
        for edge in edges {
            if !edge.is_char_range() || value < edge.min || value > edge.max {
                continue;
            }
            let mut new_state = state;
            new_state.element_id = edge.target;
            self.enqueue_scan_target(current, new_state, edge.target);
        }
    }

    fn scan_atomic_token(
        &mut self,
        state: ParserState,
        token_id: i32,
    ) {
        let grammar = Arc::clone(&self.grammar);
        let current =
            grammar.per_rule_fsm(state.rule_id).expect("per-rule FSM");
        let edges: Vec<FsmEdge> =
            current.fsm().fsm().state_edges(state.element_id).to_vec();
        for edge in edges {
            let matched = if edge.is_token() {
                current
                    .fsm()
                    .fsm()
                    .token_edge_info(edge.aux_index())
                    .contains(token_id)
            } else if edge.is_exclude_token() {
                current
                    .fsm()
                    .fsm()
                    .exclude_token_edge_info(edge.aux_index())
                    .accepts(token_id)
            } else {
                false
            };
            if !matched {
                continue;
            }
            let mut new_state = state;
            new_state.element_id = edge.target;
            self.enqueue_scan_target(current, new_state, edge.target);
        }
    }

    /// Enqueues a post-scan `new_state`: scanable leaves skip the predict/complete queue.
    fn enqueue_scan_target(
        &mut self,
        current: &crate::fsm::CompactFsmWithStartEndWithSize,
        new_state: ParserState,
        target: i32,
    ) {
        let f = current.fsm();
        if !f.is_non_terminal_state(target)
            && !f.is_end_state(target)
            && f.is_scanable_state(target)
        {
            self.enqueue_without_processing(new_state);
        } else {
            self.enqueue(new_state);
        }
    }
}
