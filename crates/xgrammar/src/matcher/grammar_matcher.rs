//! Stateful matcher to match tokens to a BNF grammar — the core logic of grammar-guided
//! generation.
//!
//! This module implements the non-deterministic pushdown automaton (NPDA) matching algorithm
//! to match characters to a BNF grammar. It keeps track of the current state of the matching
//! process by maintaining several stacks internally as possible paths in the NPDA. It also
//! supports backtracking, token and character budgets, captures, and sampling temperatures.
//!
//! It is particularly capable of finding the set of tokens that are acceptable for the next
//! step and storing them in a bitmask. This aids in grammar-guided generation.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, OnceLock},
    time::Instant,
};

use super::{
    budget_body_match_progress::BudgetBodyMatchProgress,
    capture_span_event::CaptureSpanEvent,
    error::MatcherError,
    token_bitmask::{bitmask_row_range, get_bitmask_size},
};
use crate::{
    compiler::{AdaptiveTokenMask, AdaptiveTokenMaskCache, AdaptiveTokenMaskStoreType, CompiledGrammar},
    fsm::CompactFsmWithStartEnd,
    parser::{EarleyParser, ParserRow, ParserState},
    support::{common_prefix_len, escape_bytes, intset_intersection, intset_union},
    tokenizer::TokenizerInfo,
};

/// A stateful matcher to match tokens to the specified BNF grammar.
///
/// This class is the core logic of grammar-guided generation. It implements the NPDA matching
/// algorithm, maintains several internal stacks as possible paths in the NPDA, and supports
/// backtracking. It can find the set of tokens acceptable for the next step and store them in
/// a bitmask.
#[derive(Debug, Clone)]
pub struct GrammarMatcher {
    parser: EarleyParser,
    tokenizer_info: Arc<TokenizerInfo>,
    adaptive_token_mask_cache: Arc<AdaptiveTokenMaskCache>,
    stop_token_ids: Vec<i32>,
    terminate_without_stop_token: bool,
    default_temperature: Option<f32>,
    /// Set once the temperature-conflict warning was reported (keeps the matcher `Sync`).
    has_warned_temperature_conflict: OnceLock<()>,
    /// Number of parser rows created by each accepted string/token, for rollback.
    token_length_history: Vec<i32>,
    /// Set by the last mask computation when an exhausted budget could be enforced: the next
    /// accept commits the same state transition used to compute that mask.
    budget_enforce_pending: bool,
    /// Whether the pending decision includes a suffix/stop completion without its marker.
    budget_force_close_pending: bool,
    budget_exceeded_warned: bool,
    char_budget_exceeded_warned: bool,
    /// Whether the current definitive accept relaxed a character budget.
    char_budget_relaxed: bool,
    /// Whether speculative character-budget relaxation should be recorded.
    record_char_budget_relaxation: bool,
    /// Whether byte history is needed to recognize a budgeted suffix/stop body boundary.
    has_budget_marker_rules: bool,
    /// Whether the grammar has Token/ExcludeToken edges.
    has_token_edges: bool,
    /// Incremental body-FSM progress keyed by (rule id, occurrence start row).
    budget_body_match_cache: HashMap<i64, BudgetBodyMatchProgress>,
    /// The bytes accepted so far (maintained for captures and budgeted marker rules).
    accepted_bytes: Vec<u8>,
    /// `row_byte_end[i]` is the number of accepted bytes after input position `i`.
    row_byte_end: Vec<i64>,
    /// Bytes tentatively consumed by the current token or string advance.
    temporary_input_bytes: Vec<u8>,
    /// Parser row immediately before `temporary_input_bytes`.
    temporary_input_start_row: i32,
    tmp_rejected_indices: Vec<i32>,
    tmp_rejected_indices_delta: Vec<i32>,
}

fn warn(message: &str) {
    eprintln!("{message}");
}

fn occurrence_key(
    rule_id: i32,
    start_pos: i32,
) -> i64 {
    (i64::from(rule_id) << 32) | i64::from(start_pos as u32)
}

fn get_bit(
    row: &[i32],
    token_id: i32,
) -> bool {
    (row[(token_id / 32) as usize] >> (token_id % 32)) & 1 != 0
}

fn set_bit(
    row: &mut [i32],
    token_id: i32,
    value: bool,
) {
    let (word, bit) = ((token_id / 32) as usize, token_id % 32);
    if value {
        row[word] |= 1 << bit;
    } else {
        row[word] &= !(1 << bit);
    }
}

/// Word-level checks over the first `vocab_size` bits of `row` (the C++ `DynamicBitset::Any` /
/// `All` view of the bitmask): full words are compared whole, the partial last word is masked.
/// Words per block of the bitmask row scans. Each block is reduced without branches (so it
/// vectorizes) and the scan stops at the first deciding block; upstream checks word by word.
const SCAN_BLOCK_WORDS: usize = 64;

fn row_has_any_token(
    row: &[i32],
    vocab_size: i32,
) -> bool {
    let (full_words, last_mask) = ((vocab_size / 32) as usize, !(-1 << (vocab_size % 32)));
    row[..full_words].chunks(SCAN_BLOCK_WORDS).any(|block| block.iter().fold(0, |acc, &word| acc | word) != 0)
        || row.get(full_words).is_some_and(|&word| word & last_mask != 0)
}

fn row_all_true(
    row: &[i32],
    vocab_size: i32,
) -> bool {
    let (full_words, last_mask) = ((vocab_size / 32) as usize, !(-1 << (vocab_size % 32)));
    row[..full_words].chunks(SCAN_BLOCK_WORDS).all(|block| block.iter().fold(-1, |acc, &word| acc & word) == -1)
        && row.get(full_words).is_none_or(|&word| word & last_mask == last_mask)
}

impl GrammarMatcher {
    /// Constructs a [`GrammarMatcher`] from the preprocessing result of type
    /// [`CompiledGrammar`], terminating on the tokenizer's stop tokens.
    #[must_use]
    pub fn from_compiled_grammar(
        compiled: &CompiledGrammar,
        terminate_without_stop_token: bool,
    ) -> Self {
        Self::with_options(compiled, None, terminate_without_stop_token, None)
            .expect("no override stop tokens or default temperature to validate")
    }

    /// Constructs a [`GrammarMatcher`] with an optional stop-token override and a default
    /// sampling temperature used when no active rule specifies one.
    ///
    /// # Errors
    /// Returns [`MatcherError`] if `override_stop_tokens` is empty or out of the vocabulary,
    /// or `default_temperature` is negative or not finite.
    pub fn with_options(
        compiled: &CompiledGrammar,
        override_stop_tokens: Option<Vec<i32>>,
        terminate_without_stop_token: bool,
        default_temperature: Option<f32>,
    ) -> Result<Self, MatcherError> {
        let tokenizer_info = compiled.tokenizer_info_arc();
        let vocab_size = tokenizer_info.vocab_size();
        if let Some(ids) = &override_stop_tokens {
            if ids.is_empty() {
                return Err(MatcherError::EmptyOverrideStopTokens);
            }
            // Stop token ids are written into the token bitmask, so they must be in range.
            if let Some(&token_id) = ids.iter().find(|&&id| id < 0 || id >= vocab_size) {
                return Err(MatcherError::StopTokenOutOfRange {
                    token_id,
                    vocab_size,
                });
            }
        }
        if default_temperature.is_some_and(|temperature| !(temperature.is_finite() && temperature >= 0.0)) {
            return Err(MatcherError::InvalidDefaultTemperature);
        }
        let parser = EarleyParser::new(Arc::clone(compiled.grammar()), None);
        let grammar = Arc::clone(&parser.grammar);
        let has_budget_marker_rules = (parser.has_budget_rules || parser.has_char_budget_rules)
            && (0..grammar.num_rules()).any(|rule_id| {
                let rule = grammar.rule(rule_id);
                (rule.max_tokens >= 0 || rule.max_chars >= 0)
                    && grammar.suffix_stop_info(rule_id).is_some_and(|info| info.body_rule_id >= 0)
            });
        let has_token_edges = parser.has_char_budget_rules && {
            let fsm = grammar.complete_fsm();
            (0..fsm.num_states())
                .any(|state| fsm.state_edges(state).iter().any(|edge| edge.is_token() || edge.is_exclude_token()))
        };
        let stop_token_ids = override_stop_tokens.unwrap_or_else(|| tokenizer_info.stop_token_ids().to_vec());
        Ok(Self {
            parser,
            adaptive_token_mask_cache: compiled.adaptive_token_mask_cache_arc(),
            tokenizer_info,
            stop_token_ids,
            terminate_without_stop_token,
            default_temperature,
            has_warned_temperature_conflict: OnceLock::new(),
            token_length_history: Vec::new(),
            budget_enforce_pending: false,
            budget_force_close_pending: false,
            budget_exceeded_warned: false,
            char_budget_exceeded_warned: false,
            char_budget_relaxed: false,
            record_char_budget_relaxation: false,
            has_budget_marker_rules,
            has_token_edges,
            budget_body_match_cache: HashMap::new(),
            accepted_bytes: Vec::new(),
            row_byte_end: vec![0],
            temporary_input_bytes: Vec::new(),
            temporary_input_start_row: -1,
            tmp_rejected_indices: Vec::new(),
            tmp_rejected_indices_delta: Vec::new(),
        })
    }

    /// The tokenizer info backing this matcher.
    #[must_use]
    pub fn tokenizer_info(&self) -> &TokenizerInfo {
        &self.tokenizer_info
    }

    /// The stop token ids this matcher accepts as terminators.
    #[must_use]
    pub fn stop_token_ids(&self) -> &[i32] {
        &self.stop_token_ids
    }

    /// The underlying Earley parser.
    #[must_use]
    pub fn parser(&self) -> &EarleyParser {
        &self.parser
    }

    /// Returns the maximum number of rollback tokens allowed (`-1` means unbounded).
    #[must_use]
    pub fn max_rollback_tokens(&self) -> i32 {
        -1
    }

    /// Forks the matcher: a deep copy of all state except the compiled grammar and tokenizer
    /// info, which are shared.
    #[must_use]
    pub fn fork(&self) -> GrammarMatcher {
        self.clone()
    }

    /// A human-readable dump of the matcher's latest internal parser states (debugging only).
    #[must_use]
    pub fn debug_print_internal_state(&self) -> String {
        self.parser.print_states()
    }

    /// Checks if the grammar's root rule has been fully matched by the input accepted so far.
    #[must_use]
    pub fn is_completed(&self) -> bool {
        self.parser.is_completed()
    }

    /// Whether the stop token has been accepted.
    #[must_use]
    pub fn is_stop_token_accepted(&self) -> bool {
        self.parser.stop_token_is_accepted
    }

    /// Checks if the matcher has accepted the stop token and terminated (or, with
    /// `terminate_without_stop_token`, whether the grammar is completed).
    #[must_use]
    pub fn is_terminated(&self) -> bool {
        if self.terminate_without_stop_token {
            return self.parser.is_completed();
        }
        self.is_stop_token_accepted()
    }

    /// Accepts the stop token if the grammar is currently completed.
    pub fn accept_stop_token(&mut self) -> bool {
        if self.terminate_without_stop_token || !self.parser.is_completed() {
            return false;
        }
        debug_assert!(!self.parser.stop_token_is_accepted);
        self.token_length_history.push(0);
        self.parser.stop_token_is_accepted = true;
        true
    }

    /// Resets the matcher to the initial state.
    pub fn reset(&mut self) {
        self.token_length_history.clear();
        self.parser.current_token_index = -1;
        self.budget_enforce_pending = false;
        self.budget_force_close_pending = false;
        self.budget_exceeded_warned = false;
        self.char_budget_exceeded_warned = false;
        self.char_budget_relaxed = false;
        self.record_char_budget_relaxation = false;
        self.budget_body_match_cache.clear();
        self.temporary_input_start_row = -1;
        self.temporary_input_bytes.clear();
        self.accepted_bytes.clear();
        self.row_byte_end = vec![0];
        self.parser.reset();
    }

    fn should_track_accepted_bytes(&self) -> bool {
        self.parser.capture_tracking || self.has_budget_marker_rules
    }

    /// Records one new input position per byte of `bytes`.
    fn append_per_byte_rows(
        &mut self,
        bytes: &[u8],
    ) {
        for &byte in bytes {
            self.accepted_bytes.push(byte);
            self.row_byte_end.push(self.accepted_bytes.len() as i64);
        }
    }

    /// Records one new input position consuming all bytes of a token.
    fn append_atomic_row(
        &mut self,
        bytes: &[u8],
    ) {
        self.accepted_bytes.extend_from_slice(bytes);
        self.row_byte_end.push(self.accepted_bytes.len() as i64);
    }

    fn finish_character_budget_accept(&mut self) {
        if self.char_budget_relaxed && !self.char_budget_exceeded_warned {
            self.char_budget_exceeded_warned = true;
            warn(
                "The character budget (max_chars) of a rule was exceeded: the rule could not end at the position where \
                 its budget ran out, so the budget is relaxed until the rule can end. This warning is reported once \
                 per matcher.",
            );
        }
        self.char_budget_relaxed = false;
    }

    fn finish_accept(
        &mut self,
        consumed_past_deadline: bool,
    ) -> bool {
        self.budget_enforce_pending = false;
        self.budget_force_close_pending = false;
        if consumed_past_deadline && !self.budget_exceeded_warned {
            self.budget_exceeded_warned = true;
            warn(
                "The token budget (max_tokens) of a rule was exceeded: the rule could not end at the position where \
                 its budget ran out, so the budget is relaxed until the rule can end. This warning is reported once \
                 per matcher.",
            );
        }
        self.finish_character_budget_accept();
        true
    }

    /// Whether the bytes consumed by this expired suffix/stop occurrence form a complete match
    /// of its body, so the budget may end it without consuming the marker.
    fn can_force_complete_without_marker(
        &mut self,
        state: &ParserState,
        use_char_budget: bool,
    ) -> bool {
        let expired = if use_char_budget {
            self.parser.is_char_expired_state(state)
        } else {
            self.parser.is_expired_state(state)
        };
        if !expired || state.rule_id < 0 {
            return false;
        }
        let grammar = Arc::clone(&self.parser.grammar);
        let rule = grammar.rule(state.rule_id);
        if if use_char_budget {
            rule.max_chars < 0
        } else {
            rule.max_tokens < 0
        } {
            return false;
        }
        let Some(info) = grammar.suffix_stop_info(state.rule_id).filter(|info| info.body_rule_id >= 0) else {
            return false;
        };
        debug_assert!(self.should_track_accepted_bytes());
        let start_row = if state.rule_start_pos == ParserState::NO_PREV_INPUT_POS {
            0
        } else {
            state.rule_start_pos
        };
        let begin = if (start_row as usize) < self.row_byte_end.len() {
            self.row_byte_end[start_row as usize]
        } else {
            debug_assert!(self.temporary_input_start_row >= 0 && start_row >= self.temporary_input_start_row);
            (self.accepted_bytes.len() as i64) + i64::from(start_row - self.temporary_input_start_row)
        };
        let end = (self.accepted_bytes.len() + self.temporary_input_bytes.len()) as i64;
        let body_fsm = grammar.per_rule_fsm(info.body_rule_id).expect("per-rule FSM").fsm();

        let initial_progress = || {
            let mut states = HashSet::from([body_fsm.start()]);
            body_fsm.fsm().epsilon_closure(&mut states);
            BudgetBodyMatchProgress {
                begin_byte: begin,
                end_byte: begin,
                states,
                temporary_input_snapshot: Vec::new(),
            }
        };
        let accepted_len = self.accepted_bytes.len() as i64;
        let progress = self
            .budget_body_match_cache
            .entry(occurrence_key(state.rule_id, state.rule_start_pos))
            .or_insert_with(initial_progress);
        if progress.begin_byte != begin
            || progress.end_byte > end
            || (progress.end_byte > accepted_len && progress.temporary_input_snapshot != self.temporary_input_bytes)
        {
            // Only expected after restoring external matcher state; reinitialize defensively.
            *progress = initial_progress();
        }
        let mut next_states = HashSet::new();
        let mut offset = progress.end_byte;
        while offset < end && !progress.states.is_empty() {
            let byte = if offset < accepted_len {
                self.accepted_bytes[offset as usize]
            } else {
                self.temporary_input_bytes[(offset - accepted_len) as usize]
            };
            body_fsm.fsm().advance_char(&progress.states, i32::from(byte), &mut next_states);
            std::mem::swap(&mut progress.states, &mut next_states);
            offset += 1;
        }
        progress.end_byte = end;
        progress.temporary_input_snapshot = if end > accepted_len {
            self.temporary_input_bytes.clone()
        } else {
            Vec::new()
        };
        progress.states.iter().any(|&state_id| body_fsm.is_end_state(state_id))
    }

    /// Commits the current budget boundary: discards expired states and completes eligible
    /// suffix/stop occurrences without their marker. `use_char_budget` selects the character
    /// budget instead of the token budget. Returns whether a derivation remains viable.
    fn apply_budget_enforcement_impl(
        &mut self,
        use_char_budget: bool,
    ) -> bool {
        debug_assert!(self.parser.tmp_process_state_queue.is_empty());
        let previous_row = self.parser.latest_row();
        let is_expired = |parser: &EarleyParser, state: &ParserState| {
            if use_char_budget {
                parser.is_char_expired_state(state)
            } else {
                parser.is_expired_state(state)
            }
        };

        self.parser.tmp_states_visited_in_queue.clear();
        self.parser.tmp_states_to_be_added.clear();
        self.parser.tmp_completed_lazy_occurrences.clear();
        self.parser.tmp_accept_stop_token = self.parser.is_completed();

        let mut force_completed_states = Vec::new();
        let mut force_completed_occurrences = HashSet::new();
        for state in &previous_row.states {
            if !is_expired(&self.parser, state) {
                self.parser.enqueue_without_processing(*state);
                continue;
            }
            if !self.can_force_complete_without_marker(state, use_char_budget) {
                continue;
            }
            if force_completed_occurrences.insert(occurrence_key(state.rule_id, state.rule_start_pos)) {
                force_completed_states.push(*state);
            }
        }
        for state in &force_completed_states {
            self.parser.complete(state, false);
        }
        self.parser.process_queue();
        if !self.parser.tmp_completed_lazy_occurrences.is_empty() {
            self.parser.remove_committed_lazy_states();
        }

        let any_expired = self.parser.tmp_states_to_be_added.iter().any(|state| is_expired(&self.parser, state));
        let any_alive = self.parser.tmp_states_to_be_added.iter().any(|state| !is_expired(&self.parser, state));
        if any_expired && (any_alive || self.parser.tmp_accept_stop_token) {
            // Completing a budgeted suffix/stop rule can expose an already-expired parent
            // alternative: once another derivation can continue or finish at this boundary,
            // expired derivations may not consume more input.
            let mut states = std::mem::take(&mut self.parser.tmp_states_to_be_added);
            states.retain(|state| !is_expired(&self.parser, state));
            self.parser.tmp_states_to_be_added = states;
        }

        let viable = self.parser.tmp_accept_stop_token || !self.parser.tmp_states_to_be_added.is_empty();
        if !viable {
            if use_char_budget {
                // Completions above may have extended the latest completable and capture rows.
                self.parser.replace_latest_row(&ParserRow {
                    completed: self.parser.is_completed(),
                    ..previous_row
                });
            }
            return false;
        }
        let row = ParserRow {
            states: std::mem::take(&mut self.parser.tmp_states_to_be_added),
            completed: self.parser.tmp_accept_stop_token,
            ..self.parser.latest_row()
        };
        self.parser.replace_latest_row(&row);
        true
    }

    fn apply_budget_enforcement(&mut self) -> bool {
        self.apply_budget_enforcement_impl(false)
    }

    /// Reapplies character-budget enforcement for newly exposed parent occurrences.
    fn apply_character_budget_enforcement_to_fixed_point(&mut self) -> bool {
        let mut applied_any = false;
        for _ in 0..=self.parser.grammar.num_rules() {
            let has_expired_state = self.has_char_expired_state();
            if !has_expired_state || !self.apply_budget_enforcement_impl(true) {
                return applied_any;
            }
            applied_any = true;
        }
        applied_any
    }

    fn has_char_expired_state(&self) -> bool {
        self.parser.scanable_state_history.back().iter().any(|state| self.parser.is_char_expired_state(state))
    }

    fn advance_with_character_budget(
        &mut self,
        byte: u8,
    ) -> bool {
        if !self.parser.has_char_budget_rules || !EarleyParser::starts_utf8_codepoint(byte) {
            return self.parser.advance(byte);
        }
        if !self.has_char_expired_state() {
            return self.parser.advance(byte);
        }
        let snapshot = self.parser.latest_row();
        let enforced = self.apply_character_budget_enforcement_to_fixed_point();
        if !enforced && self.record_char_budget_relaxation {
            self.char_budget_relaxed = true;
        }
        let accepted = self.parser.advance(byte);
        if !accepted && enforced {
            self.parser.replace_latest_row(&snapshot);
        }
        accepted
    }

    fn advance_atomic_token_with_character_budget(
        &mut self,
        token_id: i32,
        token_char_count: i32,
    ) -> bool {
        let mut has_expired_state = false;
        let mut crosses_budget = false;
        if self.parser.has_char_budget_rules && token_char_count > 0 {
            let current_char_index = self.parser.current_char_index();
            for state in self.parser.scanable_state_history.back() {
                has_expired_state = has_expired_state || self.parser.is_char_expired_state(state);
                crosses_budget = crosses_budget
                    || (state.char_budget_deadline >= 0
                        && current_char_index + token_char_count > state.char_budget_deadline);
            }
        }
        let mut snapshot = None;
        let mut enforced = false;
        if has_expired_state {
            snapshot = Some(self.parser.latest_row());
            enforced = self.apply_character_budget_enforcement_to_fixed_point();
            if !enforced && self.record_char_budget_relaxation {
                self.char_budget_relaxed = true;
            }
        }
        let accepted = self.parser.advance_atomic_token(token_id, token_char_count);
        if !accepted && enforced {
            self.parser.replace_latest_row(snapshot.as_ref().expect("snapshot taken before enforcement"));
        }
        if accepted && crosses_budget && self.record_char_budget_relaxation {
            self.char_budget_relaxed = true;
        }
        accepted
    }

    fn advance_byte(
        &mut self,
        byte: u8,
    ) -> bool {
        if self.parser.has_char_budget_rules {
            self.advance_with_character_budget(byte)
        } else {
            self.parser.advance(byte)
        }
    }

    /// The sampling temperature of the current position: the maximum temperature over the
    /// active rules of all live paths, or the default temperature if none specifies one.
    #[must_use]
    pub fn temperature(&self) -> Option<f32> {
        let mut syntax_temperature: Option<f32> = None;
        let mut has_temperature_conflict = false;
        for state in self.parser.scanable_state_history.back() {
            if state.active_temperature_rule_id == -1 {
                continue;
            }
            let temperature = self
                .parser
                .grammar
                .rule(state.active_temperature_rule_id)
                .temperature
                .expect("active temperature rules specify a temperature");
            if syntax_temperature.is_some_and(|current| current != temperature) {
                has_temperature_conflict = true;
            }
            if syntax_temperature.is_none_or(|current| temperature > current) {
                syntax_temperature = Some(temperature);
            }
        }
        if has_temperature_conflict && self.has_warned_temperature_conflict.set(()).is_ok() {
            warn(&format!(
                "Multiple active grammar paths specify different temperatures. Using the maximum temperature {}.",
                syntax_temperature.unwrap_or_default()
            ));
        }
        syntax_temperature.or(self.default_temperature)
    }

    /// Accepts one token and updates the state of the matcher.
    ///
    /// When the end of the root rule is reached, the matcher can only accept the stop token.
    /// The matcher is terminated after accepting the stop token; the termination state can be
    /// canceled using [`Self::rollback`].
    pub fn accept_token(
        &mut self,
        token_id: i32,
    ) -> bool {
        if self.is_stop_token_accepted() {
            warn(&format!(
                "The matcher has terminated after accepting the stop token, but is trying to accept new token with id \
                 {token_id}."
            ));
            return false;
        }
        let vocab_size = self.tokenizer_info.vocab_size();
        if token_id < 0 || token_id >= vocab_size {
            warn(&format!("The token id {token_id} is out of range [0, {vocab_size}). Rejecting the token."));
            return false;
        }
        let is_stop_token = self.stop_token_ids.contains(&token_id);
        if !is_stop_token && self.tokenizer_info.special_token_ids().contains(&token_id) {
            // Padding ids are special ids without an entry in the decoded vocabulary.
            let decoded = self
                .tokenizer_info
                .decoded_vocab()
                .get(token_id as usize)
                .map_or_else(|| "<padding>".to_owned(), |token| String::from_utf8_lossy(token).into_owned());
            warn(&format!("GrammarMatcher cannot accept special token id {token_id}: {decoded}. Rejecting the token."));
            return false;
        }

        self.parser.current_token_index = self.token_length_history.len() as i32;
        self.char_budget_relaxed = false;
        self.record_char_budget_relaxation = false;
        let enforce_budget = self.budget_enforce_pending;
        let force_budget_close = self.budget_force_close_pending;
        if force_budget_close {
            // Validate on a copy first: a rejected token must not commit the budget-close
            // transition.
            let mut trial = self.clone();
            trial.parser.capture_recording = false;
            if !trial.apply_budget_enforcement() {
                return false;
            }
            trial.budget_enforce_pending = false;
            trial.budget_force_close_pending = false;
            if !trial.accept_token(token_id) {
                return false;
            }
        }

        // Capture events are only recorded on the definitive accept path.
        self.parser.capture_recording = self.parser.capture_tracking;
        let accepted = self.accept_token_definitive(token_id, is_stop_token, enforce_budget, force_budget_close);
        self.parser.capture_recording = false;
        self.parser.skip_expired_states = false;
        accepted
    }

    fn accept_token_definitive(
        &mut self,
        token_id: i32,
        is_stop_token: bool,
        enforce_budget: bool,
        force_budget_close: bool,
    ) -> bool {
        if force_budget_close {
            if !self.apply_budget_enforcement() {
                return false;
            }
            self.budget_enforce_pending = false;
            self.budget_force_close_pending = false;
        }
        self.parser.skip_expired_states = enforce_budget && !force_budget_close;

        // The token extends a derivation past its budget iff expired states may scan (no
        // enforcement) while some exist.
        let consumed_past_deadline = self.parser.has_budget_rules
            && !enforce_budget
            && self.parser.scanable_state_history.back().iter().any(|state| self.parser.is_expired_state(state));

        if is_stop_token {
            return self.accept_stop_token() && self.finish_accept(consumed_past_deadline);
        }

        let tokenizer = Arc::clone(&self.tokenizer_info);
        let token = &tokenizer.decoded_vocab()[token_id as usize];
        let has_char_budget_rules = self.parser.has_char_budget_rules;
        let token_char_count = if has_char_budget_rules {
            token.iter().filter(|&&byte| EarleyParser::starts_utf8_codepoint(byte)).count() as i32
        } else {
            0
        };
        let size_before_token = self.parser.rule_id_to_completable_states.len() as i32;
        let row_before_token = has_char_budget_rules.then(|| self.parser.latest_row());
        let restore_row_before_token = |matcher: &mut Self| {
            if let Some(snapshot) = &row_before_token {
                matcher.parser.replace_latest_row(snapshot);
            }
        };

        // Phase 1: the atomic token path, from the current state.
        let atomic_success = if has_char_budget_rules {
            self.advance_atomic_token_with_character_budget(token_id, token_char_count)
        } else {
            self.parser.advance_atomic_token(token_id, 0)
        };
        let mut atomic_row = None;
        if atomic_success {
            atomic_row = Some(self.parser.latest_row());
            self.parser.pop_last_states(1);
        }
        restore_row_before_token(self);

        // Phase 2: the byte-by-byte path, from the same original state.
        self.record_char_budget_relaxation = true;
        let track_temporary_input = has_char_budget_rules && self.has_budget_marker_rules;
        if track_temporary_input {
            self.temporary_input_start_row = self.parser.scanable_state_history.len() as i32 - 1;
            self.temporary_input_bytes.clear();
        }
        let mut pos = 0;
        let mut byte_path_success = true;
        for &byte in token {
            if !self.advance_byte(byte) {
                byte_path_success = false;
                break;
            }
            if track_temporary_input {
                self.temporary_input_bytes.push(byte);
            }
            pos += 1;
        }
        if track_temporary_input {
            self.temporary_input_start_row = -1;
            self.temporary_input_bytes.clear();
        }

        // Phase 3: combine the results (merge with deduplication).
        if !byte_path_success && !atomic_success {
            self.parser.pop_last_states(pos);
            restore_row_before_token(self);
            self.record_char_budget_relaxation = false;
            self.char_budget_relaxed = false;
            return false;
        }
        if atomic_success && !byte_path_success {
            self.parser.pop_last_states(pos);
            restore_row_before_token(self);
            self.char_budget_relaxed = false;
            let accepted = if has_char_budget_rules {
                self.advance_atomic_token_with_character_budget(token_id, token_char_count)
            } else {
                self.parser.advance_atomic_token(token_id, 0)
            };
            debug_assert!(accepted);
            self.token_length_history.push(1);
            if self.should_track_accepted_bytes() {
                self.append_atomic_row(token);
            }
        } else if byte_path_success && !atomic_success {
            self.token_length_history.push(token.len() as i32);
            if self.should_track_accepted_bytes() {
                self.append_per_byte_rows(token);
            }
        } else {
            let atomic_row = atomic_row.expect("atomic path succeeded");
            if token.is_empty() {
                // Zero-length token: the byte path created no position, so push the atomic one.
                let char_index = self.parser.current_char_index();
                let entered = self.parser.has_entered_char_budget();
                self.parser.scanable_state_history.push_row(&atomic_row.states);
                self.parser.rule_id_to_completable_states.push_row(&atomic_row.completable_states);
                self.parser.is_completed.push(atomic_row.completed);
                self.parser.push_capture_row(&atomic_row.capture_events);
                self.parser.push_char_count_row(char_index, entered);
                self.token_length_history.push(1);
                if self.should_track_accepted_bytes() {
                    self.append_atomic_row(token);
                }
            } else {
                self.merge_atomic_row_into_byte_path(size_before_token, atomic_row);
                self.token_length_history.push(token.len() as i32);
                if self.should_track_accepted_bytes() {
                    self.append_per_byte_rows(token);
                }
            }
        }

        if has_char_budget_rules {
            // Close exhausted occurrences at the token boundary, including after zero-byte
            // atomic tokens.
            self.apply_character_budget_enforcement_to_fixed_point();
        }
        self.record_char_budget_relaxation = false;
        self.finish_accept(consumed_past_deadline)
    }

    /// Merges the row produced by the atomic token path into the final row of the byte path.
    fn merge_atomic_row_into_byte_path(
        &mut self,
        size_before_token: i32,
        atomic_row: ParserRow,
    ) {
        let mut merged = self.parser.latest_scanable_states();
        for state in &atomic_row.states {
            if !merged.contains(state) {
                merged.push(*state);
            }
        }
        let mut merged_completable = self.parser.latest_completable_states();
        let mut merged_capture_row = self.parser.copy_last_capture_row();
        let byte_completed = self.parser.is_completed();
        let final_char_index = self.parser.current_char_index();
        let final_char_budget_entered = self.parser.has_entered_char_budget();
        self.parser.pop_last_states(1);

        for pair in &atomic_row.completable_states {
            if !merged_completable.contains(pair) {
                merged_completable.push(*pair);
            }
        }
        // In the atomic path the after-token position was `size_before_token`; in the byte path
        // it is the row being re-pushed. Remap events that started there so empty spans stay
        // empty in the byte-path numbering.
        let new_row = self.parser.rule_id_to_completable_states.len() as i32;
        for mut event in atomic_row.capture_events {
            if event.start_pos == size_before_token {
                event.start_pos = new_row;
            }
            if event.occurrence_start_pos == size_before_token {
                event.occurrence_start_pos = new_row;
            }
            for target in &mut event.stop_capture_targets {
                if target.start_pos == size_before_token {
                    target.start_pos = new_row;
                }
            }
            match merged_capture_row.iter_mut().find(|existing| {
                existing.rule_id == event.rule_id
                    && existing.start_pos == event.start_pos
                    && existing.occurrence_start_pos == event.occurrence_start_pos
            }) {
                None => merged_capture_row.push(event),
                Some(existing) => {
                    existing.hidden_suffix_bytes = existing.hidden_suffix_bytes.max(event.hidden_suffix_bytes);
                    existing.hidden_stop_bytes = existing.hidden_stop_bytes.max(event.hidden_stop_bytes);
                    for target in event.stop_capture_targets {
                        if !existing.stop_capture_targets.contains(&target) {
                            existing.stop_capture_targets.push(target);
                        }
                    }
                },
            }
        }

        self.parser.scanable_state_history.push_row(&merged);
        self.parser.rule_id_to_completable_states.push_row(&merged_completable);
        self.parser.is_completed.push(byte_completed || atomic_row.completed);
        self.parser.push_capture_row(&merged_capture_row);
        self.parser.push_char_count_row(final_char_index, final_char_budget_entered);
    }

    /// Accepts a string and updates the state of the matcher. The whole string is one step in
    /// rollback.
    pub fn accept_string(
        &mut self,
        input: &str,
    ) -> bool {
        self.accept_bytes(input.as_bytes())
    }

    /// Accepts raw bytes and updates the state of the matcher. On rejection the matcher is
    /// left unchanged.
    pub fn accept_bytes(
        &mut self,
        input: &[u8],
    ) -> bool {
        if self.is_stop_token_accepted() {
            warn(&format!(
                "The matcher has terminated after accepting the stop token, but is trying to accept new string \"{}\".",
                escape_bytes(input)
            ));
            return false;
        }
        self.parser.current_token_index = self.token_length_history.len() as i32;
        self.char_budget_relaxed = false;
        self.record_char_budget_relaxation = true;
        self.parser.capture_recording = self.parser.capture_tracking;
        let accepted = self.accept_bytes_definitive(input);
        self.parser.capture_recording = false;
        accepted
    }

    fn accept_bytes_definitive(
        &mut self,
        input: &[u8],
    ) -> bool {
        let has_char_budget_rules = self.parser.has_char_budget_rules;
        let row_before_input = has_char_budget_rules.then(|| self.parser.latest_row());
        let track_temporary_input = has_char_budget_rules && self.has_budget_marker_rules;
        if track_temporary_input {
            self.temporary_input_start_row = self.parser.scanable_state_history.len() as i32 - 1;
            self.temporary_input_bytes.clear();
        }
        for (accepted_cnt, &byte) in input.iter().enumerate() {
            if !self.advance_byte(byte) {
                self.parser.pop_last_states(accepted_cnt as i32);
                if let Some(snapshot) = &row_before_input {
                    self.parser.replace_latest_row(snapshot);
                }
                if track_temporary_input {
                    self.temporary_input_start_row = -1;
                    self.temporary_input_bytes.clear();
                }
                self.record_char_budget_relaxation = false;
                self.char_budget_relaxed = false;
                return false;
            }
            if track_temporary_input {
                self.temporary_input_bytes.push(byte);
            }
        }
        self.token_length_history.push(input.len() as i32);
        if self.should_track_accepted_bytes() {
            self.append_per_byte_rows(input);
        }
        if has_char_budget_rules && !input.is_empty() {
            // Leave the parser at the enforced boundary even when no later input is supplied.
            self.apply_character_budget_enforcement_to_fixed_point();
        }
        if track_temporary_input {
            self.temporary_input_start_row = -1;
            self.temporary_input_bytes.clear();
        }
        self.record_char_budget_relaxation = false;
        self.finish_character_budget_accept();
        true
    }

    /// Gets the set of tokens that are acceptable for the next step and stores them in row
    /// `index` of `bitmask`, a buffer of `shape` (`[get_bitmask_size(vocab_size)]` or
    /// `[batch, get_bitmask_size(vocab_size)]`). Returns whether the bitmask needs to be applied
    /// (is not all-true).
    ///
    /// # Errors
    /// Returns [`MatcherError`] after the stop token has been accepted, or for an invalid
    /// bitmask shape or index.
    pub fn fill_next_token_bitmask(
        &mut self,
        bitmask: &mut [i32],
        shape: &[i64],
        index: i32,
    ) -> Result<bool, MatcherError> {
        if self.is_stop_token_accepted() {
            return Err(MatcherError::Terminated {
                action: "find the next token mask",
            });
        }
        let vocab_size = self.tokenizer_info.vocab_size();
        let range = bitmask_row_range(shape, vocab_size, index)?;
        let row = bitmask.get_mut(range).ok_or_else(|| {
            MatcherError::InvalidArgument("The provided bitmask buffer is smaller than its shape".to_owned())
        })?;
        self.parser.current_token_index = self.token_length_history.len() as i32;
        if self.parser.has_budget_rules {
            let mut any_expired = false;
            let mut any_alive = false;
            let mut can_force_close_without_marker = false;
            for state in self.parser.latest_scanable_states() {
                if self.parser.is_expired_state(&state) {
                    any_expired = true;
                    can_force_close_without_marker = can_force_close_without_marker
                        || (self.has_budget_marker_rules && self.can_force_complete_without_marker(&state, false));
                } else {
                    any_alive = true;
                }
            }
            if can_force_close_without_marker {
                // Compute the enforcing state on a copy. The real state is committed only after
                // an allowed token is accepted.
                let mut trial = self.clone();
                trial.parser.capture_recording = false;
                let applied = trial.apply_budget_enforcement();
                if applied {
                    trial.fill_bitmask_for_states(row, false);
                }
                if applied && (trial.parser.is_completed() || row_has_any_token(row, vocab_size)) {
                    self.budget_enforce_pending = true;
                    self.budget_force_close_pending = true;
                    return Ok(!row_all_true(row, vocab_size));
                }
            }
            if any_expired && (any_alive || self.parser.is_completed()) {
                // No marker-less completion is needed: the ordinary max_tokens path.
                self.fill_bitmask_for_states(row, true);
                if row_has_any_token(row, vocab_size) || self.parser.is_completed() {
                    self.budget_enforce_pending = true;
                    self.budget_force_close_pending = false;
                    return Ok(!row_all_true(row, vocab_size));
                }
            }
            // No enforcement at this position: relax the budget for one step and report it on
            // the next accept.
            self.budget_enforce_pending = false;
            self.budget_force_close_pending = false;
        }
        self.fill_bitmask_for_states(row, false);
        Ok(!row_all_true(row, vocab_size))
    }

    /// Accepts the tokens of `adaptive_token_mask` that fit in the remaining character budget,
    /// and checks the others token by token.
    fn fill_bitmask_for_char_budget_boundary(
        &mut self,
        row: &mut [i32],
        adaptive_token_mask: &AdaptiveTokenMask,
        remaining_chars: i32,
    ) {
        let tokenizer = Arc::clone(&self.tokenizer_info);
        let token_char_counts = tokenizer.token_char_counts();
        let vocab = tokenizer.sorted_decoded_vocab();

        let mut tokens_to_check = adaptive_token_mask.uncertain_indices.clone();
        match adaptive_token_mask.store_type {
            AdaptiveTokenMaskStoreType::Accepted => {
                for &index in &adaptive_token_mask.accepted_indices {
                    if token_char_counts[index as usize] <= remaining_chars {
                        set_bit(row, vocab[index as usize].0, true);
                    } else {
                        tokens_to_check.push(index);
                    }
                }
            },
            AdaptiveTokenMaskStoreType::AcceptedBitset => {
                for (index, (token_id, _)) in vocab.iter().enumerate() {
                    if !adaptive_token_mask.accepted_bitset.get(*token_id as usize) {
                        continue;
                    }
                    if token_char_counts[index] <= remaining_chars {
                        set_bit(row, *token_id, true);
                    } else {
                        tokens_to_check.push(index as i32);
                    }
                }
            },
            AdaptiveTokenMaskStoreType::Rejected => {
                let mut blocked = adaptive_token_mask.rejected_indices.clone();
                blocked.extend_from_slice(&adaptive_token_mask.uncertain_indices);
                blocked.sort_unstable();
                for (index, (token_id, _)) in vocab.iter().enumerate() {
                    let index = index as i32;
                    if token_char_counts[index as usize] <= remaining_chars {
                        if blocked.binary_search(&index).is_err() {
                            set_bit(row, *token_id, true);
                        }
                    } else if adaptive_token_mask.rejected_indices.binary_search(&index).is_err() {
                        tokens_to_check.push(index);
                    }
                }
            },
        }
        tokens_to_check.sort_unstable();
        tokens_to_check.dedup();

        let saved_temporary_input_start_row = self.temporary_input_start_row;
        let saved_temporary_input_bytes = std::mem::take(&mut self.temporary_input_bytes);
        let all_latest_states = self.parser.latest_scanable_states();
        let all_latest_completed = self.parser.is_completed();

        self.parser.push_states_to_check(&all_latest_states, all_latest_completed);
        let initial_state_history_size = self.parser.scanable_state_history.len() as i32;
        let mut previous_index: Option<usize> = None;
        let mut previous_matched_size = 0;
        self.temporary_input_start_row = initial_state_history_size - 1;
        let mut byte_rejected_tokens = Vec::new();

        for &index in &tokens_to_check {
            let token = &vocab[index as usize].1;
            let common_prefix_length = previous_index
                .map_or(0, |previous| common_prefix_len(&vocab[previous].1, token).min(previous_matched_size));
            self.parser.pop_last_states(
                self.parser.scanable_state_history.len() as i32 - initial_state_history_size - common_prefix_length,
            );
            self.temporary_input_bytes.truncate(common_prefix_length as usize);

            let mut accepted = !token.is_empty();
            let mut matched_size = common_prefix_length;
            while (matched_size as usize) < token.len() {
                let byte = token[matched_size as usize];
                if !self.advance_with_character_budget(byte) {
                    accepted = false;
                    break;
                }
                self.temporary_input_bytes.push(byte);
                matched_size += 1;
            }
            if accepted {
                set_bit(row, vocab[index as usize].0, true);
            } else {
                byte_rejected_tokens.push(index);
            }
            previous_index = Some(index as usize);
            previous_matched_size = matched_size;
        }
        self.parser.pop_last_states(self.parser.scanable_state_history.len() as i32 - initial_state_history_size + 1);

        for &index in &byte_rejected_tokens {
            self.parser.push_states_to_check(&all_latest_states, all_latest_completed);
            self.temporary_input_start_row = self.parser.scanable_state_history.len() as i32 - 1;
            self.temporary_input_bytes.clear();
            let (token_id, _) = vocab[index as usize];
            if self.advance_atomic_token_with_character_budget(token_id, token_char_counts[index as usize]) {
                set_bit(row, token_id, true);
                self.parser.pop_last_states(1);
            }
            self.parser.pop_last_states(1);
        }

        self.temporary_input_start_row = saved_temporary_input_start_row;
        self.temporary_input_bytes = saved_temporary_input_bytes;
    }

    /// Fills `row` from the current states, optionally excluding the states whose budget
    /// deadline has passed.
    fn fill_bitmask_for_states(
        &mut self,
        row: &mut [i32],
        skip_expired: bool,
    ) {
        let tokenizer = Arc::clone(&self.tokenizer_info);
        let sorted_decoded_vocab = tokenizer.sorted_decoded_vocab();
        let subtree_range = tokenizer.trie_subtree_nodes_range();
        let token_char_counts = tokenizer.token_char_counts();
        let cache = Arc::clone(&self.adaptive_token_mask_cache);
        let has_char_budget_rules = self.parser.has_char_budget_rules;

        // A copy, because the scanable history changes while filling the mask.
        let latest_states: Vec<ParserState> = self
            .parser
            .scanable_state_history
            .back()
            .iter()
            .filter(|state| !(skip_expired && self.parser.is_expired_state(state)))
            .copied()
            .collect();

        // The final accepted token set is the union of the accepted sets of all states; the
        // final rejected set is the intersection of the rejected sets. Rejected indices refer to
        // the sorted decoded vocabulary. The accepted set is built directly in `row` (upstream
        // builds it in a scratch bitset and copies it over).
        row.fill(0);
        // {-1} means the universal set, i.e. all tokens initially.
        self.tmp_rejected_indices.clear();
        self.tmp_rejected_indices.push(-1);

        let mut latest_states_with_masks: Vec<(ParserState, &AdaptiveTokenMask)> = Vec::new();
        for state in &latest_states {
            let adaptive_token_mask = cache
                .get(&state.cache_key())
                .unwrap_or_else(|| panic!("the adaptive token mask cache has no entry for {state}"));
            if state.char_budget_deadline >= 0 {
                let remaining_chars = state.char_budget_deadline - self.parser.current_char_index();
                if remaining_chars <= tokenizer.max_token_chars() {
                    self.fill_bitmask_for_char_budget_boundary(row, adaptive_token_mask, remaining_chars.max(0));
                    continue;
                }
            }
            latest_states_with_masks.push((*state, adaptive_token_mask));
            match adaptive_token_mask.store_type {
                AdaptiveTokenMaskStoreType::AcceptedBitset => {
                    for (word, &bits) in row.iter_mut().zip(adaptive_token_mask.accepted_bitset.as_words()) {
                        *word |= bits as i32;
                    }
                },
                AdaptiveTokenMaskStoreType::Accepted => {
                    for &index in &adaptive_token_mask.accepted_indices {
                        set_bit(row, sorted_decoded_vocab[index as usize].0, true);
                    }
                },
                AdaptiveTokenMaskStoreType::Rejected => {},
            }
        }

        for (state, adaptive_token_mask) in latest_states_with_masks {
            self.tmp_rejected_indices_delta.clear();
            // Examine only the current state.
            self.parser.push_one_state_to_check(state);
            let track_temporary_input = has_char_budget_rules && self.has_budget_marker_rules;
            let mut saved_temporary_input = None;
            if track_temporary_input {
                saved_temporary_input =
                    Some((self.temporary_input_start_row, std::mem::take(&mut self.temporary_input_bytes)));
                self.temporary_input_start_row = self.parser.scanable_state_history.len() as i32 - 1;
            }

            let mut prev_token: Option<&[u8]> = None;
            let mut prev_matched_size = 0;
            let mut last_rejected_uncertain_range = 0;
            let uncertain_indices = &adaptive_token_mask.uncertain_indices;
            let mut position = 0;
            while position < uncertain_indices.len() {
                let cur_token_idx = uncertain_indices[position];
                position += 1;
                // Skip a token that is already accepted.
                if get_bit(row, sorted_decoded_vocab[cur_token_idx as usize].0) {
                    continue;
                }
                // Skip a token on the subtree of a rejected token.
                if cur_token_idx < last_rejected_uncertain_range {
                    if adaptive_token_mask.store_type == AdaptiveTokenMaskStoreType::Rejected {
                        self.tmp_rejected_indices_delta.push(cur_token_idx);
                    } else {
                        // Accepted masks need no per-token record for a rejected subtree.
                        let remaining = &uncertain_indices[position..];
                        let search_size =
                            remaining.len().min((last_rejected_uncertain_range - cur_token_idx - 1) as usize);
                        position +=
                            remaining[..search_size].partition_point(|&idx| idx < last_rejected_uncertain_range);
                    }
                    continue;
                }

                let cur_token = sorted_decoded_vocab[cur_token_idx as usize].1.as_slice();
                let mut accepted = !cur_token.is_empty() || !has_char_budget_rules;

                // Reuse the matched prefix shared with the previous token.
                if let Some(prev) = prev_token {
                    let lcp_len = common_prefix_len(cur_token, prev);
                    if lcp_len > prev_matched_size {
                        last_rejected_uncertain_range = subtree_range[cur_token_idx as usize];
                        accepted = false;
                    } else if lcp_len < prev_matched_size {
                        self.parser.pop_last_states(prev_matched_size - lcp_len);
                        if track_temporary_input {
                            self.temporary_input_bytes.truncate(lcp_len as usize);
                        }
                    }
                    prev_matched_size = prev_matched_size.min(lcp_len);
                }

                if accepted {
                    for (j, &byte) in cur_token.iter().enumerate().skip(prev_matched_size as usize) {
                        if !self.advance_byte(byte) {
                            last_rejected_uncertain_range = subtree_range[cur_token_idx as usize];
                            accepted = false;
                            break;
                        }
                        if track_temporary_input {
                            self.temporary_input_bytes.push(byte);
                        }
                        prev_matched_size = j as i32 + 1;
                    }
                }

                let mut retried_atomically = false;
                if !accepted && has_char_budget_rules && self.has_token_edges {
                    // Retry through Token/ExcludeToken edges in place: rebuild the single-state
                    // row, run the atomic path, then rebuild it again so trial-time enforcement
                    // does not leak.
                    retried_atomically = true;
                    self.parser.pop_last_states(prev_matched_size + 1);
                    if track_temporary_input {
                        self.temporary_input_bytes.clear();
                    }
                    self.parser.push_one_state_to_check(state);
                    let saved_capture_recording = self.parser.capture_recording;
                    let saved_record_relaxation = self.record_char_budget_relaxation;
                    self.parser.capture_recording = false;
                    self.record_char_budget_relaxation = false;
                    accepted = self.advance_atomic_token_with_character_budget(
                        sorted_decoded_vocab[cur_token_idx as usize].0,
                        token_char_counts[cur_token_idx as usize],
                    );
                    self.parser.capture_recording = saved_capture_recording;
                    self.record_char_budget_relaxation = saved_record_relaxation;
                    self.parser.pop_last_states(if accepted {
                        2
                    } else {
                        1
                    });
                    self.parser.push_one_state_to_check(state);
                    prev_matched_size = 0;
                    if accepted {
                        last_rejected_uncertain_range = cur_token_idx + 1;
                    }
                }

                match adaptive_token_mask.store_type {
                    AdaptiveTokenMaskStoreType::AcceptedBitset | AdaptiveTokenMaskStoreType::Accepted => {
                        if accepted {
                            set_bit(row, sorted_decoded_vocab[cur_token_idx as usize].0, true);
                        }
                    },
                    AdaptiveTokenMaskStoreType::Rejected => {
                        if !accepted {
                            self.tmp_rejected_indices_delta.push(cur_token_idx);
                        }
                    },
                }
                // The retry dropped this token's rows, so its prefix cannot be reused.
                prev_token = if retried_atomically {
                    None
                } else {
                    Some(cur_token)
                };
            }

            self.parser.pop_last_states(prev_matched_size + 1);
            if let Some((start_row, bytes)) = saved_temporary_input {
                self.temporary_input_start_row = start_row;
                self.temporary_input_bytes = bytes;
            }
            if adaptive_token_mask.store_type == AdaptiveTokenMaskStoreType::Rejected {
                // rejected = rejected ∩ (mask.rejected ∪ delta)
                intset_union(&mut self.tmp_rejected_indices_delta, &adaptive_token_mask.rejected_indices);
                intset_intersection(&mut self.tmp_rejected_indices, &self.tmp_rejected_indices_delta);
            }
        }

        let can_reach_end = self.parser.is_completed();
        self.set_token_bitmask(row, can_reach_end, false);
    }

    /// Turns `row`, which holds the accepted tokens, into the next-token mask.
    fn set_token_bitmask(
        &mut self,
        row: &mut [i32],
        can_reach_end: bool,
        allow_special_token: bool,
    ) {
        // next = accepted when the rejected set is universal, otherwise
        // all_tokens - (rejected - accepted).
        let sorted_decoded_vocab = self.tokenizer_info.sorted_decoded_vocab();
        if self.tmp_rejected_indices.len() == 1 && self.tmp_rejected_indices[0] == -1 {
            if allow_special_token {
                for &id in self.tokenizer_info.special_token_ids() {
                    set_bit(row, id, true);
                }
            }
        } else {
            // Collect rejected - accepted while `row` still holds the accepted set.
            let mut cleared = std::mem::take(&mut self.tmp_rejected_indices_delta);
            cleared.clear();
            cleared.extend(
                self.tmp_rejected_indices
                    .iter()
                    .map(|&index| sorted_decoded_vocab[index as usize].0)
                    .filter(|&token_id| !get_bit(row, token_id)),
            );
            row.fill(-1);
            for &token_id in &cleared {
                set_bit(row, token_id, false);
            }
            self.tmp_rejected_indices_delta = cleared;
            if !allow_special_token {
                for &id in self.tokenizer_info.special_token_ids() {
                    set_bit(row, id, false);
                }
            }
        }
        // Overridden stop tokens can occur in the compiler's accepted token set; apply their
        // termination policy after constructing either representation.
        for &id in &self.stop_token_ids {
            set_bit(row, id, can_reach_end);
        }
    }

    /// Finds the jump-forward string for jump-forward decoding: the longest string that will be
    /// valid according to the current syntax. Does not change the matcher state.
    ///
    /// # Errors
    /// Returns [`MatcherError::Terminated`] after the stop token has been accepted.
    pub fn find_jump_forward_string(&mut self) -> Result<Vec<u8>, MatcherError> {
        if self.is_stop_token_accepted() {
            return Err(MatcherError::Terminated {
                action: "get the jump forward string",
            });
        }
        self.parser.current_token_index = self.token_length_history.len() as i32;
        if self.budget_force_close_pending {
            let mut trial = self.clone();
            trial.parser.capture_recording = false;
            if trial.apply_budget_enforcement() {
                trial.budget_enforce_pending = false;
                trial.budget_force_close_pending = false;
                return trial.find_jump_forward_string();
            }
        }

        let grammar = Arc::clone(&self.parser.grammar);
        let mut result = Vec::new();
        let mut num_accepted_chars = 0;
        loop {
            // The state comes to the end of the grammar.
            if self.parser.is_completed() {
                break;
            }
            // Check that for every state the next possible char is unique and the same.
            let states = self.parser.latest_scanable_states();
            let mut next_char = -1;
            let mut can_find_next_char = true;
            for state in &states {
                if self.budget_enforce_pending && self.parser.is_expired_state(state) {
                    continue;
                }
                let fsm = grammar.per_rule_fsm(state.rule_id).expect("per-rule FSM");
                for edge in fsm.fsm().fsm().state_edges(state.element_id) {
                    if !edge.is_char_range() {
                        continue;
                    }
                    if edge.min != edge.max || (next_char != -1 && next_char != edge.min) {
                        can_find_next_char = false;
                        break;
                    }
                    next_char = edge.min;
                }
            }
            if next_char == -1 || !can_find_next_char {
                break;
            }
            let next_char = next_char as u8;
            if EarleyParser::starts_utf8_codepoint(next_char)
                && states.iter().any(|state| self.parser.is_char_expired_state(state))
            {
                break;
            }
            result.push(next_char);
            self.parser.advance(next_char);
            num_accepted_chars += 1;
        }
        self.parser.pop_last_states(num_accepted_chars);
        Ok(result)
    }

    /// Rolls the matcher back by `num_tokens` accepted tokens or strings.
    ///
    /// # Errors
    /// Returns [`MatcherError::RollbackTooFar`] if `num_tokens` exceeds the saved history.
    pub fn rollback(
        &mut self,
        num_tokens: i32,
    ) -> Result<(), MatcherError> {
        if num_tokens > self.token_length_history.len() as i32 {
            return Err(MatcherError::RollbackTooFar {
                requested: num_tokens,
                available: self.token_length_history.len(),
            });
        }
        for _ in 0..num_tokens {
            let steps = self.token_length_history.pop().expect("history checked above");
            self.parser.pop_last_states(steps);
            if self.should_track_accepted_bytes() && steps > 0 {
                let new_len = self.row_byte_end.len() - steps as usize;
                self.row_byte_end.truncate(new_len);
                self.accepted_bytes.truncate(*self.row_byte_end.last().expect("byte row 0") as usize);
            }
        }
        self.budget_enforce_pending = false;
        self.budget_force_close_pending = false;
        self.char_budget_relaxed = false;
        self.record_char_budget_relaxation = false;
        self.temporary_input_start_row = -1;
        self.temporary_input_bytes.clear();
        self.budget_body_match_cache.clear();
        Ok(())
    }

    /// The captured spans recorded so far as `(capture_name, matched_bytes)` pairs, in
    /// completion order. With `deduplicate`, only the last (longest) completion of each rule
    /// occurrence is kept.
    #[must_use]
    pub fn get_captures(
        &self,
        deduplicate: bool,
    ) -> Vec<(String, Vec<u8>)> {
        let mut result = Vec::new();
        if !self.parser.capture_tracking {
            return result;
        }
        let history = &self.parser.capture_event_history;
        debug_assert_eq!(history.len(), self.row_byte_end.len());
        let grammar = &self.parser.grammar;

        // Flatten the event history, ordered by completion position and order.
        let mut events = Vec::new();
        for (row, row_events) in history.iter().enumerate() {
            for event in row_events {
                let start_row = if event.start_pos == ParserState::NO_PREV_INPUT_POS {
                    0
                } else {
                    event.start_pos
                };
                events.push(CaptureSpanEvent {
                    rule_id: event.rule_id,
                    start_row,
                    occurrence_start_pos: event.occurrence_start_pos,
                    end_row: row as i32,
                    hidden_suffix_bytes: event.hidden_suffix_bytes,
                    hidden_stop_bytes: event.hidden_stop_bytes,
                    stop_capture_targets: event.stop_capture_targets.clone(),
                    marker_present: false,
                    marker_start_byte: -1,
                });
            }
        }

        // One occurrence may complete at several candidate end positions; keep the last.
        let mut keep = vec![true; events.len()];
        if deduplicate {
            let mut last_index: HashMap<i64, usize> = HashMap::new();
            for (i, event) in events.iter().enumerate() {
                if let Some(previous) = last_index.insert(occurrence_key(event.rule_id, event.start_row), i) {
                    keep[previous] = false;
                }
            }
        }

        // Resolve every marker to a concrete byte span once.
        for (i, event) in events.iter_mut().enumerate() {
            let hidden_bytes = event.hidden_suffix_bytes.max(event.hidden_stop_bytes);
            if !keep[i] || hidden_bytes <= 0 {
                continue;
            }
            let info = grammar.suffix_stop_info(event.rule_id).expect("hidden bytes come from suffix/stop info");
            let event_start = self.row_byte_end[event.start_row as usize];
            let event_end = self.row_byte_end[event.end_row as usize];
            event.marker_present = true;
            if info.body_rule_id == -1 {
                event.marker_start_byte = event_start.max(event_end - i64::from(hidden_bytes));
                continue;
            }
            let body_fsm = grammar.per_rule_fsm(info.body_rule_id).expect("per-rule FSM").fsm();
            let marker_fsm = grammar.per_rule_fsm(info.marker_rule_id).expect("per-rule FSM").fsm();
            let body_ends = self.accepted_prefixes(body_fsm, event_start, event_end);
            let marker_starts = self.accepted_suffixes(marker_fsm, event_start, event_end);
            match (0..body_ends.len()).find(|&offset| body_ends[offset] && marker_starts[offset]) {
                Some(offset) => event.marker_start_byte = event_start + offset as i64,
                None => {
                    debug_assert!(false, "Could not recover the suffix/stop marker boundary");
                    event.marker_present = false;
                    event.marker_start_byte = -1;
                },
            }
        }

        // Associate each stop marker only with captured occurrences on its concrete parent
        // chain.
        let mut hidden_stop_spans_by_capture: HashMap<i64, Vec<(i64, i64)>> = HashMap::new();
        for (i, event) in events.iter().enumerate() {
            if !keep[i] || event.hidden_stop_bytes <= 0 || !event.marker_present {
                continue;
            }
            let event_end = self.row_byte_end[event.end_row as usize];
            if event.marker_start_byte < event_end {
                for target in &event.stop_capture_targets {
                    hidden_stop_spans_by_capture
                        .entry(occurrence_key(target.rule_id, target.start_pos))
                        .or_default()
                        .push((event.marker_start_byte, event_end));
                }
            }
        }
        for spans in hidden_stop_spans_by_capture.values_mut() {
            spans.sort_unstable();
        }

        for (i, event) in events.iter().enumerate() {
            if !keep[i] {
                continue;
            }
            let rule = grammar.rule(event.rule_id);
            let info = grammar.suffix_stop_info(event.rule_id);
            let capture_start = self.row_byte_end[event.start_row as usize];
            let mut capture_end = self.row_byte_end[event.end_row as usize];
            if let Some(info) = info.filter(|info| event.marker_present && !info.stop_capture_name.is_empty()) {
                let marker = if event.marker_start_byte < capture_end {
                    self.accepted_bytes[event.marker_start_byte as usize..capture_end as usize].to_vec()
                } else {
                    Vec::new()
                };
                result.push((info.stop_capture_name.clone(), marker));
            }
            if rule.capture_name.is_empty() {
                continue;
            }
            if event.hidden_suffix_bytes > 0 && event.marker_present {
                capture_end = event.marker_start_byte;
            }
            let mut capture = Vec::new();
            let mut cursor = capture_start;
            let mut append_bytes = |begin: i64, end: i64| {
                if begin < end {
                    capture.extend_from_slice(&self.accepted_bytes[begin as usize..end as usize]);
                }
            };
            if let Some(spans) =
                hidden_stop_spans_by_capture.get(&occurrence_key(event.rule_id, event.occurrence_start_pos))
            {
                for &(hidden_start, hidden_end) in spans {
                    if hidden_end <= capture_start {
                        continue;
                    }
                    if hidden_start >= capture_end {
                        break;
                    }
                    append_bytes(cursor, hidden_start.max(capture_start));
                    cursor = cursor.max(hidden_end.min(capture_end));
                }
            }
            append_bytes(cursor, capture_end);
            result.push((rule.capture_name.clone(), capture));
        }
        result
    }

    /// Which prefixes of `accepted_bytes[begin..end]` the leaf FSM accepts (index = length).
    fn accepted_prefixes(
        &self,
        fsm: &CompactFsmWithStartEnd,
        begin: i64,
        end: i64,
    ) -> Vec<bool> {
        let contains_end = |states: &HashSet<i32>| states.iter().any(|&state| fsm.is_end_state(state));
        let mut result = vec![false; (end - begin + 1) as usize];
        let mut states = HashSet::from([fsm.start()]);
        fsm.fsm().epsilon_closure(&mut states);
        result[0] = contains_end(&states);
        let mut next_states = HashSet::new();
        for offset in 0..(end - begin) {
            let byte = self.accepted_bytes[(begin + offset) as usize];
            fsm.fsm().advance_char(&states, i32::from(byte), &mut next_states);
            std::mem::swap(&mut states, &mut next_states);
            result[(offset + 1) as usize] = contains_end(&states);
        }
        result
    }

    /// Which suffixes of `accepted_bytes[begin..end]` the leaf FSM accepts (index = suffix
    /// start offset), computed by a reverse NFA traversal so locating a delimiter stays linear.
    fn accepted_suffixes(
        &self,
        marker_fsm: &CompactFsmWithStartEnd,
        begin: i64,
        end: i64,
    ) -> Vec<bool> {
        let fsm = marker_fsm.fsm();
        let reachable = marker_fsm.reachable_states();
        let num_states = fsm.num_states() as usize;
        let mut reverse_epsilon: Vec<Vec<i32>> = vec![Vec::new(); num_states];
        let mut reverse_char: Vec<Vec<(i32, i32, i32)>> = vec![Vec::new(); num_states];
        for &source in &reachable {
            for edge in fsm.state_edges(source) {
                if !reachable.contains(&edge.target) {
                    continue;
                }
                if edge.is_epsilon() {
                    reverse_epsilon[edge.target as usize].push(source);
                } else if edge.is_char_range() {
                    reverse_char[edge.target as usize].push((source, edge.min, edge.max));
                }
            }
        }
        let add_reverse_epsilon_closure = |states: &mut HashSet<i32>| {
            let mut queue: Vec<i32> = states.iter().copied().collect();
            let mut i = 0;
            while i < queue.len() {
                for &predecessor in &reverse_epsilon[queue[i] as usize] {
                    if states.insert(predecessor) {
                        queue.push(predecessor);
                    }
                }
                i += 1;
            }
        };

        let length = (end - begin) as usize;
        let mut result = vec![false; length + 1];
        let mut states: HashSet<i32> = (0..fsm.num_states()).filter(|&state| marker_fsm.is_end_state(state)).collect();
        add_reverse_epsilon_closure(&mut states);
        result[length] = states.contains(&marker_fsm.start());
        let mut previous_states = HashSet::new();
        for offset in (1..=length).rev() {
            previous_states.clear();
            let byte = i32::from(self.accepted_bytes[begin as usize + offset - 1]);
            for &target in &states {
                for &(source, min, max) in &reverse_char[target as usize] {
                    if min <= byte && byte <= max {
                        previous_states.insert(source);
                    }
                }
            }
            add_reverse_epsilon_closure(&mut previous_states);
            std::mem::swap(&mut states, &mut previous_states);
            result[offset - 1] = states.contains(&marker_fsm.start());
        }
        result
    }

    /// Traverses a speculative-decoding draft tree and fills one token-mask row per node (and,
    /// when given, one temperature per node; `-1` means none or not visited).
    ///
    /// The tree arrays use node indices or `-1` for no edge. The root is node zero. Returns
    /// `false` only when `time_threshold` is positive and the traversal exceeds it.
    ///
    /// # Errors
    /// Returns [`MatcherError::InvalidArgument`] when array lengths, indices, or the tree shape
    /// are invalid.
    pub fn traverse_draft_tree(
        &mut self,
        retrieve_next_token: &[i64],
        retrieve_next_sibling: &[i64],
        draft_tokens: &[i64],
        token_bitmask: &mut [i32],
        time_threshold: f64,
        mut temperatures: Option<&mut [f32]>,
    ) -> Result<bool, MatcherError> {
        let invalid = |message: String| Err(MatcherError::InvalidArgument(message));
        let num_nodes = retrieve_next_token.len();
        if retrieve_next_sibling.len() != num_nodes {
            return invalid(
                "The retrieve_next_token and retrieve_next_sibling tensors must have the same length".to_owned(),
            );
        }
        if draft_tokens.len() != num_nodes {
            return invalid("The retrieve_next_token and draft_tokens tensors must have the same length".to_owned());
        }
        let bitmask_words = get_bitmask_size(self.tokenizer_info.vocab_size()) as usize;
        if token_bitmask.len() != num_nodes * bitmask_words {
            return invalid("The token_bitmask batch size must match the number of nodes in the tree".to_owned());
        }
        if let Some(temperatures) = temperatures.as_deref_mut() {
            if temperatures.len() != num_nodes {
                return invalid("The temperatures size must match the number of nodes in the tree".to_owned());
            }
            temperatures.fill(-1.0);
        }
        if num_nodes == 0 {
            return invalid("The draft tree must not be empty".to_owned());
        }
        if retrieve_next_sibling[0] != -1 {
            return invalid("The root node must not have siblings".to_owned());
        }
        // The edges must form a tree: the root is never a target, and every other node is the
        // child or next sibling of at most one node.
        let mut in_degree = vec![0; num_nodes];
        for i in 0..num_nodes {
            for (name, value) in
                [("retrieve_next_token", retrieve_next_token[i]), ("retrieve_next_sibling", retrieve_next_sibling[i])]
            {
                if value < -1 || value >= num_nodes as i64 {
                    return invalid(format!(
                        "{name}[{i}] = {value} is out of bounds: it should be in [-1, {num_nodes})."
                    ));
                }
                if value != -1 {
                    in_degree[value as usize] += 1;
                }
            }
        }
        if in_degree[0] != 0 {
            return invalid("The root node must not be the child or sibling of a node".to_owned());
        }
        if let Some(node) = (1..num_nodes).find(|&node| in_degree[node] > 1) {
            return invalid(format!(
                "Node {node} is the target of {} retrieve_next_token / retrieve_next_sibling entries; the draft tree \
                 must not contain cycles or shared nodes.",
                in_degree[node]
            ));
        }
        self.traverse_draft_tree_recursive(
            0,
            None,
            retrieve_next_token,
            retrieve_next_sibling,
            draft_tokens,
            token_bitmask,
            bitmask_words,
            temperatures,
            time_threshold,
            Instant::now(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn traverse_draft_tree_recursive(
        &mut self,
        current: usize,
        parent: Option<usize>,
        retrieve_next_token: &[i64],
        retrieve_next_sibling: &[i64],
        draft_tokens: &[i64],
        token_bitmask: &mut [i32],
        bitmask_words: usize,
        mut temperatures: Option<&mut [f32]>,
        time_threshold: f64,
        start_time: Instant,
    ) -> Result<bool, MatcherError> {
        let accepted = if current == 0 {
            // The first token generated by the target model is always accepted.
            true
        } else {
            let parent = parent.expect("non-root draft tree nodes have a parent");
            let token = draft_tokens[current];
            let accepted = token >= 0 && token < (bitmask_words * 32) as i64 && {
                let token = token as usize;
                (token_bitmask[parent * bitmask_words + token / 32] as u32 & (1_u32 << (token % 32))) != 0
            };
            // Check the timeout for non-root nodes so the root token mask is still computed.
            if accepted && time_threshold > 0.0 && start_time.elapsed().as_secs_f64() > time_threshold {
                return Ok(false);
            }
            accepted
        };

        let row_range = current * bitmask_words..(current + 1) * bitmask_words;
        let token_accepted = accepted
            && (current == 0 || i32::try_from(draft_tokens[current]).is_ok_and(|token| self.accept_token(token)));
        if token_accepted && !self.is_terminated() {
            let shape = [(token_bitmask.len() / bitmask_words) as i64, bitmask_words as i64];
            self.fill_next_token_bitmask(token_bitmask, &shape, current as i32)?;
            if let Some(temperatures) = temperatures.as_deref_mut() {
                temperatures[current] = self.temperature().unwrap_or(-1.0);
            }
            let child = retrieve_next_token[current];
            if child != -1
                && !self.traverse_draft_tree_recursive(
                    child as usize,
                    Some(current),
                    retrieve_next_token,
                    retrieve_next_sibling,
                    draft_tokens,
                    token_bitmask,
                    bitmask_words,
                    temperatures.as_deref_mut(),
                    time_threshold,
                    start_time,
                )?
            {
                if current != 0 {
                    self.rollback(1)?;
                }
                return Ok(false);
            }
        } else {
            token_bitmask[row_range].fill(0);
        }
        if token_accepted && current != 0 {
            self.rollback(1)?;
        }

        let sibling = retrieve_next_sibling[current];
        if sibling != -1
            && !self.traverse_draft_tree_recursive(
                sibling as usize,
                parent,
                retrieve_next_token,
                retrieve_next_sibling,
                draft_tokens,
                token_bitmask,
                bitmask_words,
                temperatures,
                time_threshold,
                start_time,
            )?
        {
            return Ok(false);
        }
        Ok(true)
    }
}
