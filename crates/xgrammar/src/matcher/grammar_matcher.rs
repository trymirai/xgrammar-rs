//! Stateful matcher to match tokens to a BNF grammar — the core logic of grammar-guided
//! generation.
//!
//! This module implements the non-deterministic pushdown automaton (NPDA) matching algorithm
//! to match characters to a BNF grammar. It keeps track of the current state of the matching
//! process by maintaining several stacks internally as possible paths in the NPDA. It also
//! supports backtracking.
//!
//! It is particularly capable of finding the set of tokens that are acceptable for the next
//! step and storing them in a bitmask. This aids in grammar-guided generation.

use std::{sync::Arc, time::Instant};

use super::{matcher_error::MatcherTerminatedError, token_bitmask::get_bitmask_size};
use crate::{
    compiler::{AdaptiveTokenMaskCache, AdaptiveTokenMaskStoreType},
    functor::grammar_optimizer,
    grammar::Grammar,
    parser::{EarleyParser, ParserState},
    support::{DynamicBitset, common_prefix_len, intset_intersection, intset_union},
    tokenizer::{TokenizerInfo, VocabType},
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
    adaptive_token_mask_cache: Option<Arc<AdaptiveTokenMaskCache>>,
    /// Stop token ids used for termination (tokenizer defaults, or an override).
    stop_token_ids: Vec<i32>,
    terminate_without_stop_token: bool,
    /// Lengths of accepted strings/tokens, for rollback.
    token_length_history: Vec<i32>,
    /// Scratch bitset reused by the adaptive-cache fill path.
    tmp_accepted_bitset: DynamicBitset,
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

    /// Constructs a [`GrammarMatcher`] from the preprocessing result of type
    /// [`CompiledGrammar`](crate::compiler::CompiledGrammar).
    ///
    /// `compiled` is obtained through preprocessing the grammar and tokenizer.
    #[must_use]
    pub fn from_compiled_grammar(
        compiled: &crate::compiler::CompiledGrammar,
        terminate_without_stop_token: bool,
    ) -> Self {
        Self::from_compiled_grammar_with_options(compiled, None, terminate_without_stop_token)
    }

    /// Constructs a [`GrammarMatcher`] from a compiled grammar with optional stop-token override.
    ///
    /// `override_stop_tokens`, when set, must be non-empty.
    /// `max_rollback_tokens` is accepted for API parity but unused (always reports `-1`).
    ///
    /// # Panics
    /// Panics if `override_stop_tokens` is `Some` and empty.
    #[must_use]
    pub fn from_compiled_grammar_with_options(
        compiled: &crate::compiler::CompiledGrammar,
        override_stop_tokens: Option<Vec<i32>>,
        terminate_without_stop_token: bool,
    ) -> Self {
        Self::build_with_cache(
            compiled.grammar(),
            compiled.tokenizer_info_arc(),
            Some(compiled.adaptive_token_mask_cache_arc()),
            override_stop_tokens,
            terminate_without_stop_token,
        )
    }

    fn build(
        grammar: &Grammar,
        tokenizer_info: TokenizerInfo,
        terminate_without_stop_token: bool,
    ) -> Self {
        Self::build_with_cache(grammar, Arc::new(tokenizer_info), None, None, terminate_without_stop_token)
    }

    fn build_with_cache(
        grammar: &Grammar,
        tokenizer_info: Arc<TokenizerInfo>,
        adaptive_token_mask_cache: Option<Arc<AdaptiveTokenMaskCache>>,
        override_stop_tokens: Option<Vec<i32>>,
        terminate_without_stop_token: bool,
    ) -> Self {
        let optimized = if grammar.is_optimized() {
            grammar.clone()
        } else {
            grammar_optimizer(grammar)
        };
        let parser = EarleyParser::new(Arc::new(optimized), ParserState::invalid(), true);
        let stop_token_ids = match override_stop_tokens {
            Some(ids) => {
                assert!(!ids.is_empty(), "The override_stop_tokens should not be empty");
                ids
            },
            None => tokenizer_info.stop_token_ids().to_vec(),
        };
        Self {
            parser,
            tokenizer_info,
            adaptive_token_mask_cache,
            stop_token_ids,
            terminate_without_stop_token,
            token_length_history: Vec::new(),
            tmp_accepted_bitset: DynamicBitset::new(0),
        }
    }

    /// Accepts a string and updates the state of the matcher.
    ///
    /// The whole string is considered as one step in rollback. This complements
    /// [`Self::accept_token`]; [`Self::accept_token`] should always be used to accept tokens.
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

    /// Accepts one token and updates the state of the matcher.
    ///
    /// # Termination state
    ///
    /// When the end of the root rule is reached, the matcher can only accept the stop token.
    /// The matcher is terminated after accepting the stop token, i.e. no [`Self::accept_token`]
    /// or [`Self::fill_next_token_bitmask`] operations can be performed. The termination state
    /// can be canceled using [`Self::rollback`].
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
        if self.stop_token_ids.contains(&token_id) {
            return self.accept_stop_token();
        }
        if self.tokenizer_info.special_token_ids().contains(&token_id) {
            return false;
        }
        // Clone the Arc so decoded bytes can be borrowed across `&mut self.parser` uses.
        let tokenizer = Arc::clone(&self.tokenizer_info);
        let decoded = &tokenizer.decoded_vocab()[token_id as usize];

        // Phase 1: the atomic-token path (token/exclude-token edges), captured then rolled back.
        let atomic_success = self.parser.advance_atomic_token(token_id);
        let (atomic_states, atomic_completable, atomic_completed) = if atomic_success {
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
        for &byte in decoded {
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
            self.parser.push_position(&atomic_states, &atomic_completable, atomic_completed);
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
            self.parser.push_position(&merged, &merged_comp, byte_completed || atomic_completed);
            self.token_length_history.push(decoded.len() as i32);
        }
        true
    }

    /// Gets the set of tokens that are acceptable for the next step and stores them in a
    /// bitmask.
    ///
    /// The bitmask must be pre-allocated with shape `(get_bitmask_size(vocab_size),)` and dtype
    /// `i32`. Returns whether the bitmask needs to be applied (not all-true).
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

        // Prefer the adaptive token-mask cache (populated at compile time). Empty cache
        // (e.g. zero-vocab tokenizer) falls back to brute-force.
        if self.adaptive_token_mask_cache.as_ref().is_some_and(|cache| !cache.is_empty()) {
            return Ok(self.fill_next_token_bitmask_with_cache(bitmask, index));
        }

        Ok(self.fill_next_token_bitmask_brute_force(bitmask, index))
    }

    fn fill_next_token_bitmask_with_cache(
        &mut self,
        bitmask: &mut [i32],
        index: i32,
    ) -> bool {
        let cache = Arc::clone(self.adaptive_token_mask_cache.as_ref().expect("cache path requires a non-empty cache"));
        let vocab_size = self.tokenizer_info.vocab_size() as usize;
        let size = get_bitmask_size(self.tokenizer_info.vocab_size()) as usize;
        let start = index as usize * size;
        let row = &mut bitmask[start..start + size];
        row.fill(0);

        let tokenizer = Arc::clone(&self.tokenizer_info);
        let sorted = tokenizer.sorted_decoded_vocab();
        let subtree_range = tokenizer.trie_subtree_nodes_range();
        let latest_states = self.parser.latest_scanable_states();

        if self.tmp_accepted_bitset.len() != vocab_size {
            self.tmp_accepted_bitset = DynamicBitset::new(vocab_size);
        } else {
            self.tmp_accepted_bitset.reset_all();
        }
        let mut tmp_rejected_indices = vec![-1];

        struct StateMask<'a> {
            state: ParserState,
            mask: &'a crate::compiler::AdaptiveTokenMask,
        }
        let mut latest_states_with_masks: Vec<StateMask<'_>> = Vec::new();

        for state in &latest_states {
            let key = state.cache_key();
            let mask = cache.get(&key).unwrap_or_else(|| panic!("adaptive token-mask cache missing entry for {state}"));
            latest_states_with_masks.push(StateMask {
                state: *state,
                mask,
            });
            match mask.store_type {
                AdaptiveTokenMaskStoreType::AcceptedBitset => {
                    self.tmp_accepted_bitset.or_assign(&mask.accepted_bitset);
                },
                AdaptiveTokenMaskStoreType::Accepted => {
                    for &idx in &mask.accepted_indices {
                        let token_id = sorted[idx as usize].0 as usize;
                        self.tmp_accepted_bitset.set(token_id, true);
                    }
                },
                AdaptiveTokenMaskStoreType::Rejected => {},
            }
        }

        let mut tmp_rejected_indices_delta = Vec::new();

        for entry in &latest_states_with_masks {
            let mask = entry.mask;
            tmp_rejected_indices_delta.clear();
            self.parser.push_one_state_to_check(entry.state);

            let mut prev_token: Option<&[u8]> = None;
            let mut prev_matched_size = 0;
            let mut last_rejected_uncertain_range = 0;

            for &cur_token_idx in &mask.uncertain_indices {
                let token_id = sorted[cur_token_idx as usize].0 as usize;
                if self.tmp_accepted_bitset.get(token_id) {
                    continue;
                }

                if cur_token_idx < last_rejected_uncertain_range {
                    if mask.store_type == AdaptiveTokenMaskStoreType::Rejected {
                        tmp_rejected_indices_delta.push(cur_token_idx);
                    }
                    continue;
                }

                let cur_token = &sorted[cur_token_idx as usize].1;
                let mut accepted = true;

                if let Some(prev) = prev_token {
                    let lcp_len = common_prefix_len(cur_token, prev);
                    if lcp_len > prev_matched_size {
                        last_rejected_uncertain_range = subtree_range[cur_token_idx as usize];
                        accepted = false;
                    } else if lcp_len < prev_matched_size {
                        self.parser.pop_last_states(prev_matched_size - lcp_len);
                    }
                    prev_matched_size = prev_matched_size.min(lcp_len);
                }

                if accepted {
                    for (j, &byte) in cur_token.iter().enumerate().skip(prev_matched_size as usize) {
                        if !self.parser.advance(byte) {
                            last_rejected_uncertain_range = subtree_range[cur_token_idx as usize];
                            accepted = false;
                            break;
                        }
                        prev_matched_size = j as i32 + 1;
                    }
                }

                match mask.store_type {
                    AdaptiveTokenMaskStoreType::AcceptedBitset | AdaptiveTokenMaskStoreType::Accepted => {
                        if accepted {
                            self.tmp_accepted_bitset.set(token_id, true);
                        }
                    },
                    AdaptiveTokenMaskStoreType::Rejected => {
                        if !accepted {
                            tmp_rejected_indices_delta.push(cur_token_idx);
                        }
                    },
                }

                prev_token = Some(cur_token.as_slice());
            }

            self.parser.pop_last_states(prev_matched_size + 1);

            if mask.store_type == AdaptiveTokenMaskStoreType::Rejected {
                intset_union(&mut tmp_rejected_indices_delta, &mask.rejected_indices);
                intset_intersection(&mut tmp_rejected_indices, &tmp_rejected_indices_delta);
            }
        }

        let can_reach_end = self.parser.is_completed();
        set_token_bitmask(
            row,
            &self.tmp_accepted_bitset,
            &tmp_rejected_indices,
            can_reach_end,
            &self.tokenizer_info,
            &self.stop_token_ids,
            false,
        );

        (0..self.tokenizer_info.vocab_size()).any(|t| row[(t / 32) as usize] >> (t % 32) & 1 == 0)
    }

    fn fill_next_token_bitmask_brute_force(
        &mut self,
        bitmask: &mut [i32],
        index: i32,
    ) -> bool {
        let vocab_size = self.tokenizer_info.vocab_size();
        let size = get_bitmask_size(vocab_size) as usize;
        let start = index as usize * size;
        let row = &mut bitmask[start..start + size];
        row.fill(0);

        let can_reach_end = self.parser.is_completed();
        let tokenizer = Arc::clone(&self.tokenizer_info);
        for (token_id, decoded) in tokenizer.sorted_decoded_vocab() {
            if self.token_acceptable(*token_id, decoded) {
                let id = *token_id as usize;
                row[id / 32] |= 1 << (id % 32);
            }
        }
        if can_reach_end {
            for &id in &self.stop_token_ids {
                let id = id as usize;
                row[id / 32] |= 1 << (id % 32);
            }
        }
        (0..vocab_size).any(|t| row[(t / 32) as usize] >> (t % 32) & 1 == 0)
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

    /// Checks if the matcher has accepted the stop token and terminated.
    ///
    /// See [`Self::accept_token`].
    #[must_use]
    pub fn is_terminated(&self) -> bool {
        if self.terminate_without_stop_token {
            return self.parser.is_completed();
        }
        self.is_stop_token_accepted()
    }

    /// Checks if the grammar's root rule has been fully matched by the input accepted so far.
    ///
    /// Unlike [`Self::is_terminated`], this does not require the stop token to have been accepted.
    /// See [`Self::is_terminated`] and [`Self::accept_token`].
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

    /// Rolls the matcher back to a previous state.
    ///
    /// `num_tokens` cannot exceed the current number of steps, nor can it exceed the specified
    /// maximum number of rollback tokens.
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
            let steps = self.token_length_history.pop().expect("history non-empty");
            self.parser.pop_last_states(steps);
        }
    }

    /// Returns the maximum number of rollback tokens allowed (`-1` means unbounded).
    #[must_use]
    pub fn max_rollback_tokens(&self) -> i32 {
        -1
    }

    /// Forks the matcher.
    ///
    /// Returns a new [`GrammarMatcher`] with a deep copy of all state except the compiled
    /// grammar and tokenizer info, which are shared with this matcher.
    #[must_use]
    pub fn fork(&self) -> GrammarMatcher {
        self.clone()
    }

    /// The stop token ids this matcher accepts as terminators.
    #[must_use]
    pub fn stop_token_ids(&self) -> &[i32] {
        &self.stop_token_ids
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
        if retrieve_next_sibling.len() != node_count || draft_tokens.len() != node_count {
            return Err(
                "retrieve_next_token, retrieve_next_sibling, and draft_tokens must have the same length".to_owned()
            );
        }
        if retrieve_next_sibling[0] != -1 {
            return Err("the root node must not have siblings".to_owned());
        }
        let bitmask_words = get_bitmask_size(self.tokenizer_info.vocab_size()) as usize;
        let expected_len =
            node_count.checked_mul(bitmask_words).ok_or_else(|| "token bitmask shape overflow".to_owned())?;
        if token_bitmask.len() != expected_len {
            return Err("the token_bitmask batch size and width must match the draft tree and vocabulary".to_owned());
        }
        for &index in retrieve_next_token.iter().chain(retrieve_next_sibling.iter()) {
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
            let parent = parent.ok_or_else(|| "non-root draft tree nodes must have a parent".to_owned())?;
            let token = draft_tokens[current];
            if token < 0 || token >= (bitmask_words * 32) as i64 {
                false
            } else {
                let token = token as usize;
                let parent_row = &token_bitmask[parent * bitmask_words..(parent + 1) * bitmask_words];
                (parent_row[token / 32] as u32 & (1_u32 << (token % 32))) != 0
            }
        };

        if accepted && current != 0 && time_threshold > 0.0 && start_time.elapsed().as_secs_f64() > time_threshold {
            return Ok(false);
        }

        let row_start = current * bitmask_words;
        let row_end = row_start + bitmask_words;
        if accepted {
            let token_accepted =
                current == 0 || i32::try_from(draft_tokens[current]).is_ok_and(|token| self.accept_token(token));
            if token_accepted {
                if self.is_terminated() {
                    token_bitmask[row_start..row_end].fill(0);
                } else {
                    self.fill_next_token_bitmask(token_bitmask, current as i32).map_err(|error| error.to_string())?;
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

    /// Finds the jump-forward string for jump-forward decoding.
    ///
    /// This is the longest string that will be valid according to the current syntax.
    ///
    /// This method does not change the grammar state.
    ///
    /// # Errors
    /// Returns [`MatcherTerminatedError`] after the stop token has been accepted.
    pub fn find_jump_forward_string(&mut self) -> Result<Vec<u8>, MatcherTerminatedError> {
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
                let fsm = self.parser.grammar().per_rule_fsm(state.rule_id).expect("per-rule FSM");
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

fn set_token_bitmask(
    row: &mut [i32],
    accepted_bitset: &DynamicBitset,
    rejected_indices: &[i32],
    can_reach_end: bool,
    tokenizer_info: &TokenizerInfo,
    stop_token_ids: &[i32],
    allow_special_token: bool,
) {
    let vocab_size = tokenizer_info.vocab_size() as usize;
    let sorted = tokenizer_info.sorted_decoded_vocab();

    if rejected_indices.len() == 1 && rejected_indices[0] == -1 {
        for token_id in 0..vocab_size {
            if accepted_bitset.get(token_id) {
                row[token_id / 32] |= 1 << (token_id % 32);
            }
        }
        if allow_special_token {
            for &id in tokenizer_info.special_token_ids() {
                let id = id as usize;
                row[id / 32] |= 1 << (id % 32);
            }
        }
        if can_reach_end {
            for &id in stop_token_ids {
                let id = id as usize;
                row[id / 32] |= 1 << (id % 32);
            }
        }
        return;
    }

    for token_id in 0..vocab_size {
        row[token_id / 32] = -1;
    }
    let last_word = vocab_size / 32;
    let remaining = vocab_size % 32;
    if remaining != 0 {
        row[last_word] = ((1u32 << remaining) - 1) as i32;
    }

    for &idx in rejected_indices {
        let token_id = sorted[idx as usize].0 as usize;
        if !accepted_bitset.get(token_id) {
            row[token_id / 32] &= !(1 << (token_id % 32));
        }
    }
    if !allow_special_token {
        for &id in tokenizer_info.special_token_ids() {
            let id = id as usize;
            row[id / 32] &= !(1 << (id % 32));
        }
    }
    if !can_reach_end {
        for &id in stop_token_ids {
            let id = id as usize;
            row[id / 32] &= !(1 << (id % 32));
        }
    }
}
