//! The Earley parser.
//!
//! The parser walks the per-rule FSMs of an optimized [`Grammar`]: most live
//! [`ParserState`] values have `rule_id >= 0` and scan FSM byte/token edges. Lookahead
//! assertions use the IR path (`rule_id == -1`) with `sequence_id` pointing at the
//! assertion expression; the parser predicts, scans, and completes those states over the
//! grammar expression tree in the classic Earley three-phase loop.
//!
//! The matcher and the token-mask cache builder extend the parser (they derive from it in the
//! C++), so the fields they drive directly are public.

use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};

use smallvec::SmallVec;

use super::{
    capture_event::CaptureEvent, capture_occurrence::CaptureOccurrence, parser_row::ParserRow,
    parser_state::ParserState, repeat_detector::RepeatDetector,
};
use crate::{
    fsm::FsmEdge,
    grammar::{Grammar, GrammarExprType},
    support::{Compact2dArray, handle_utf8_first_byte},
};

const FSM_STATE_INITIALIZED: u8 = 1 << 0;
const FSM_STATE_SCANABLE: u8 = 1 << 1;
const FSM_STATE_NON_TERMINAL: u8 = 1 << 2;
const FSM_STATE_END: u8 = 1 << 3;
const FSM_STATE_HAS_EDGES: u8 = 1 << 4;

/// An incremental Earley parser over an optimized grammar's per-rule FSMs.
#[derive(Debug, Clone)]
pub struct EarleyParser {
    /// The optimized grammar being parsed.
    pub grammar: Arc<Grammar>,
    /// Whether the stop token can be accepted in this advance round.
    pub tmp_accept_stop_token: bool,
    /// `is_completed[i]` = whether the root rule is completed after accepting `i` inputs.
    pub is_completed: Vec<bool>,
    /// Per input position: `(referenced_rule_id, parent_state)` pairs awaiting completion.
    pub rule_id_to_completable_states: Compact2dArray<(i32, ParserState)>,
    /// Per input position: the scanable states after accepting that input.
    pub scanable_state_history: Compact2dArray<ParserState>,
    /// Scratch: states to add to the next scanable-history row.
    pub tmp_states_to_be_added: Vec<ParserState>,
    /// Scratch: the processing queue.
    pub tmp_process_state_queue: VecDeque<ParserState>,
    /// Scratch: states already enqueued this round.
    pub tmp_states_visited_in_queue: RepeatDetector,
    /// Whether the stop token has been accepted.
    pub stop_token_is_accepted: bool,
    /// Lazily computed FSM state properties, indexed by rule id and state id.
    fsm_state_flags_cache: Vec<u8>,
    /// Whether each rule can match the empty string.
    rule_is_nullable: Vec<bool>,
    /// The index of the LLM token currently being accepted, set by the matcher; `-1` before
    /// any token. `budget_deadline` values are compared against it.
    pub current_token_index: i32,
    /// Whether states past their budget deadline are skipped when scanning.
    pub skip_expired_states: bool,
    /// Whether any rule of the grammar has a token budget.
    pub has_budget_rules: bool,
    /// The number of Unicode codepoints accepted at every parser history row.
    pub char_count_history: Vec<i32>,
    /// Whether any rule of the grammar has a character budget.
    pub has_char_budget_rules: bool,
    /// Whether a character-budgeted occurrence was entered since the initial parser row.
    pub char_budget_entry_history: Vec<bool>,
    /// Entry-history value for the row currently being expanded.
    tmp_char_budget_entered: bool,
    /// Whether any rule has a capture or stop_capture name. When false the capture machinery is
    /// disabled.
    pub capture_tracking: bool,
    /// Whether the grammar contains suffix/stop spans that may affect captures.
    has_hidden_capture_rules: bool,
    /// Whether capture events are recorded in `complete`. Only enabled during definitive
    /// advances, never during speculative exploration.
    pub capture_recording: bool,
    /// `capture_event_history[i]` stores the events recorded when input position `i` was
    /// created. Kept aligned with `scanable_state_history` whenever `capture_tracking` is true.
    pub capture_event_history: Compact2dArray<CaptureEvent>,
    /// The lazy rule occurrences `(rule_id, rule_start_pos)` completed while building the
    /// current row; their remaining states are removed when the row is finalized.
    pub tmp_completed_lazy_occurrences: Vec<(i32, i32)>,
}

impl EarleyParser {
    /// Creates a parser over `grammar`, starting from `initial_state` or, if `None`, from the
    /// root rule.
    ///
    /// # Panics
    /// Panics if the grammar has not been optimized.
    #[must_use]
    pub fn new(
        grammar: Arc<Grammar>,
        initial_state: Option<ParserState>,
    ) -> Self {
        assert!(
            grammar.is_optimized(),
            "The grammar is not optimized. Please optimize the grammar before using the Earley parser."
        );
        let num_rules = grammar.num_rules() as usize;
        let has_budget_rules = grammar.rules().iter().any(|rule| rule.max_tokens >= 0);
        let has_char_budget_rules = grammar.rules().iter().any(|rule| rule.max_chars >= 0);
        let capture_tracking = grammar.rules().iter().any(|rule| !rule.capture_name.is_empty())
            || grammar.suffix_stop_infos().iter().any(|info| !info.stop_capture_name.is_empty());
        let has_hidden_capture_rules =
            grammar.suffix_stop_infos().iter().any(|info| info.hidden_suffix_bytes > 0 || info.hidden_stop_bytes > 0);
        let mut rule_is_nullable = vec![false; num_rules];
        for &rule_id in grammar.allow_empty_rule_ids() {
            rule_is_nullable[rule_id as usize] = true;
        }
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
            fsm_state_flags_cache: Vec::new(),
            rule_is_nullable,
            current_token_index: -1,
            skip_expired_states: false,
            has_budget_rules,
            char_count_history: Vec::new(),
            has_char_budget_rules,
            char_budget_entry_history: Vec::new(),
            tmp_char_budget_entered: false,
            capture_tracking,
            has_hidden_capture_rules,
            capture_recording: false,
            capture_event_history: Compact2dArray::new(),
            tmp_completed_lazy_occurrences: Vec::new(),
        };
        let initial_state = initial_state.unwrap_or_else(|| parser.root_initial_state());
        parser.push_state_and_expand(initial_state);
        parser
    }

    /// Whether the root rule is currently completed (the stop token is acceptable).
    #[must_use]
    pub fn is_completed(&self) -> bool {
        *self.is_completed.last().expect("parser always has at least one position")
    }

    /// The scanable states at the latest position.
    #[must_use]
    pub fn latest_scanable_states(&self) -> Vec<ParserState> {
        self.scanable_state_history.back().to_vec()
    }

    /// The `(referenced_rule_id, parent_state)` completable pairs at the latest position.
    #[must_use]
    pub fn latest_completable_states(&self) -> Vec<(i32, ParserState)> {
        self.rule_id_to_completable_states.back().to_vec()
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

    /// Whether completing the occurrence of a suffix/stop rule actually consumed its trailing
    /// marker. A non-looping tag dispatch can also complete before its trigger is seen; only the
    /// terminal post-dispatch state has no outgoing edges.
    fn completion_consumed_marker(
        &self,
        state: &ParserState,
    ) -> bool {
        let body = self.grammar.expr(self.grammar.rule(state.rule_id).body_expr_id);
        if body.ty != GrammarExprType::TagDispatch {
            return true;
        }
        let fsm = self.grammar.per_rule_fsm(state.rule_id).expect("per-rule FSM").fsm().fsm();
        fsm.state_edges(state.element_id).is_empty()
    }

    /// Collects the captured rule occurrences whose concrete Earley parent chains contain the
    /// given stop completion, following only the parent links of this occurrence.
    fn collect_stop_capture_targets(
        &self,
        state: &ParserState,
    ) -> Vec<CaptureOccurrence> {
        let mut targets = Vec::new();
        let mut pending = vec![CaptureOccurrence {
            rule_id: state.rule_id,
            start_pos: state.rule_start_pos,
        }];
        let mut visited = HashSet::new();
        while let Some(occurrence) = pending.pop() {
            if !visited.insert(occurrence) {
                continue;
            }
            if self.rule_has_capture(occurrence.rule_id) {
                targets.push(occurrence);
            }
            if occurrence.start_pos == ParserState::NO_PREV_INPUT_POS {
                continue;
            }
            for &(ref_rule_id, parent_state) in self.rule_id_to_completable_states.row(occurrence.start_pos as usize) {
                if ref_rule_id != occurrence.rule_id || parent_state.rule_id < 0 {
                    continue;
                }
                pending.push(CaptureOccurrence {
                    rule_id: parent_state.rule_id,
                    start_pos: parent_state.rule_start_pos,
                });
            }
        }
        targets
    }

    /// Records a capture or hidden-span event for a completed rule in the current row.
    fn record_capture_event(
        &mut self,
        state: &ParserState,
        marker_present: bool,
    ) {
        let info = self.grammar.suffix_stop_info(state.rule_id).cloned();
        let marker_consumed = marker_present
            && info.as_ref().is_some_and(|info| info.hidden_suffix_bytes > 0 || info.hidden_stop_bytes > 0)
            && self.completion_consumed_marker(state);
        let (hidden_suffix_bytes, hidden_stop_bytes, body_rule_id) = match (&info, marker_consumed) {
            (Some(info), true) => (info.hidden_suffix_bytes, info.hidden_stop_bytes, info.body_rule_id),
            _ => (0, 0, -1),
        };
        let mut event_start_pos = state.rule_start_pos;
        if marker_consumed && body_rule_id == state.rule_id {
            // A self-referencing body helper marks the zero-width event inserted immediately after
            // a dynamic string trigger. Its capture span is the fixed-length marker before it.
            debug_assert!(event_start_pos != ParserState::NO_PREV_INPUT_POS);
            event_start_pos -= hidden_suffix_bytes.max(hidden_stop_bytes);
            debug_assert!(event_start_pos >= 0);
        }
        let stop_capture_targets = if hidden_stop_bytes > 0 {
            self.collect_stop_capture_targets(state)
        } else {
            Vec::new()
        };
        self.capture_event_history.push_in_latest_row(CaptureEvent {
            rule_id: state.rule_id,
            start_pos: event_start_pos,
            occurrence_start_pos: state.rule_start_pos,
            hidden_suffix_bytes,
            hidden_stop_bytes,
            stop_capture_targets,
        });
    }

    fn resolve_active_temperature_rule(
        &self,
        rule_id: i32,
        inherited_rule_id: i32,
    ) -> i32 {
        if self.grammar.rule(rule_id).temperature.is_some() {
            rule_id
        } else {
            inherited_rule_id
        }
    }

    fn rule_has_capture(
        &self,
        rule_id: i32,
    ) -> bool {
        self.capture_tracking && rule_id >= 0 && !self.grammar.rule(rule_id).capture_name.is_empty()
    }

    fn rule_has_hidden_bytes(
        &self,
        rule_id: i32,
    ) -> bool {
        if !self.capture_tracking || !self.has_hidden_capture_rules || rule_id < 0 {
            return false;
        }
        self.grammar
            .suffix_stop_info(rule_id)
            .is_some_and(|info| info.hidden_suffix_bytes > 0 || info.hidden_stop_bytes > 0)
    }

    fn rule_needs_capture_event(
        &self,
        rule_id: i32,
    ) -> bool {
        self.rule_has_capture(rule_id) || self.rule_has_hidden_bytes(rule_id)
    }

    /// Whether the state's derivation may not consume the next token.
    #[must_use]
    pub fn is_expired_state(
        &self,
        state: &ParserState,
    ) -> bool {
        state.budget_deadline >= 0 && self.current_token_index > state.budget_deadline
    }

    /// Whether the state's derivation may not consume another Unicode codepoint.
    #[must_use]
    pub fn is_char_expired_state(
        &self,
        state: &ParserState,
    ) -> bool {
        state.char_budget_deadline >= 0 && self.current_char_index() >= state.char_budget_deadline
    }

    /// The deadline for a newly predicted occurrence of the rule: its own budget counted from
    /// the current token, capped by the parent's deadline for nested budgets.
    fn deadline_for_rule(
        &self,
        rule_id: i32,
        parent_deadline: i32,
    ) -> i32 {
        let own = self.grammar.rule(rule_id).max_tokens;
        if own < 0 {
            return parent_deadline;
        }
        let deadline = (i64::from(self.current_token_index) + i64::from(own)).min(i64::from(i32::MAX)) as i32;
        if parent_deadline >= 0 {
            deadline.min(parent_deadline)
        } else {
            deadline
        }
    }

    /// The character deadline for a newly predicted rule occurrence.
    fn char_deadline_for_rule(
        &self,
        rule_id: i32,
        parent_deadline: i32,
    ) -> i32 {
        let own = self.grammar.rule(rule_id).max_chars;
        if own < 0 {
            return parent_deadline;
        }
        let deadline = self.current_char_index().saturating_add(own);
        if parent_deadline >= 0 {
            deadline.min(parent_deadline)
        } else {
            deadline
        }
    }

    /// Whether `byte` starts a UTF-8 codepoint (is not a continuation byte).
    #[must_use]
    pub fn starts_utf8_codepoint(byte: u8) -> bool {
        (byte & 0xC0) != 0x80
    }

    /// Removes the last `count` recorded positions.
    ///
    /// # Panics
    /// Panics if `count` is not smaller than the number of recorded positions.
    pub fn pop_last_states(
        &mut self,
        count: i32,
    ) {
        self.stop_token_is_accepted = false;
        let count = count as usize;
        assert!(
            count < self.rule_id_to_completable_states.len(),
            "The number of states to be popped is larger than the size of states."
        );
        self.rule_id_to_completable_states.pop_back(count);
        let new_len = self.is_completed.len() - count;
        self.is_completed.truncate(new_len);
        self.scanable_state_history.pop_back(count);
        if self.capture_tracking {
            self.capture_event_history.pop_back(count);
        }
        if self.has_char_budget_rules {
            let new_len = self.char_count_history.len() - count;
            self.char_count_history.truncate(new_len);
            let new_len = self.char_budget_entry_history.len() - count;
            self.char_budget_entry_history.truncate(new_len);
        }
    }

    /// The completion operation of the Earley parser: advances the parents waiting for the
    /// completed rule. `marker_present` is false when a suffix/stop rule is force-completed
    /// without its trailing marker.
    pub fn complete(
        &mut self,
        state: &ParserState,
        marker_present: bool,
    ) {
        // Record capture and hidden-span events, only during definitive advances.
        if self.capture_recording && self.rule_needs_capture_event(state.rule_id) {
            self.record_capture_event(state, marker_present);
        }
        if state.rule_id != -1 && self.grammar.rule(state.rule_id).is_lazy {
            self.tmp_completed_lazy_occurrences.push((state.rule_id, state.rule_start_pos));
        }
        if state.rule_start_pos == ParserState::NO_PREV_INPUT_POS {
            // If a root rule can achieve here, then it must be completed.
            self.tmp_accept_stop_token = true;
            return;
        }
        // Completion only enqueues, so the parent row does not change while it is walked.
        for index in self.rule_id_to_completable_states.row_range(state.rule_start_pos as usize) {
            let (ref_id, parent_state) = self.rule_id_to_completable_states.data()[index];
            if ref_id != state.rule_id {
                continue;
            }
            if parent_state.rule_id == -1 {
                let parent_expr = self.grammar.expr(parent_state.sequence_id);
                let element_expr = self.grammar.expr(parent_expr.data[parent_state.element_id as usize]);
                debug_assert!(matches!(element_expr.ty, GrammarExprType::RuleRef | GrammarExprType::Repeat));
                if element_expr.ty == GrammarExprType::RuleRef {
                    self.enqueue(parent_state.at_element(parent_state.element_id + 1));
                    continue;
                }
                // The parent state is a repeat: increase the repeat count.
                let (_, min_repeat_count, max_repeat_count) = element_expr.repeat();
                let mut new_state = parent_state;
                new_state.repeat_count += 1;
                if new_state.repeat_count >= min_repeat_count {
                    self.enqueue(parent_state.at_element(parent_state.element_id + 1));
                }
                if new_state.repeat_count < max_repeat_count {
                    self.enqueue(new_state);
                }
                continue;
            }
            // The rule is referenced by a FSM: check whether the parent sits on a repeat edge (by
            // invariance, a state with a repeat edge has exactly one outgoing edge).
            let complete_fsm = self.grammar.complete_fsm();
            let repeat = complete_fsm
                .state_edges(parent_state.element_id)
                .iter()
                .filter(|edge| edge.is_repeat_ref())
                .map(|edge| (edge.target, complete_fsm.repeat_edge_info(edge.aux_index())))
                .find(|(_, info)| info.rule_id() == ref_id)
                .map(|(target, info)| (target, info.lower(), info.upper()));
            let Some((target, lower, upper)) = repeat else {
                self.enqueue(parent_state);
                continue;
            };
            let new_count = parent_state.repeat_count + 1;
            if new_count >= lower {
                self.enqueue(parent_state.at_element(target));
            }
            if new_count < upper {
                self.enqueue(parent_state.at_element_with_repeat(parent_state.element_id, new_count));
            }
        }
    }

    /// The prediction operation of the Earley parser. Returns `(scanable, completable)`.
    pub fn predict(
        &mut self,
        state: &ParserState,
    ) -> (bool, bool) {
        if state.rule_id != -1 {
            let flags = self.fsm_state_flags(state.rule_id, state.element_id);
            if flags & FSM_STATE_NON_TERMINAL != 0 {
                self.expand_next_rule_ref_element_on_fsm(state);
            }
            return (flags & FSM_STATE_SCANABLE != 0, flags & FSM_STATE_END != 0);
        }
        let grammar_expr = self.grammar.expr(state.sequence_id);
        debug_assert!(matches!(grammar_expr.ty, GrammarExprType::Sequence | GrammarExprType::EmptyStr));
        if state.element_id == grammar_expr.len() as i32 {
            // The rule is completed.
            return (false, true);
        }
        let element_expr_id = grammar_expr.data[state.element_id as usize];
        let element_expr = self.grammar.expr(element_expr_id);
        match element_expr.ty {
            GrammarExprType::RuleRef => {
                self.expand_next_rule_ref_element(state, state.sequence_id, element_expr_id);
                (false, false)
            },
            GrammarExprType::CharacterClassStar => {
                if state.sub_element_id == 0 {
                    self.enqueue(state.at_element(state.element_id + 1));
                }
                (true, false)
            },
            GrammarExprType::Repeat => {
                let (_, min_repeat_count, max_repeat_count) = element_expr.repeat();
                debug_assert!(state.repeat_count <= max_repeat_count);
                self.expand_next_rule_ref_element(state, state.sequence_id, element_expr_id);
                if state.repeat_count >= min_repeat_count {
                    self.enqueue(state.at_element(state.element_id + 1));
                }
                (false, false)
            },
            GrammarExprType::ByteString | GrammarExprType::CharacterClass => (true, false),
            GrammarExprType::Token | GrammarExprType::ExcludeToken => (false, false),
            other => panic!("The element type is not supported! The type is: {other:?}"),
        }
    }

    fn scan(
        &mut self,
        state: &ParserState,
        ch: u8,
    ) {
        if state.rule_id != -1 {
            self.advance_fsm(state, ch);
            return;
        }
        let element_expr_id = self.grammar.expr(state.sequence_id).data[state.element_id as usize];
        match self.grammar.expr(element_expr_id).ty {
            GrammarExprType::ByteString => self.advance_byte_string(state, ch, element_expr_id),
            GrammarExprType::CharacterClass => self.advance_character_class(state, ch, element_expr_id),
            GrammarExprType::CharacterClassStar => self.advance_character_class_star(state, ch, element_expr_id),
            other => panic!("The element type is not supported! The type is: {other:?}"),
        }
    }

    /// Runs the predict/complete loop over the processing queue.
    pub fn process_queue(&mut self) {
        while let Some(state) = self.tmp_process_state_queue.pop_front() {
            let (scanable, completable) = self.predict(&state);
            if completable {
                self.complete(&state, true);
            }
            if scanable {
                self.tmp_states_to_be_added.push(state);
            }
        }
    }

    /// Advances the parser by one input byte. Returns false (leaving state unchanged) if the
    /// byte is not accepted.
    pub fn advance(
        &mut self,
        ch: u8,
    ) -> bool {
        debug_assert!(self.tmp_process_state_queue.is_empty());
        self.tmp_states_visited_in_queue.clear();
        self.tmp_states_to_be_added.clear();
        self.tmp_accept_stop_token = false;
        self.tmp_completed_lazy_occurrences.clear();
        if self.has_char_budget_rules {
            self.tmp_char_budget_entered = *self.char_budget_entry_history.last().expect("char budget history");
            self.char_count_history.push(self.current_char_index() + i32::from(Self::starts_utf8_codepoint(ch)));
        }
        // Scanning only queues states, so the latest row does not change while it is walked.
        for index in self.scanable_state_history.row_range(self.scanable_state_history.len() - 1) {
            let state = self.scanable_state_history.data()[index];
            if self.skip_expired_states && self.is_expired_state(&state) {
                continue;
            }
            self.scan(&state, ch);
        }
        if self.tmp_process_state_queue.is_empty() && self.tmp_states_to_be_added.is_empty() {
            if self.has_char_budget_rules {
                self.char_count_history.pop();
            }
            return false;
        }
        self.rule_id_to_completable_states.push_row(&[]);
        if self.capture_tracking {
            self.capture_event_history.push_row(&[]);
        }
        self.process_queue();
        self.finish_row(false);
        true
    }

    /// Finalizes the row built by the processing loop: removes committed lazy states and
    /// records completion, scanable states and (unless already pushed) the character rows.
    fn finish_row(
        &mut self,
        push_char_count: bool,
    ) {
        if !self.tmp_completed_lazy_occurrences.is_empty() {
            self.remove_committed_lazy_states();
        }
        self.is_completed.push(self.tmp_accept_stop_token);
        self.scanable_state_history.push_row(&self.tmp_states_to_be_added);
        if self.has_char_budget_rules {
            if push_char_count {
                self.char_count_history.push(self.current_char_index());
            }
            self.char_budget_entry_history.push(self.tmp_char_budget_entered);
        }
    }

    /// Removes the states of the lazy occurrences completed in the current row
    /// (committed-shortest matching).
    pub fn remove_committed_lazy_states(&mut self) {
        let committed = std::mem::take(&mut self.tmp_completed_lazy_occurrences);
        self.tmp_states_to_be_added.retain(|state| {
            !committed
                .iter()
                .any(|&(rule_id, rule_start_pos)| state.rule_id == rule_id && state.rule_start_pos == rule_start_pos)
        });
        self.tmp_completed_lazy_occurrences = committed;
    }

    #[inline]
    fn fsm_state_flags(
        &mut self,
        rule_id: i32,
        state_id: i32,
    ) -> u8 {
        // Per-rule FSMs are views over the grammar's complete FSM, so a state id determines its
        // rule and one cache over the complete FSM serves every rule (upstream keeps a full-size
        // cache per rule).
        match self.fsm_state_flags_cache.get(state_id as usize) {
            Some(&flags) if flags != 0 => flags,
            _ => self.initialize_fsm_state_flags(rule_id, state_id),
        }
    }

    #[cold]
    #[inline(never)]
    fn initialize_fsm_state_flags(
        &mut self,
        rule_id: i32,
        state_id: i32,
    ) -> u8 {
        let fsm = self.grammar.per_rule_fsm(rule_id).expect("per-rule FSM").fsm();
        let cache = &mut self.fsm_state_flags_cache;
        if cache.is_empty() {
            cache.resize(fsm.num_states() as usize, 0);
        }
        let mut flags = FSM_STATE_INITIALIZED;
        if fsm.is_end_state(state_id) {
            flags |= FSM_STATE_END;
        }
        let edges = fsm.fsm().state_edges(state_id);
        if !edges.is_empty() {
            flags |= FSM_STATE_HAS_EDGES;
        }
        for edge in edges {
            if edge.is_char_range() || edge.is_token() || edge.is_exclude_token() {
                flags |= FSM_STATE_SCANABLE;
            } else if edge.is_rule_ref() || edge.is_epsilon() || edge.is_repeat_ref() {
                flags |= FSM_STATE_NON_TERMINAL;
            }
        }
        cache[state_id as usize] = flags;
        flags
    }

    /// The initial state expanded from the root rule of the grammar.
    #[must_use]
    pub fn root_initial_state(&self) -> ParserState {
        let root_rule_id = self.grammar.root_rule_id();
        ParserState {
            budget_deadline: self.deadline_for_rule(root_rule_id, -1),
            active_temperature_rule_id: self.resolve_active_temperature_rule(root_rule_id, -1),
            char_budget_deadline: self.char_deadline_for_rule(root_rule_id, -1),
            ..ParserState::new(
                root_rule_id,
                self.grammar.root_rule().body_expr_id,
                self.grammar.per_rule_fsm(root_rule_id).expect("per-rule FSM").fsm().start(),
                ParserState::NO_PREV_INPUT_POS,
            )
        }
    }

    /// Pushes `state` and expands it through the predict/complete loop into a new position.
    pub fn push_state_and_expand(
        &mut self,
        state: ParserState,
    ) {
        self.tmp_states_visited_in_queue.clear();
        self.tmp_accept_stop_token = false;
        self.tmp_states_to_be_added.clear();
        self.tmp_completed_lazy_occurrences.clear();
        self.enqueue(state);
        self.rule_id_to_completable_states.push_row(&[]);
        if self.capture_tracking {
            self.capture_event_history.push_row(&[]);
        }
        self.process_queue();
        self.finish_row(true);
    }

    /// Resets the parser to the freshly-expanded root rule.
    pub fn reset(&mut self) {
        let rows = self.rule_id_to_completable_states.len();
        self.rule_id_to_completable_states.pop_back(rows);
        let rows = self.scanable_state_history.len();
        self.scanable_state_history.pop_back(rows);
        self.is_completed.clear();
        self.stop_token_is_accepted = false;
        if self.capture_tracking {
            let rows = self.capture_event_history.len();
            self.capture_event_history.pop_back(rows);
        }
        self.char_count_history.clear();
        self.char_budget_entry_history.clear();
        self.tmp_char_budget_entered = false;
        self.capture_recording = false;
        debug_assert!(self.tmp_process_state_queue.is_empty());
        let root = self.root_initial_state();
        self.push_state_and_expand(root);
    }

    /// Records the completable parent pairs for a predicted rule reference, applying the
    /// right-recursion optimization when allowed. Returns whether the child becomes a root.
    fn add_completable_parents(
        &mut self,
        state: &ParserState,
        ref_rule_id: i32,
        is_right_recursion: bool,
        parent_entry: ParserState,
    ) -> bool {
        // The right-recursion optimization elides the completion of the parent rule (and, in the
        // to-root case, corrupts the start position of the child rule), so it is disabled when
        // either rule produces capture-history events.
        let cur_pos = self.rule_id_to_completable_states.len() as i32 - 1;
        if !is_right_recursion
            || state.rule_start_pos == cur_pos
            || self.rule_needs_capture_event(state.rule_id)
            || self.rule_needs_capture_event(ref_rule_id)
        {
            self.rule_id_to_completable_states.push_in_latest_row((ref_rule_id, parent_entry));
            return false;
        }
        if state.rule_start_pos == ParserState::NO_PREV_INPUT_POS {
            return true;
        }
        // The parents live in an earlier row, so appending to the latest row leaves them in place;
        // like upstream, duplicates are checked against the latest row as it was before the loop.
        let parents = self.rule_id_to_completable_states.row_range(state.rule_start_pos as usize);
        let latest = self.rule_id_to_completable_states.row_range(self.rule_id_to_completable_states.len() - 1);
        for index in parents {
            let (parent_rule_id, parent_state) = self.rule_id_to_completable_states.data()[index];
            if parent_rule_id != state.rule_id {
                continue;
            }
            let known = self.rule_id_to_completable_states.data()[latest.clone()]
                .iter()
                .any(|&(rule_id, s)| s == parent_state && rule_id == ref_rule_id);
            if !known {
                self.rule_id_to_completable_states.push_in_latest_row((ref_rule_id, parent_state));
            }
        }
        false
    }

    /// Enqueues the start state of a newly predicted occurrence of `ref_rule_id`.
    fn enqueue_rule_start(
        &mut self,
        state: &ParserState,
        ref_rule_id: i32,
        right_recursion_to_root: bool,
    ) {
        let ref_rule = self.grammar.rule(ref_rule_id);
        if ref_rule.max_chars >= 0 {
            self.tmp_char_budget_entered = true;
        }
        let start_pos = if right_recursion_to_root {
            ParserState::NO_PREV_INPUT_POS
        } else {
            self.rule_id_to_completable_states.len() as i32 - 1
        };
        let new_state = ParserState {
            budget_deadline: self.deadline_for_rule(ref_rule_id, state.budget_deadline),
            active_temperature_rule_id: self
                .resolve_active_temperature_rule(ref_rule_id, state.active_temperature_rule_id),
            char_budget_deadline: self.char_deadline_for_rule(ref_rule_id, state.char_budget_deadline),
            ..ParserState::new(
                ref_rule_id,
                ref_rule.body_expr_id,
                self.grammar.per_rule_fsm(ref_rule_id).expect("per-rule FSM").fsm().start(),
                start_pos,
            )
        };
        self.enqueue(new_state);
    }

    /// Predicts a rule reference or repeat element in an IR sequence (lookahead path).
    fn expand_next_rule_ref_element(
        &mut self,
        state: &ParserState,
        sequence_id: i32,
        sub_element_expr_id: i32,
    ) {
        let sequence_len = self.grammar.expr(sequence_id).len() as i32;
        let sub_expr = self.grammar.expr(sub_element_expr_id);
        debug_assert!(matches!(sub_expr.ty, GrammarExprType::RuleRef | GrammarExprType::Repeat));
        let ref_rule_id = sub_expr.data[0];
        let is_right_recursion = state.element_id == sequence_len - 1 && sub_expr.ty != GrammarExprType::Repeat;
        let right_recursion_to_root = self.add_completable_parents(state, ref_rule_id, is_right_recursion, *state);
        if self.rule_is_nullable[ref_rule_id as usize] {
            self.enqueue(state.at_element(state.element_id + 1));
        }
        self.enqueue_rule_start(state, ref_rule_id, right_recursion_to_root);
    }

    /// Predicts the rule references, repeat references, and epsilon edges of a FSM state.
    fn expand_next_rule_ref_element_on_fsm(
        &mut self,
        state: &ParserState,
    ) {
        let edges: SmallVec<[FsmEdge; 8]> =
            SmallVec::from_slice(self.grammar.complete_fsm().state_edges(state.element_id));
        for edge in &edges {
            if edge.is_epsilon() {
                self.enqueue(state.at_element(edge.target));
                continue;
            }
            let target = edge.target;
            let (ref_rule_id, is_repeat) = if edge.is_rule_ref() {
                (edge.ref_rule_id(), false)
            } else if edge.is_repeat_ref() {
                let info = self.grammar.complete_fsm().repeat_edge_info(edge.aux_index());
                let (rule_id, lower, upper) = (info.rule_id(), info.lower(), info.upper());
                if state.repeat_count >= lower {
                    self.enqueue(state.at_element(target));
                }
                if state.repeat_count >= upper {
                    continue;
                }
                (rule_id, true)
            } else {
                continue;
            };

            let target_flags = self.fsm_state_flags(state.rule_id, target);
            let is_right_recursion =
                !is_repeat && target_flags & FSM_STATE_HAS_EDGES == 0 && target_flags & FSM_STATE_END != 0;
            // For repeat edges the parent entry keeps the source state and the repeat count; for
            // rule references it is the post-transition state.
            let parent_entry = if is_repeat {
                state.at_element_with_repeat(state.element_id, state.repeat_count)
            } else {
                state.at_element(target)
            };
            let right_recursion_to_root =
                self.add_completable_parents(state, ref_rule_id, is_right_recursion, parent_entry);
            if !is_repeat && self.rule_is_nullable[ref_rule_id as usize] {
                self.enqueue(state.at_element(target));
            }
            self.enqueue_rule_start(state, ref_rule_id, right_recursion_to_root);
        }
    }

    fn advance_byte_string(
        &mut self,
        state: &ParserState,
        ch: u8,
        expr_id: i32,
    ) {
        let expr = self.grammar.expr(expr_id);
        debug_assert!(expr.len() as i32 > state.sub_element_id);
        if expr.data[state.sub_element_id as usize] as u8 != ch {
            return;
        }
        let mut new_state = *state;
        new_state.sub_element_id += 1;
        if new_state.sub_element_id == expr.len() as i32 {
            new_state.element_id += 1;
            new_state.sub_element_id = 0;
            self.enqueue(new_state);
        } else {
            self.tmp_states_to_be_added.push(new_state);
        }
    }

    /// Shared UTF-8 handling of character classes. `star` keeps the state on the same element
    /// after a full codepoint is matched.
    fn advance_character_class_impl(
        &mut self,
        state: &ParserState,
        ch: u8,
        expr_id: i32,
        star: bool,
    ) {
        let class_data = self.grammar.expr(expr_id).data;
        let is_negative = class_data[0] != 0;
        let ranges = || class_data[1..].chunks_exact(2);
        let finish = |new_state: &mut ParserState| {
            if !star {
                new_state.element_id += 1;
            }
            new_state.partial_codepoint = 0;
        };

        // The state is matching a UTF-8 character (continuation bytes).
        if state.sub_element_id > 0 {
            if (ch & 0xC0) != 0x80 {
                return;
            }
            let mut new_state = *state;
            new_state.sub_element_id -= 1;
            new_state.partial_codepoint = (new_state.partial_codepoint << 6) | i32::from(ch & 0x3F);
            if new_state.sub_element_id == 0 {
                let codepoint = new_state.partial_codepoint;
                let matches_range = ranges().any(|range| codepoint >= range[0] && codepoint <= range[1]);
                if matches_range != is_negative {
                    finish(&mut new_state);
                    self.enqueue(new_state);
                }
            } else {
                // Check whether the partial codepoint could still match any range. Negative
                // classes always continue and verify on the final byte.
                let remaining_bytes = new_state.sub_element_id;
                let min_codepoint = new_state.partial_codepoint << (6 * remaining_bytes);
                let max_codepoint = min_codepoint | ((1 << (6 * remaining_bytes)) - 1);
                if is_negative || ranges().any(|range| max_codepoint >= range[0] && min_codepoint <= range[1]) {
                    self.tmp_states_to_be_added.push(new_state);
                }
            }
            return;
        }

        // Non-ASCII first bytes.
        if !ch.is_ascii() {
            let Some((num_bytes, partial)) = handle_utf8_first_byte(ch) else {
                return;
            };
            debug_assert!(num_bytes > 1);
            let remaining = 6 * (num_bytes as i32 - 1);
            let min_codepoint = partial << remaining;
            let max_codepoint = min_codepoint | ((1 << remaining) - 1);
            if is_negative || ranges().any(|range| max_codepoint >= range[0] && min_codepoint <= range[1]) {
                let mut new_state = *state;
                new_state.sub_element_id = num_bytes as i32 - 1;
                new_state.partial_codepoint = partial;
                self.tmp_states_to_be_added.push(new_state);
            }
            return;
        }

        let in_range = ranges().any(|range| range[0] as u8 <= ch && ch <= range[1] as u8);
        if in_range != is_negative {
            let mut new_state = *state;
            if !star {
                new_state.element_id += 1;
                new_state.sub_element_id = 0;
            }
            self.enqueue(new_state);
        }
    }

    fn advance_character_class(
        &mut self,
        state: &ParserState,
        ch: u8,
        expr_id: i32,
    ) {
        debug_assert_eq!(self.grammar.expr(expr_id).ty, GrammarExprType::CharacterClass);
        self.advance_character_class_impl(state, ch, expr_id, false);
    }

    fn advance_character_class_star(
        &mut self,
        state: &ParserState,
        ch: u8,
        expr_id: i32,
    ) {
        debug_assert_eq!(self.grammar.expr(expr_id).ty, GrammarExprType::CharacterClassStar);
        self.advance_character_class_impl(state, ch, expr_id, true);
    }

    /// Enqueues a post-scan state: scanable leaves skip the predict/complete queue.
    fn enqueue_scan_target(
        &mut self,
        new_state: ParserState,
    ) {
        let flags = self.fsm_state_flags(new_state.rule_id, new_state.element_id);
        if flags & FSM_STATE_NON_TERMINAL == 0 && flags & FSM_STATE_END == 0 && flags & FSM_STATE_SCANABLE != 0 {
            self.enqueue_without_processing(new_state);
        } else {
            self.enqueue(new_state);
        }
    }

    fn advance_fsm(
        &mut self,
        state: &ParserState,
        ch: u8,
    ) {
        let value = i32::from(ch);
        let targets: SmallVec<[i32; 4]> = self
            .grammar
            .complete_fsm()
            .state_edges(state.element_id)
            .iter()
            .filter(|edge| edge.is_char_range() && edge.min <= value && value <= edge.max)
            .map(|edge| edge.target)
            .collect();
        for element_id in targets {
            self.enqueue_scan_target(ParserState {
                element_id,
                ..*state
            });
        }
    }

    fn scan_atomic_token(
        &mut self,
        state: &ParserState,
        token_id: i32,
    ) {
        if state.rule_id == -1 {
            return;
        }
        let fsm = self.grammar.complete_fsm();
        let targets: SmallVec<[i32; 4]> = fsm
            .state_edges(state.element_id)
            .iter()
            .filter(|edge| {
                if edge.is_token() {
                    fsm.token_edge_info(edge.aux_index()).contains(token_id)
                } else if edge.is_exclude_token() {
                    fsm.exclude_token_edge_info(edge.aux_index()).accepts(token_id)
                } else {
                    false
                }
            })
            .map(|edge| edge.target)
            .collect();
        for element_id in targets {
            self.enqueue_scan_target(ParserState {
                element_id,
                ..*state
            });
        }
    }

    /// Advances the parser by accepting a whole token via token/exclude-token edges, consuming
    /// `token_char_count` codepoints. Returns false (state unchanged) if no state advances.
    pub fn advance_atomic_token(
        &mut self,
        token_id: i32,
        token_char_count: i32,
    ) -> bool {
        debug_assert!(self.tmp_process_state_queue.is_empty());
        self.tmp_states_visited_in_queue.clear();
        self.tmp_states_to_be_added.clear();
        self.tmp_accept_stop_token = false;
        self.tmp_completed_lazy_occurrences.clear();
        if self.has_char_budget_rules {
            self.tmp_char_budget_entered = *self.char_budget_entry_history.last().expect("char budget history");
            self.char_count_history.push(self.current_char_index() + token_char_count);
        }
        let latest_states = self.latest_scanable_states();
        for state in &latest_states {
            if self.skip_expired_states && self.is_expired_state(state) {
                continue;
            }
            self.scan_atomic_token(state, token_id);
        }
        if self.tmp_process_state_queue.is_empty() && self.tmp_states_to_be_added.is_empty() {
            if self.has_char_budget_rules {
                self.char_count_history.pop();
            }
            return false;
        }
        self.rule_id_to_completable_states.push_row(&[]);
        if self.capture_tracking {
            self.capture_event_history.push_row(&[]);
        }
        self.process_queue();
        self.finish_row(false);
        true
    }

    /// Enqueues `state` for prediction/completion if it was not visited this round.
    pub fn enqueue(
        &mut self,
        state: ParserState,
    ) {
        if !self.tmp_states_visited_in_queue.is_visited(&state) {
            self.tmp_process_state_queue.push_back(state);
            self.tmp_states_visited_in_queue.insert(state);
        }
    }

    /// Adds `state` directly to the next scanable row if it was not visited this round.
    pub fn enqueue_without_processing(
        &mut self,
        state: ParserState,
    ) {
        if !self.tmp_states_visited_in_queue.is_visited(&state) {
            self.tmp_states_visited_in_queue.insert(state);
            self.tmp_states_to_be_added.push(state);
        }
    }

    /// Pushes one state as a new temporary position to check (without expanding it).
    pub fn push_one_state_to_check(
        &mut self,
        state: ParserState,
    ) {
        let completed = self.is_completed();
        self.push_states_to_check(&[state], completed);
    }

    /// Pushes a temporary parser row for token-mask checking.
    pub fn push_states_to_check(
        &mut self,
        states: &[ParserState],
        completed: bool,
    ) {
        self.rule_id_to_completable_states.push_row(&[]);
        self.is_completed.push(completed);
        self.scanable_state_history.push_row(states);
        if self.capture_tracking {
            self.capture_event_history.push_row(&[]);
        }
        if self.has_char_budget_rules {
            self.char_count_history.push(self.current_char_index());
            let entered = *self.char_budget_entry_history.last().expect("char budget history");
            self.char_budget_entry_history.push(entered);
        }
    }

    /// Pushes a character-count row for a parser row created by the matcher.
    pub fn push_char_count_row(
        &mut self,
        char_count: i32,
        char_budget_entered: bool,
    ) {
        if self.has_char_budget_rules {
            self.char_count_history.push(char_count);
            self.char_budget_entry_history.push(char_budget_entered);
        }
    }

    /// The number of codepoints accepted so far.
    #[must_use]
    pub fn current_char_index(&self) -> i32 {
        self.char_count_history.last().copied().unwrap_or(0)
    }

    /// Whether a character-budgeted occurrence was entered since the initial row.
    #[must_use]
    pub fn has_entered_char_budget(&self) -> bool {
        self.has_char_budget_rules && self.char_budget_entry_history.last().copied().unwrap_or(false)
    }

    /// Copies the capture events of the latest input position.
    #[must_use]
    pub fn copy_last_capture_row(&self) -> Vec<CaptureEvent> {
        if self.capture_tracking {
            self.capture_event_history.back().to_vec()
        } else {
            Vec::new()
        }
    }

    /// Pushes a new row of capture events, keeping the capture history aligned with the state
    /// history when a position is created outside of `advance`.
    pub fn push_capture_row(
        &mut self,
        events: &[CaptureEvent],
    ) {
        if self.capture_tracking {
            self.capture_event_history.push_row(events);
        }
    }

    /// A copy of the latest history row.
    #[must_use]
    pub fn latest_row(&self) -> ParserRow {
        ParserRow {
            states: self.latest_scanable_states(),
            completable_states: self.latest_completable_states(),
            completed: self.is_completed(),
            capture_events: self.copy_last_capture_row(),
        }
    }

    /// Replaces the latest history row (used to restore a row after a failed tentative
    /// transition, or to commit an enforced one).
    pub fn replace_latest_row(
        &mut self,
        row: &ParserRow,
    ) {
        self.scanable_state_history.pop_back(1);
        self.scanable_state_history.push_row(&row.states);
        self.rule_id_to_completable_states.pop_back(1);
        self.rule_id_to_completable_states.push_row(&row.completable_states);
        *self.is_completed.last_mut().expect("parser always has at least one position") = row.completed;
        if self.capture_tracking {
            self.capture_event_history.pop_back(1);
            self.capture_event_history.push_row(&row.capture_events);
        }
    }

    /// A human-readable dump of the latest scanable states.
    #[must_use]
    pub fn print_states(&self) -> String {
        let mut result = format!("There are {} steps in history. Last step: [\n", self.scanable_state_history.len());
        for state in self.scanable_state_history.back() {
            result.push_str(&format!("{state}, \n"));
        }
        result.push(']');
        result
    }
}
