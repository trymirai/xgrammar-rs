//! The grammar matcher — the string/token-accepting front end over the Earley parser.
//!
//! This drives the parser to accept input strings and tokens and to compute the next-token
//! bitmask. Ported from `cpp/grammar_matcher.cc`. The token-mask computation here is the
//! from-scratch variant (checking each candidate token against the parser); the precomputed
//! adaptive-mask cache is a performance optimization deferred to a later milestone.

use std::{sync::Arc, time::Instant};

use super::{
    matcher_error::MatcherTerminatedError, token_bitmask::get_bitmask_size,
};
use crate::{
    functor::grammar_optimizer,
    grammar::Grammar,
    parser::{EarleyParser, ParserState},
    tokenizer::{TokenizerInfo, VocabType},
};

/// Matches input against a grammar by driving an [`EarleyParser`].
#[derive(Debug, Clone)]
pub struct GrammarMatcher {
    parser: EarleyParser,
    tokenizer_info: TokenizerInfo,
    terminate_without_stop_token: bool,
    /// Lengths of accepted strings/tokens, for rollback.
    token_length_history: Vec<i32>,
}

impl GrammarMatcher {
    /// Creates a matcher over `grammar`, optimizing it first if needed. With
    /// `terminate_without_stop_token` the matcher is considered terminated once the grammar
    /// is completed (no stop token required) — the mode used for string-acceptance testing.
    /// The tokenizer is empty, so only string acceptance is supported.
    #[must_use]
    pub fn from_grammar(
        grammar: &Grammar,
        terminate_without_stop_token: bool,
    ) -> Self {
        let empty = TokenizerInfo::new(&[], VocabType::Raw, None, None, false);
        Self::build(grammar, empty, terminate_without_stop_token)
    }

    /// Creates a matcher over `grammar` and `tokenizer_info` (terminating on the stop token).
    #[must_use]
    pub fn from_grammar_and_tokenizer(
        grammar: &Grammar,
        tokenizer_info: TokenizerInfo,
    ) -> Self {
        Self::build(grammar, tokenizer_info, false)
    }

    /// Creates a matcher over a [`CompiledGrammar`](crate::compiler::CompiledGrammar).
    #[must_use]
    pub fn from_compiled_grammar(
        compiled: &crate::compiler::CompiledGrammar,
        terminate_without_stop_token: bool,
    ) -> Self {
        Self::build(
            compiled.grammar(),
            compiled.tokenizer_info().clone(),
            terminate_without_stop_token,
        )
    }

    fn build(
        grammar: &Grammar,
        tokenizer_info: TokenizerInfo,
        terminate_without_stop_token: bool,
    ) -> Self {
        let optimized = if grammar.is_optimized() {
            grammar.clone()
        } else {
            grammar_optimizer(grammar)
        };
        let parser = EarleyParser::new(
            Arc::new(optimized),
            ParserState::invalid(),
            true,
        );
        Self {
            parser,
            tokenizer_info,
            terminate_without_stop_token,
            token_length_history: Vec::new(),
        }
    }

    /// Accepts `input` byte by byte (see [`Self::accept_bytes`]).
    pub fn accept_string(
        &mut self,
        input: &str,
    ) -> bool {
        self.accept_bytes(input.as_bytes())
    }

    /// Accepts `input` byte by byte. On rejection the parser is rolled back to its prior state
    /// and `false` is returned (the acceptance is transactional).
    pub fn accept_bytes(
        &mut self,
        input: &[u8],
    ) -> bool {
        if self.is_stop_token_accepted() {
            return false;
        }
        for (accepted_cnt, &byte) in input.iter().enumerate() {
            if !self.parser.advance(byte) {
                self.parser.pop_last_states(accepted_cnt as i32);
                return false;
            }
        }
        self.token_length_history.push(input.len() as i32);
        true
    }

    /// Accepts the token with id `token_id` (its decoded string and/or atomic-token edges).
    ///
    /// Stop tokens terminate the matcher; special and out-of-range tokens are rejected.
    pub fn accept_token(
        &mut self,
        token_id: i32,
    ) -> bool {
        if self.is_stop_token_accepted() {
            return false;
        }
        if token_id < 0 || token_id >= self.tokenizer_info.vocab_size() {
            return false;
        }
        if self.tokenizer_info.stop_token_ids().contains(&token_id) {
            return self.accept_stop_token();
        }
        if self.tokenizer_info.special_token_ids().contains(&token_id) {
            return false;
        }
        let decoded =
            self.tokenizer_info.decoded_vocab()[token_id as usize].clone();

        // Phase 1: the atomic-token path (token/exclude-token edges), captured then rolled back.
        let atomic_success = self.parser.advance_atomic_token(token_id);
        let (atomic_states, atomic_completable, atomic_completed) =
            if atomic_success {
                let s = self.parser.latest_scanable_states();
                let c = self.parser.latest_completable_states();
                let done = self.parser.is_completed();
                self.parser.pop_last_states(1);
                (s, c, done)
            } else {
                (Vec::new(), Vec::new(), false)
            };

        // Phase 2: the byte-by-byte path from the same starting state.
        let mut pos = 0;
        let mut byte_ok = true;
        for &byte in &decoded {
            if !self.parser.advance(byte) {
                byte_ok = false;
                break;
            }
            pos += 1;
        }

        // Phase 3: combine.
        if !byte_ok && !atomic_success {
            self.parser.pop_last_states(pos);
            return false;
        }
        if atomic_success && !byte_ok {
            self.parser.pop_last_states(pos);
            self.parser.advance_atomic_token(token_id);
            self.token_length_history.push(1);
        } else if byte_ok && !atomic_success {
            self.token_length_history.push(decoded.len() as i32);
        } else if decoded.is_empty() {
            // Zero-length token: the byte path created no position, so push the atomic one.
            self.parser.push_position(
                &atomic_states,
                &atomic_completable,
                atomic_completed,
            );
            self.token_length_history.push(1);
        } else {
            // Both paths succeeded: merge the atomic states into the final byte position.
            let mut merged = self.parser.latest_scanable_states();
            for s in &atomic_states {
                if !merged.contains(s) {
                    merged.push(*s);
                }
            }
            let mut merged_comp = self.parser.latest_completable_states();
            let byte_completed = self.parser.is_completed();
            self.parser.pop_last_states(1);
            for cs in &atomic_completable {
                if !merged_comp.contains(cs) {
                    merged_comp.push(*cs);
                }
            }
            self.parser.push_position(
                &merged,
                &merged_comp,
                byte_completed || atomic_completed,
            );
            self.token_length_history.push(decoded.len() as i32);
        }
        true
    }

    /// Fills `bitmask` (a `1 × get_bitmask_size(vocab)` row at `index`) with the set of tokens
    /// acceptable in the current state: bit set = allowed.
    ///
    /// Returns whether any token is masked out (some token is rejected).
    ///
    /// # Errors
    /// Returns [`MatcherTerminatedError`] after the stop token has been accepted.
    pub fn fill_next_token_bitmask(
        &mut self,
        bitmask: &mut [i32],
        index: i32,
    ) -> Result<bool, MatcherTerminatedError> {
        if self.is_stop_token_accepted() {
            return Err(MatcherTerminatedError);
        }
        let vocab_size = self.tokenizer_info.vocab_size();
        let size = get_bitmask_size(vocab_size) as usize;
        let start = index as usize * size;
        let row = &mut bitmask[start..start + size];
        row.fill(0);

        let can_reach_end = self.parser.is_completed();
        let sorted: Vec<(i32, Vec<u8>)> =
            self.tokenizer_info.sorted_decoded_vocab().to_vec();
        for (token_id, decoded) in &sorted {
            if self.token_acceptable(*token_id, decoded) {
                let id = *token_id as usize;
                row[id / 32] |= 1 << (id % 32);
            }
        }
        if can_reach_end {
            for &id in self.tokenizer_info.stop_token_ids() {
                let id = id as usize;
                row[id / 32] |= 1 << (id % 32);
            }
        }
        // A token is masked unless its bit is set; report whether anything is masked.
        Ok((0..vocab_size).any(|t| row[(t / 32) as usize] >> (t % 32) & 1 == 0))
    }

    /// Whether `token_id` (with decoded bytes `decoded`) can be accepted from the current
    /// state, leaving the parser unchanged.
    fn token_acceptable(
        &mut self,
        token_id: i32,
        decoded: &[u8],
    ) -> bool {
        if self.parser.advance_atomic_token(token_id) {
            self.parser.pop_last_states(1);
            return true;
        }
        if decoded.is_empty() {
            return false;
        }
        let mut pos = 0;
        let mut ok = true;
        for &byte in decoded {
            if !self.parser.advance(byte) {
                ok = false;
                break;
            }
            pos += 1;
        }
        self.parser.pop_last_states(pos);
        ok
    }

    /// The tokenizer info backing this matcher.
    #[must_use]
    pub fn tokenizer_info(&self) -> &TokenizerInfo {
        &self.tokenizer_info
    }

    /// Accepts the stop token if the grammar is currently completed.
    pub fn accept_stop_token(&mut self) -> bool {
        if self.terminate_without_stop_token || !self.parser.is_completed() {
            return false;
        }
        self.token_length_history.push(0);
        self.parser.set_stop_token_accepted(true);
        true
    }

    /// Whether the matcher has terminated.
    #[must_use]
    pub fn is_terminated(&self) -> bool {
        if self.terminate_without_stop_token {
            return self.parser.is_completed();
        }
        self.is_stop_token_accepted()
    }

    /// Whether the grammar is currently in a completed (acceptable-stop) state.
    #[must_use]
    pub fn is_completed(&self) -> bool {
        self.parser.is_completed()
    }

    /// Whether the stop token has been accepted.
    #[must_use]
    pub fn is_stop_token_accepted(&self) -> bool {
        self.parser.is_stop_token_accepted()
    }

    /// Resets the matcher to the initial state.
    pub fn reset(&mut self) {
        self.parser.reset();
        self.token_length_history.clear();
    }

    /// Rolls the matcher back by `num_tokens` accepted tokens/strings.
    ///
    /// # Panics
    /// Panics if `num_tokens` exceeds the saved history.
    pub fn rollback(
        &mut self,
        num_tokens: i32,
    ) {
        assert!(
            num_tokens <= self.token_length_history.len() as i32,
            "cannot rollback more tokens than are in history"
        );
        for _ in 0..num_tokens {
            let steps =
                self.token_length_history.pop().expect("history non-empty");
            self.parser.pop_last_states(steps);
        }
    }

    /// The maximum number of tokens that can be rolled back (always `-1`, i.e. unbounded —
    /// matching the C++).
    #[must_use]
    pub fn max_rollback_tokens(&self) -> i32 {
        -1
    }

    /// Returns a deep copy of the matcher with independent state.
    #[must_use]
    pub fn fork(&self) -> GrammarMatcher {
        self.clone()
    }

    /// The stop token ids of the bound tokenizer.
    #[must_use]
    pub fn stop_token_ids(&self) -> &[i32] {
        self.tokenizer_info.stop_token_ids()
    }

    /// Traverses a speculative-decoding draft tree and fills one token-mask row per node.
    ///
    /// The three tree arrays use node indices or `-1` for no edge. The root is node zero and
    /// may not have a sibling. Returns `false` only when `time_threshold` is positive and the
    /// traversal exceeds it.
    ///
    /// # Errors
    /// Returns a message when array lengths, indices, or the bitmask shape are invalid.
    pub fn traverse_draft_tree(
        &mut self,
        retrieve_next_token: &[i64],
        retrieve_next_sibling: &[i64],
        draft_tokens: &[i64],
        token_bitmask: &mut [i32],
        time_threshold: f64,
    ) -> Result<bool, String> {
        let node_count = retrieve_next_token.len();
        if node_count == 0 {
            return Err("the draft tree must not be empty".to_owned());
        }
        if retrieve_next_sibling.len() != node_count
            || draft_tokens.len() != node_count
        {
            return Err(
                "retrieve_next_token, retrieve_next_sibling, and draft_tokens must have the same length"
                    .to_owned(),
            );
        }
        if retrieve_next_sibling[0] != -1 {
            return Err("the root node must not have siblings".to_owned());
        }
        let bitmask_words =
            get_bitmask_size(self.tokenizer_info.vocab_size()) as usize;
        let expected_len = node_count
            .checked_mul(bitmask_words)
            .ok_or_else(|| "token bitmask shape overflow".to_owned())?;
        if token_bitmask.len() != expected_len {
            return Err(
                "the token_bitmask batch size and width must match the draft tree and vocabulary"
                    .to_owned(),
            );
        }
        for &index in
            retrieve_next_token.iter().chain(retrieve_next_sibling.iter())
        {
            if index < -1 || index >= node_count as i64 {
                return Err("draft tree node index is out of range".to_owned());
            }
        }

        self.traverse_draft_tree_recursive(
            0,
            None,
            retrieve_next_token,
            retrieve_next_sibling,
            draft_tokens,
            token_bitmask,
            bitmask_words,
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
        time_threshold: f64,
        start_time: Instant,
    ) -> Result<bool, String> {
        let accepted = if current == 0 {
            true
        } else {
            let parent = parent.ok_or_else(|| {
                "non-root draft tree nodes must have a parent".to_owned()
            })?;
            let token = draft_tokens[current];
            if token < 0 || token >= (bitmask_words * 32) as i64 {
                false
            } else {
                let token = token as usize;
                let parent_row = &token_bitmask
                    [parent * bitmask_words..(parent + 1) * bitmask_words];
                (parent_row[token / 32] as u32 & (1_u32 << (token % 32))) != 0
            }
        };

        if accepted
            && current != 0
            && time_threshold > 0.0
            && start_time.elapsed().as_secs_f64() > time_threshold
        {
            return Ok(false);
        }

        let row_start = current * bitmask_words;
        let row_end = row_start + bitmask_words;
        if accepted {
            let token_accepted = current == 0
                || i32::try_from(draft_tokens[current])
                    .is_ok_and(|token| self.accept_token(token));
            if token_accepted {
                if self.is_terminated() {
                    token_bitmask[row_start..row_end].fill(0);
                } else {
                    self.fill_next_token_bitmask(token_bitmask, current as i32)
                        .map_err(|error| error.to_string())?;
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
                            time_threshold,
                            start_time,
                        )?
                    {
                        if current != 0 {
                            self.rollback(1);
                        }
                        return Ok(false);
                    }
                }
                if current != 0 {
                    self.rollback(1);
                }
            } else {
                token_bitmask[row_start..row_end].fill(0);
            }
        } else {
            token_bitmask[row_start..row_end].fill(0);
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
                time_threshold,
                start_time,
            )?
        {
            return Ok(false);
        }
        Ok(true)
    }

    /// Finds the longest string of forced (uniquely-determined) next characters from the
    /// current state, without advancing the matcher (jump-forward decoding).
    ///
    /// # Errors
    /// Returns [`MatcherTerminatedError`] after the stop token has been accepted.
    pub fn find_jump_forward_string(
        &mut self
    ) -> Result<Vec<u8>, MatcherTerminatedError> {
        if self.is_stop_token_accepted() {
            return Err(MatcherTerminatedError);
        }
        let mut result: Vec<u8> = Vec::new();
        let mut num_accepted = 0;
        loop {
            if self.parser.is_completed() {
                break;
            }
            let states = self.parser.latest_scanable_states();
            let mut next_char: i32 = -1;
            let mut can_continue = true;
            for state in &states {
                let fsm = self
                    .parser
                    .grammar()
                    .per_rule_fsm(state.rule_id)
                    .expect("per-rule FSM");
                for edge in fsm.fsm().fsm().state_edges(state.element_id) {
                    if !edge.is_char_range() {
                        continue;
                    }
                    if edge.min != edge.max {
                        can_continue = false;
                        break;
                    }
                    if next_char == -1 {
                        next_char = edge.min;
                    } else if next_char != edge.min {
                        can_continue = false;
                        break;
                    }
                }
                if !can_continue {
                    break;
                }
            }
            if next_char == -1 {
                can_continue = false;
            }
            if !can_continue {
                break;
            }
            result.push(next_char as u8);
            self.parser.advance(next_char as u8);
            num_accepted += 1;
        }
        self.parser.pop_last_states(num_accepted);
        Ok(result)
    }

    /// A human-readable dump of the matcher's latest internal parser states (debugging only).
    #[must_use]
    pub fn debug_print_internal_state(&self) -> String {
        let states = self.parser.latest_scanable_states();
        let mut out = format!("Latest step: {} states [\n", states.len());
        for state in &states {
            out.push_str(&format!("{state}, \n"));
        }
        out.push(']');
        out
    }

    /// The underlying Earley parser.
    #[must_use]
    pub fn parser(&self) -> &EarleyParser {
        &self.parser
    }
}
