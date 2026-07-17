//! Adaptive token-mask cache builder.

use std::sync::Arc;

use super::{
    adaptive_token_mask::AdaptiveTokenMask, rule_level_cache::RuleLevelCache,
    tag_dispatch_optimization::TagDispatchSecondSlicingBitsets,
};
use crate::{
    functor::grammar_fsm_hash_sequence,
    grammar::{Grammar, GrammarExprType, NO_EXPR},
    parser::{EarleyParser, ParserState},
    support::{DynamicBitset, common_prefix_len, hash_combine, intset_complement, intset_difference, intset_union},
    tokenizer::TokenizerInfo,
};

type FirstCharMask = [bool; 256];

/// Earley parser wrapper that builds one [`AdaptiveTokenMask`] for a fixed initial state.
pub struct GrammarMatcherForTokenMaskCache {
    parser: EarleyParser,
    init_rule_id: i32,
    initial_state: ParserState,
    tag_dispatch_rule_id_to_second_slicing_bitset: Arc<TagDispatchSecondSlicingBitsets>,
    grammar: Arc<Grammar>,
    tokenizer_info: Arc<TokenizerInfo>,
    rule_level_cache: Option<Arc<RuleLevelCache>>,
    tmp_accepted_indices: Vec<i32>,
    tmp_rejected_indices: Vec<i32>,
    tmp_uncertain_indices: Vec<i32>,
    tmp_rejected_by_lookahead_indices: Vec<i32>,
    tmp_accepted_by_lookahead_indices: Vec<i32>,
    tmp_can_reach_end_stack: Vec<bool>,
    tmp_can_reach_end_prefix_or_stack: Vec<bool>,
    tmp_token_edge_accepted: Vec<i32>,
    tmp_token_edge_excluded: Vec<i32>,
}

impl GrammarMatcherForTokenMaskCache {
    /// Creates a matcher seeded at `init_state` without expanding the initial state.
    #[must_use]
    pub fn new(
        grammar: Arc<Grammar>,
        init_state: ParserState,
        tag_dispatch_rule_id_to_second_slicing_bitset: Arc<TagDispatchSecondSlicingBitsets>,
        tokenizer_info: Arc<TokenizerInfo>,
        rule_level_cache: Option<Arc<RuleLevelCache>>,
    ) -> Self {
        let init_rule_id = init_state.rule_id;
        Self {
            parser: EarleyParser::new(Arc::clone(&grammar), init_state, false),
            init_rule_id,
            initial_state: init_state,
            tag_dispatch_rule_id_to_second_slicing_bitset,
            grammar,
            tokenizer_info,
            rule_level_cache,
            tmp_accepted_indices: Vec::new(),
            tmp_rejected_indices: Vec::new(),
            tmp_uncertain_indices: Vec::new(),
            tmp_rejected_by_lookahead_indices: Vec::new(),
            tmp_accepted_by_lookahead_indices: Vec::new(),
            tmp_can_reach_end_stack: Vec::new(),
            tmp_can_reach_end_prefix_or_stack: Vec::new(),
            tmp_token_edge_accepted: Vec::new(),
            tmp_token_edge_excluded: Vec::new(),
        }
    }

    /// Builds the adaptive token mask for the seeded parser state.
    #[must_use]
    pub fn get_adaptive_token_mask(
        &mut self,
        is_root_rule: bool,
    ) -> AdaptiveTokenMask {
        self.tmp_accepted_indices.clear();
        self.tmp_rejected_indices.clear();
        self.tmp_uncertain_indices.clear();
        self.tmp_rejected_by_lookahead_indices.clear();
        self.tmp_accepted_by_lookahead_indices.clear();
        self.tmp_can_reach_end_prefix_or_stack.clear();
        self.tmp_can_reach_end_stack.clear();
        self.tmp_can_reach_end_stack.push(false);
        self.tmp_can_reach_end_prefix_or_stack.push(false);

        let rule_level_cache_is_available =
            self.rule_level_cache.is_some() && self.grammar.per_rule_fsm_hash(self.init_rule_id).is_some();
        let mut fsm_hash = None;
        let mut new_state_id = -1;
        let mut crossing_cache: Option<AdaptiveTokenMask>;
        let lookahead_id = self.grammar.rule(self.init_rule_id).lookahead_assertion_id;
        let is_exact_lookahead = self.grammar.rule(self.init_rule_id).is_exact_lookahead;
        let mut lookahead_hash = None;
        if rule_level_cache_is_available {
            lookahead_hash = grammar_fsm_hash_sequence(&self.grammar, lookahead_id);
            let original_to_new_id = self.grammar.per_rule_fsm_new_state_ids(self.init_rule_id);
            fsm_hash = self.grammar.per_rule_fsm_hash(self.init_rule_id);
            for &(original, new_id) in original_to_new_id {
                if original == self.initial_state.element_id {
                    new_state_id = new_id;
                    break;
                }
            }
            debug_assert_ne!(new_state_id, -1);
            let fsm = self.grammar.per_rule_fsm(self.init_rule_id).expect("per-rule FSM");
            let rule_level_cache = self.rule_level_cache.as_ref().expect("rule level cache");
            if let Some(lookahead_hash_value) = lookahead_hash {
                crossing_cache = rule_level_cache.get_cache(
                    hash_combine(&[fsm_hash.expect("fsm hash"), lookahead_hash_value, u64::from(is_exact_lookahead)]),
                    new_state_id,
                    fsm.node_num(),
                    fsm.edge_num(),
                );
                if let Some(cache) = crossing_cache {
                    return cache;
                }
            }
            crossing_cache =
                rule_level_cache.get_cache(fsm_hash.expect("fsm hash"), new_state_id, fsm.node_num(), fsm.edge_num());
            if let Some(mut cache) = crossing_cache {
                self.adapt_cache_with_lookahead(&mut cache, is_root_rule);
                return cache;
            }
        }

        let mut first_character_mask = [false; 256];
        self.get_first_character_mask(&mut first_character_mask);

        let token_edge_accepted = self.take_token_edge_accepted_indices();

        let rejected_filled = if first_character_mask.iter().all(|&b| !b) {
            false
        } else {
            self.get_token_mask_with_first_character_check(&first_character_mask, is_root_rule, &token_edge_accepted)
        };

        if !token_edge_accepted.is_empty() {
            intset_union(&mut self.tmp_accepted_indices, &token_edge_accepted);
            intset_difference(&mut self.tmp_rejected_indices, &token_edge_accepted);
            intset_difference(&mut self.tmp_uncertain_indices, &token_edge_accepted);
        }

        let sorted = self.tokenizer_info.sorted_decoded_vocab();
        let vocab_size = self.tokenizer_info.vocab_size() as usize;
        if rejected_filled {
            let return_value = AdaptiveTokenMask::from_classifications(
                vocab_size,
                sorted,
                &self.tmp_accepted_indices,
                &self.tmp_rejected_indices,
                &self.tmp_uncertain_indices,
            );
            if rule_level_cache_is_available {
                self.store_rule_level_cache(
                    fsm_hash.expect("fsm hash"),
                    new_state_id,
                    lookahead_id,
                    is_root_rule,
                    is_exact_lookahead,
                    lookahead_hash,
                    return_value.clone(),
                    true,
                );
            }
            return_value
        } else {
            let return_value = AdaptiveTokenMask::from_accepted_and_uncertain(
                vocab_size,
                sorted,
                &self.tmp_accepted_indices,
                &self.tmp_uncertain_indices,
            );
            if rule_level_cache_is_available {
                self.store_rule_level_cache(
                    fsm_hash.expect("fsm hash"),
                    new_state_id,
                    lookahead_id,
                    is_root_rule,
                    is_exact_lookahead,
                    lookahead_hash,
                    return_value.clone(),
                    false,
                );
            }
            return_value
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn store_rule_level_cache(
        &mut self,
        fsm_hash: u64,
        new_state_id: i32,
        lookahead_id: i32,
        is_root_rule: bool,
        is_exact_lookahead: bool,
        lookahead_hash: Option<u64>,
        return_value: AdaptiveTokenMask,
        rejected_filled: bool,
    ) {
        let rule_level_cache = self.rule_level_cache.as_ref().expect("rule level cache");
        let fsm = self.grammar.per_rule_fsm(self.init_rule_id).expect("per-rule FSM");
        let sorted = self.tokenizer_info.sorted_decoded_vocab();
        let vocab_size = self.tokenizer_info.vocab_size() as usize;

        if lookahead_id == NO_EXPR && !is_root_rule {
            rule_level_cache.add_cache(fsm_hash, new_state_id, fsm.node_num(), fsm.edge_num(), return_value);
            return;
        }

        intset_union(&mut self.tmp_uncertain_indices, &self.tmp_rejected_by_lookahead_indices);
        intset_union(&mut self.tmp_uncertain_indices, &self.tmp_accepted_by_lookahead_indices);

        if rejected_filled {
            let rejected_indices_without_lookahead =
                sorted_set_difference(&self.tmp_rejected_indices, &self.tmp_rejected_by_lookahead_indices);
            let accepted_indices_without_lookahead =
                sorted_set_difference(&self.tmp_accepted_indices, &self.tmp_accepted_by_lookahead_indices);
            rule_level_cache.add_cache(
                fsm_hash,
                new_state_id,
                fsm.node_num(),
                fsm.edge_num(),
                AdaptiveTokenMask::from_classifications(
                    vocab_size,
                    sorted,
                    &accepted_indices_without_lookahead,
                    &rejected_indices_without_lookahead,
                    &self.tmp_uncertain_indices,
                ),
            );
        } else {
            let accepted_indices_without_lookahead =
                sorted_set_difference(&self.tmp_accepted_indices, &self.tmp_accepted_by_lookahead_indices);
            rule_level_cache.add_cache(
                fsm_hash,
                new_state_id,
                fsm.node_num(),
                fsm.edge_num(),
                AdaptiveTokenMask::from_accepted_and_uncertain(
                    vocab_size,
                    sorted,
                    &accepted_indices_without_lookahead,
                    &self.tmp_uncertain_indices,
                ),
            );
        }

        if let Some(lookahead_hash_value) = lookahead_hash {
            rule_level_cache.add_cache(
                hash_combine(&[fsm_hash, lookahead_hash_value, u64::from(is_exact_lookahead)]),
                new_state_id,
                fsm.node_num(),
                fsm.edge_num(),
                return_value,
            );
        }
    }

    /// Adapts a crossing rule-level cache entry with lookahead.
    fn adapt_cache_with_lookahead(
        &mut self,
        cache: &mut AdaptiveTokenMask,
        is_root_rule: bool,
    ) {
        let sorted_decoded_vocab = self.tokenizer_info.sorted_decoded_vocab();
        let subtree_nodes_range = self.tokenizer_info.trie_subtree_nodes_range();
        let rule = self.grammar.rule(self.init_rule_id);
        let is_exact_lookahead = rule.is_exact_lookahead;

        let mut prev_token: Option<&[u8]> = None;
        let mut prev_matched_size = 0;
        let mut last_rejected_range = 0;

        self.tmp_rejected_indices.clear();
        self.tmp_uncertain_indices.clear();
        self.tmp_accepted_indices.clear();

        if is_root_rule {
            self.tmp_rejected_indices = std::mem::take(&mut cache.uncertain_indices);
        } else {
            let lookahead_id = rule.lookahead_assertion_id;
            if lookahead_id == NO_EXPR {
                return;
            }
            for &uncertain_index in &cache.uncertain_indices {
                let token = &sorted_decoded_vocab[uncertain_index as usize].1;
                let mut accepted = true;

                if uncertain_index < last_rejected_range {
                    self.tmp_rejected_indices.push(uncertain_index);
                    continue;
                }

                if let Some(prev) = prev_token {
                    let lcp_len = common_prefix_len(token, prev);
                    if lcp_len > prev_matched_size {
                        accepted = false;
                    } else if lcp_len < prev_matched_size {
                        self.parser.pop_last_states(prev_matched_size - lcp_len);
                        self.tmp_can_reach_end_stack
                            .truncate(self.tmp_can_reach_end_stack.len() - (prev_matched_size - lcp_len) as usize);
                        self.tmp_can_reach_end_prefix_or_stack.truncate(
                            self.tmp_can_reach_end_prefix_or_stack.len() - (prev_matched_size - lcp_len) as usize,
                        );
                    }
                    prev_matched_size = prev_matched_size.min(lcp_len);
                }

                prev_token = Some(token);

                if accepted {
                    for (j, &byte) in token.iter().enumerate().skip(prev_matched_size as usize) {
                        if !self.parser.advance(byte) {
                            accepted = false;
                            break;
                        }
                        self.tmp_can_reach_end_stack.push(self.parser.is_completed());
                        self.tmp_can_reach_end_prefix_or_stack.push(
                            *self.tmp_can_reach_end_stack.last().expect("non-empty")
                                || *self.tmp_can_reach_end_prefix_or_stack.last().expect("non-empty"),
                        );
                        prev_matched_size = j as i32 + 1;
                    }
                }

                debug_assert!(!self.tmp_can_reach_end_prefix_or_stack.is_empty());
                let can_reach_end = *self.tmp_can_reach_end_prefix_or_stack.last().expect("non-empty");

                debug_assert!(!accepted, "all tokens are at least uncertain");
                if can_reach_end && prev_matched_size > 0 {
                    let (lookahead_accepted, lookahead_completed) = is_token_pass_lookahead_assertion(
                        &mut self.parser,
                        &self.grammar,
                        self.init_rule_id,
                        token,
                        &self.tmp_can_reach_end_stack,
                    );
                    if lookahead_accepted {
                        if lookahead_completed || !is_exact_lookahead {
                            self.tmp_uncertain_indices.push(uncertain_index);
                        } else {
                            self.tmp_accepted_indices.push(uncertain_index);
                        }
                    } else {
                        self.tmp_rejected_indices.push(uncertain_index);
                        last_rejected_range = subtree_nodes_range[uncertain_index as usize];
                    }
                } else {
                    self.tmp_rejected_indices.push(uncertain_index);
                    last_rejected_range = subtree_nodes_range[uncertain_index as usize];
                }
            }
        }

        cache.uncertain_indices = std::mem::take(&mut self.tmp_uncertain_indices);
        match cache.store_type {
            super::adaptive_token_mask::AdaptiveTokenMaskStoreType::Accepted => {
                if cache.accepted_indices.len() + self.tmp_accepted_indices.len()
                    < AdaptiveTokenMask::USE_BITSET_THRESHOLD
                {
                    intset_union(&mut cache.accepted_indices, &self.tmp_accepted_indices);
                } else {
                    cache.store_type = super::adaptive_token_mask::AdaptiveTokenMaskStoreType::AcceptedBitset;
                    let mut accepted_bitset = DynamicBitset::new(self.tokenizer_info.vocab_size() as usize);
                    for &accepted_index in &cache.accepted_indices {
                        let token_id = sorted_decoded_vocab[accepted_index as usize].0 as usize;
                        accepted_bitset.set(token_id, true);
                    }
                    for &accepted_index in &self.tmp_accepted_indices {
                        let token_id = sorted_decoded_vocab[accepted_index as usize].0 as usize;
                        accepted_bitset.set(token_id, true);
                    }
                    cache.accepted_indices.clear();
                    cache.accepted_bitset = accepted_bitset;
                }
            },
            super::adaptive_token_mask::AdaptiveTokenMaskStoreType::Rejected => {
                if cache.rejected_indices.len() + self.tmp_rejected_indices.len()
                    < AdaptiveTokenMask::USE_BITSET_THRESHOLD
                {
                    intset_union(&mut cache.rejected_indices, &self.tmp_rejected_indices);
                } else {
                    cache.store_type = super::adaptive_token_mask::AdaptiveTokenMaskStoreType::AcceptedBitset;
                    let mut accepted_bitset = DynamicBitset::new(self.tokenizer_info.vocab_size() as usize);
                    accepted_bitset.set_all();
                    for &special_index in self.tokenizer_info.special_token_ids() {
                        accepted_bitset.reset(special_index as usize);
                    }
                    for &uncertain_index in &cache.uncertain_indices {
                        let token_id = sorted_decoded_vocab[uncertain_index as usize].0 as usize;
                        accepted_bitset.reset(token_id);
                    }
                    for &rejected_index in &cache.rejected_indices {
                        let token_id = sorted_decoded_vocab[rejected_index as usize].0 as usize;
                        accepted_bitset.reset(token_id);
                    }
                    for &rejected_index in &self.tmp_rejected_indices {
                        let token_id = sorted_decoded_vocab[rejected_index as usize].0 as usize;
                        accepted_bitset.reset(token_id);
                    }
                    cache.rejected_indices.clear();
                    cache.accepted_bitset = accepted_bitset;
                }
            },
            super::adaptive_token_mask::AdaptiveTokenMaskStoreType::AcceptedBitset => {
                for &accepted_index in &self.tmp_accepted_indices {
                    let token_id = sorted_decoded_vocab[accepted_index as usize].0 as usize;
                    cache.accepted_bitset.set(token_id, true);
                }
            },
        }
    }

    fn get_speculative_calculation(&self) -> (bool, FirstCharMask) {
        let rule = self.grammar.rule(self.init_rule_id);
        let rule_body = self.grammar.expr(rule.body_expr_id);
        let fsm = self.grammar.per_rule_fsm(self.init_rule_id).expect("optimized grammar has per-rule FSMs").fsm();

        if rule_body.ty == GrammarExprType::TagDispatch {
            let mut speculative_mask = [false; 256];
            for edge in fsm.fsm().state_edges(self.initial_state.element_id) {
                if edge.target != fsm.start() {
                    continue;
                }
                if !edge.is_char_range() {
                    continue;
                }
                for ch in edge.min..=edge.max {
                    speculative_mask[ch as usize] = true;
                }
            }
            return (true, speculative_mask);
        }

        let mut can_be_applied = false;
        let mut speculative_mask = [false; 256];
        debug_assert!(
            self.initial_state.element_id < fsm.num_states(),
            "initial state's element id cannot exceed the whole FSM's number of states"
        );
        for edge in fsm.fsm().state_edges(self.initial_state.element_id) {
            if edge.is_char_range() {
                if edge.target == self.initial_state.element_id {
                    can_be_applied = true;
                    for ch in edge.min..=edge.max {
                        speculative_mask[ch as usize] = true;
                    }
                    continue;
                }
                if fsm.start() == self.initial_state.element_id {
                    for next_edge in fsm.fsm().state_edges(edge.target) {
                        let matches = next_edge.is_rule_ref() && next_edge.ref_rule_id() == self.init_rule_id
                            || next_edge.is_repeat_ref()
                                && fsm.fsm().repeat_edge_info(next_edge.aux_index()).rule_id() == self.init_rule_id;
                        if matches {
                            can_be_applied = true;
                            for ch in edge.min..=edge.max {
                                speculative_mask[ch as usize] = true;
                            }
                            break;
                        }
                    }
                }
            }
        }
        (can_be_applied, speculative_mask)
    }

    fn get_token_mask_with_first_character_check(
        &mut self,
        first_char_mask: &FirstCharMask,
        is_root_rule: bool,
        token_edge_accepted: &[i32],
    ) -> bool {
        let sorted_decoded_vocab = self.tokenizer_info.sorted_decoded_vocab();
        let subtree_nodes_range = self.tokenizer_info.trie_subtree_nodes_range();
        let mut possible_intervals = Vec::new();
        let possible_token_num =
            get_possible_token_intervals(sorted_decoded_vocab, first_char_mask, &mut possible_intervals);

        self.tmp_accepted_indices.reserve(possible_token_num as usize);
        let mut fill_reject_indices =
            (sorted_decoded_vocab.len() as i32 - possible_token_num) < AdaptiveTokenMask::USE_BITSET_THRESHOLD as i32;

        debug_assert!(
            !possible_intervals.is_empty(),
            "there should be at least one possible interval for the first character mask"
        );

        if possible_intervals[0].0 != 0 && fill_reject_indices {
            for i in 0..possible_intervals[0].0 {
                self.tmp_rejected_indices.push(i);
            }
        }

        debug_assert!(self.init_rule_id != -1 && self.grammar.per_rule_fsm(self.init_rule_id).is_some());
        let (speculative_calculation, speculative_mask) = self.get_speculative_calculation();

        let mut prev_matched_size = 0;
        let mut last_rejected_range = 0;
        let is_exact_lookahead = self.grammar.rule(self.init_rule_id).is_exact_lookahead;
        let definite_accepted_bitset = if self.grammar.expr(self.grammar.rule(self.init_rule_id).body_expr_id).ty
            == GrammarExprType::TagDispatch
        {
            debug_assert!(self.tag_dispatch_rule_id_to_second_slicing_bitset.contains_key(&self.init_rule_id));
            Some(&self.tag_dispatch_rule_id_to_second_slicing_bitset[&self.init_rule_id])
        } else {
            None
        };

        let mut prev_token: Option<&[u8]> = None;
        let mut skip_ptr = 0usize;
        let skip_size = token_edge_accepted.len();

        for (interval_idx, interval) in possible_intervals.iter().enumerate() {
            let mut i = interval.0;
            while i < interval.1 {
                while skip_ptr < skip_size && token_edge_accepted[skip_ptr] < i {
                    skip_ptr += 1;
                }
                if skip_ptr < skip_size && token_edge_accepted[skip_ptr] == i {
                    i += 1;
                    continue;
                }

                if i < last_rejected_range {
                    if fill_reject_indices {
                        self.tmp_rejected_indices.push(i);
                        fill_reject_indices = self.tmp_rejected_indices.len() < AdaptiveTokenMask::USE_BITSET_THRESHOLD;
                    } else {
                        i = last_rejected_range - 1;
                    }
                    i += 1;
                    continue;
                }

                let token = &sorted_decoded_vocab[i as usize].1;

                if speculative_calculation {
                    if let Some(definite_bitset) = definite_accepted_bitset {
                        if token.is_empty() {
                            self.tmp_accepted_indices.push(i);
                            i += 1;
                            continue;
                        }
                        if speculative_mask[token[0] as usize] && definite_bitset.get(i as usize) {
                            self.tmp_accepted_indices.push(i);
                            i += 1;
                            continue;
                        }
                    } else {
                        let all_accepted = token.iter().all(|&ch| ch.is_ascii() && speculative_mask[ch as usize]);
                        if all_accepted {
                            self.tmp_accepted_indices.push(i);
                            i += 1;
                            continue;
                        }
                    }
                }

                let mut accepted = true;
                if let Some(prev) = prev_token {
                    let lcp_len = common_prefix_len(token, prev);
                    if lcp_len > prev_matched_size {
                        accepted = false;
                    } else if lcp_len < prev_matched_size {
                        self.parser.pop_last_states(prev_matched_size - lcp_len);
                        self.tmp_can_reach_end_stack
                            .truncate(self.tmp_can_reach_end_stack.len() - (prev_matched_size - lcp_len) as usize);
                        self.tmp_can_reach_end_prefix_or_stack.truncate(
                            self.tmp_can_reach_end_prefix_or_stack.len() - (prev_matched_size - lcp_len) as usize,
                        );
                    }
                    prev_matched_size = prev_matched_size.min(lcp_len);
                }

                prev_token = Some(token);

                if accepted {
                    for (j, &byte) in token.iter().enumerate().skip(prev_matched_size as usize) {
                        if !self.parser.advance(byte) {
                            accepted = false;
                            break;
                        }
                        self.tmp_can_reach_end_stack.push(self.parser.is_completed());
                        self.tmp_can_reach_end_prefix_or_stack.push(
                            *self.tmp_can_reach_end_stack.last().expect("non-empty")
                                || *self.tmp_can_reach_end_prefix_or_stack.last().expect("non-empty"),
                        );
                        prev_matched_size = j as i32 + 1;
                    }
                }

                let can_reach_end = *self.tmp_can_reach_end_prefix_or_stack.last().expect("non-empty");

                if accepted {
                    self.tmp_accepted_indices.push(i);
                } else if can_reach_end && prev_matched_size > 0 {
                    let (lookahead_accepted, lookahead_completed) = is_token_pass_lookahead_assertion(
                        &mut self.parser,
                        &self.grammar,
                        self.init_rule_id,
                        token,
                        &self.tmp_can_reach_end_stack,
                    );
                    if !is_root_rule && lookahead_accepted {
                        if lookahead_completed || !is_exact_lookahead {
                            self.tmp_uncertain_indices.push(i);
                        } else {
                            self.tmp_accepted_indices.push(i);
                            self.tmp_accepted_by_lookahead_indices.push(i);
                        }
                    } else {
                        let end = subtree_nodes_range[i as usize];
                        for j in i..end {
                            self.tmp_rejected_indices.push(j);
                            self.tmp_rejected_by_lookahead_indices.push(j);
                        }
                        i = end - 1;
                    }
                } else {
                    self.tmp_rejected_indices.push(i);
                    last_rejected_range = subtree_nodes_range[i as usize];
                    fill_reject_indices = self.tmp_rejected_indices.len() < AdaptiveTokenMask::USE_BITSET_THRESHOLD;
                }
                i += 1;
            }

            if interval_idx + 1 != possible_intervals.len() && fill_reject_indices {
                let next_interval = &possible_intervals[interval_idx + 1];
                for j in interval.1..next_interval.0 {
                    self.tmp_rejected_indices.push(j);
                }
                fill_reject_indices = self.tmp_rejected_indices.len() < AdaptiveTokenMask::USE_BITSET_THRESHOLD;
            }
        }

        self.parser.pop_last_states(prev_matched_size);

        if possible_intervals.last().expect("non-empty").1 != sorted_decoded_vocab.len() as i32 && fill_reject_indices {
            let last_end = possible_intervals.last().expect("non-empty").1;
            for i in last_end..sorted_decoded_vocab.len() as i32 {
                self.tmp_rejected_indices.push(i);
            }
        }

        fill_reject_indices
    }

    fn get_first_character_mask(
        &self,
        first_character_mask: &mut FirstCharMask,
    ) {
        first_character_mask.fill(false);
        let fsm = self.grammar.per_rule_fsm(self.init_rule_id).expect("optimized grammar has per-rule FSMs").fsm();
        for edge in fsm.fsm().state_edges(self.initial_state.element_id) {
            if edge.is_char_range() {
                for c in edge.min..=edge.max {
                    first_character_mask[c as usize] = true;
                }
            }
        }
    }

    fn take_token_edge_accepted_indices(&mut self) -> Vec<i32> {
        self.tmp_token_edge_accepted.clear();
        self.tmp_token_edge_excluded.clear();

        let fsm = self.grammar.per_rule_fsm(self.init_rule_id).expect("optimized grammar has per-rule FSMs").fsm();
        let edges = fsm.fsm().state_edges(self.initial_state.element_id);
        let sorted_decoded_vocab = self.tokenizer_info.sorted_decoded_vocab();
        let sorted_size = sorted_decoded_vocab.len() as i32;
        let tid_to_sorted = self.tokenizer_info.token_id_to_sorted_vocab_index();
        let mut has_exclude_token = false;

        for edge in edges {
            if edge.is_token() {
                let info = fsm.fsm().token_edge_info(edge.aux_index());
                for &tid in info.token_ids() {
                    debug_assert!(tid >= 0 && (tid as usize) < tid_to_sorted.len());
                    let sorted_idx = tid_to_sorted[tid as usize];
                    if sorted_idx >= 0 {
                        self.tmp_token_edge_accepted.push(sorted_idx);
                    }
                }
            } else if edge.is_exclude_token() {
                has_exclude_token = true;
                let info = fsm.fsm().exclude_token_edge_info(edge.aux_index());
                for &tid in info.token_ids() {
                    debug_assert!(tid >= 0 && (tid as usize) < tid_to_sorted.len());
                    let sorted_idx = tid_to_sorted[tid as usize];
                    if sorted_idx >= 0 {
                        self.tmp_token_edge_excluded.push(sorted_idx);
                    }
                }
            }
        }

        if !has_exclude_token {
            if !self.tmp_token_edge_accepted.is_empty() {
                self.tmp_token_edge_accepted.sort_unstable();
                self.tmp_token_edge_accepted.dedup();
            }
            return std::mem::take(&mut self.tmp_token_edge_accepted);
        }

        if !self.tmp_token_edge_accepted.is_empty() {
            self.tmp_token_edge_accepted.sort_unstable();
            self.tmp_token_edge_accepted.dedup();
        }
        self.tmp_token_edge_excluded.sort_unstable();
        self.tmp_token_edge_excluded.dedup();
        intset_difference(&mut self.tmp_token_edge_excluded, &self.tmp_token_edge_accepted);
        self.tmp_token_edge_accepted = intset_complement(sorted_size, &self.tmp_token_edge_excluded);
        std::mem::take(&mut self.tmp_token_edge_accepted)
    }
}

/// Returns sorted-vocab index intervals whose first byte falls in `first_char_mask`.
#[must_use]
pub fn get_possible_token_intervals(
    sorted_decoded_vocab: &[(i32, Vec<u8>)],
    first_char_mask: &FirstCharMask,
    possible_intervals: &mut Vec<(i32, i32)>,
) -> i32 {
    possible_intervals.clear();
    let mut possible_token_num = 0;
    let mut matched_size = 0usize;
    let mut last_interval_end: Option<u8> = None;

    for ch in 0u8..=255 {
        if first_char_mask[ch as usize] {
            if last_interval_end.is_none() {
                last_interval_end = Some(ch);
            }
        } else if let Some(last_end) = last_interval_end.take() {
            let interval_left_end = lower_bound_sorted_vocab(sorted_decoded_vocab, matched_size, &[last_end]);
            let interval_right_end = lower_bound_sorted_vocab(sorted_decoded_vocab, interval_left_end, &[ch]);
            possible_intervals.push((interval_left_end as i32, interval_right_end as i32));
            possible_token_num += interval_right_end as i32 - interval_left_end as i32;
            matched_size = interval_right_end;
        }
    }

    if let Some(last_end) = last_interval_end {
        let interval_left_end = lower_bound_sorted_vocab(sorted_decoded_vocab, matched_size, &[last_end]);
        possible_intervals.push((interval_left_end as i32, sorted_decoded_vocab.len() as i32));
        possible_token_num += sorted_decoded_vocab.len() as i32 - interval_left_end as i32;
    }

    possible_token_num
}

fn lower_bound_sorted_vocab(
    sorted_decoded_vocab: &[(i32, Vec<u8>)],
    start: usize,
    key: &[u8],
) -> usize {
    start + sorted_decoded_vocab[start..].partition_point(|(_, token)| token.as_slice() < key)
}

fn is_token_pass_lookahead_assertion(
    parser: &mut EarleyParser,
    grammar: &Grammar,
    init_rule_id: i32,
    token: &[u8],
    can_reach_end_stack: &[bool],
) -> (bool, bool) {
    let mut accepted = true;
    let mut can_reach_end = true;
    let lookahead_assertion_id = grammar.rule(init_rule_id).lookahead_assertion_id;
    if lookahead_assertion_id == NO_EXPR {
        return (accepted, can_reach_end);
    }

    let lookahead_state = ParserState::new(-1, lookahead_assertion_id, 0, ParserState::NO_PREV_INPUT_POS, 0);
    parser.push_state_and_expand(lookahead_state);
    let token_len = token.len();

    if parser.is_completed() {
        parser.pop_last_states(1);
        return (accepted, can_reach_end);
    }

    for i in (0..can_reach_end_stack.len()).rev() {
        if !can_reach_end_stack[i] {
            continue;
        }
        let mut last_accept_pos = i as i32 - 1;
        for (pos, &byte) in token.iter().enumerate().skip(i) {
            if !parser.advance(byte) {
                break;
            }
            last_accept_pos = pos as i32;
            if parser.is_completed() {
                parser.pop_last_states(pos as i32 - i as i32 + 2);
                return (accepted, can_reach_end);
            }
        }
        if last_accept_pos == token_len as i32 - 1 {
            parser.pop_last_states(last_accept_pos - i as i32 + 2);
            can_reach_end = false;
            return (accepted, can_reach_end);
        }
        parser.pop_last_states(last_accept_pos - i as i32 + 1);
    }

    parser.pop_last_states(1);
    can_reach_end = false;
    accepted = false;
    (accepted, can_reach_end)
}

fn sorted_set_difference(
    left: &[i32],
    right: &[i32],
) -> Vec<i32> {
    let mut result = Vec::new();
    let mut left_idx = 0;
    let mut right_idx = 0;
    while left_idx < left.len() && right_idx < right.len() {
        if left[left_idx] < right[right_idx] {
            result.push(left[left_idx]);
            left_idx += 1;
        } else if left[left_idx] > right[right_idx] {
            right_idx += 1;
        } else {
            left_idx += 1;
            right_idx += 1;
        }
    }
    while left_idx < left.len() {
        result.push(left[left_idx]);
        left_idx += 1;
    }
    result
}
