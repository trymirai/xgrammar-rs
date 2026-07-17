//! Computes structural hashes for per-rule FSMs.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::allow_empty_rule_analyzer::rule_ref_graph;
use crate::{
    fsm::FsmEdge,
    grammar::{Grammar, GrammarExprType, NO_EXPR},
    support::hash_combine_binary,
};

const NOT_END_STATE_FLAG: i32 = -0x100;
const END_STATE_FLAG: i32 = -0x200;
const SELF_RECURSION_FLAG: i32 = -0x300;
const SIMPLE_CYCLE_FLAG: i32 = -0x400;
const UNKNOWN_FLAG: i32 = -0x500;

fn hash_combine(
    seed: u64,
    values: &[u64],
) -> u64 {
    let mut result = seed;
    for &value in values {
        hash_combine_binary(&mut result, value);
    }
    result
}

fn hash_combine_i32(
    seed: u64,
    values: &[i32],
) -> u64 {
    let mut result = seed;
    for &value in values {
        hash_combine_binary(&mut result, value as u64);
    }
    result
}

/// Computes per-rule FSM hashes and normalized state-id mappings on `grammar`.
pub fn apply(grammar: &mut Grammar) {
    GrammarFsmHasher::new(grammar).apply();
}

/// Hashes a sequence expression for rule-level cache lookup.
#[must_use]
pub fn hash_sequence(
    grammar: &Grammar,
    sequence_id: i32,
) -> Option<u64> {
    if sequence_id == NO_EXPR {
        return None;
    }
    let sequence_expr = grammar.expr(sequence_id);
    debug_assert_eq!(sequence_expr.ty, GrammarExprType::Sequence, "GrammarExpr is not a sequence");
    let mut hash_result = 0u64;
    for &expr_id in sequence_expr.data {
        let expr = grammar.expr(expr_id);
        hash_result = hash_combine_i32(hash_result, &[expr.ty.as_i32()]);
        match expr.ty {
            GrammarExprType::ByteString
            | GrammarExprType::CharacterClass
            | GrammarExprType::CharacterClassStar
            | GrammarExprType::EmptyStr
            | GrammarExprType::Token
            | GrammarExprType::ExcludeToken => {
                for &element in expr.data {
                    hash_result = hash_combine_i32(hash_result, &[element]);
                }
            },
            GrammarExprType::RuleRef => {
                let hash = grammar.per_rule_fsm_hash(expr.data[0])?;
                hash_result = hash_combine(hash_result, &[hash]);
            },
            GrammarExprType::Repeat => {
                let hash = grammar.per_rule_fsm_hash(expr.data[0])?;
                hash_result = hash_combine(hash_result, &[hash]);
                hash_result = hash_combine_i32(hash_result, &[expr.data[1], expr.data[2]]);
            },
            GrammarExprType::Sequence
            | GrammarExprType::Choices
            | GrammarExprType::TagDispatch
            | GrammarExprType::TokenTagDispatch => return None,
        }
    }
    Some(hash_result)
}

struct GrammarFsmHasher<'a> {
    grammar: &'a mut Grammar,
    visited: Vec<bool>,
    ref_graph_from_referrer_to_referee: Vec<Vec<i32>>,
    ref_graph_from_referee_to_referrer: Vec<Vec<i32>>,
    sorted_edges: Vec<Vec<FsmEdge>>,
    has_inward_edges: Vec<bool>,
    per_rule_fsm_hashes: Vec<Option<u64>>,
    per_rule_fsm_new_state_ids: Vec<Vec<(i32, i32)>>,
}

impl<'a> GrammarFsmHasher<'a> {
    fn new(grammar: &'a mut Grammar) -> Self {
        Self {
            grammar,
            visited: Vec::new(),
            ref_graph_from_referrer_to_referee: Vec::new(),
            ref_graph_from_referee_to_referrer: Vec::new(),
            sorted_edges: Vec::new(),
            has_inward_edges: Vec::new(),
            per_rule_fsm_hashes: Vec::new(),
            per_rule_fsm_new_state_ids: Vec::new(),
        }
    }

    fn apply(&mut self) {
        let num_rules = self.grammar.num_rules() as usize;
        self.per_rule_fsm_hashes = vec![None; num_rules];
        self.per_rule_fsm_new_state_ids = vec![Vec::new(); num_rules];
        self.visited = vec![false; num_rules];
        self.has_inward_edges = vec![false; self.grammar.complete_fsm().num_states() as usize];
        for state in 0..self.grammar.complete_fsm().num_states() {
            for edge in self.grammar.complete_fsm().state_edges(state) {
                self.has_inward_edges[edge.target as usize] = true;
            }
        }

        self.ref_graph_from_referee_to_referrer = rule_ref_graph(self.grammar);
        self.ref_graph_from_referrer_to_referee = vec![Vec::new(); num_rules];
        for (referee, referrers) in self.ref_graph_from_referee_to_referrer.iter().enumerate() {
            for &referer in referrers {
                self.ref_graph_from_referrer_to_referee[referer as usize].push(referee as i32);
            }
        }

        let complete_fsm = self.grammar.complete_fsm();
        self.sorted_edges.reserve(complete_fsm.num_states() as usize);
        for state in 0..complete_fsm.num_states() {
            let mut edges: Vec<FsmEdge> = complete_fsm.state_edges(state).to_vec();
            edges.sort_unstable();
            self.sorted_edges.push(edges);
        }

        for (i, per_rule_fsm) in self.grammar.per_rule_fsms_slice().iter().enumerate() {
            if per_rule_fsm.is_none() {
                self.visited[i] = true;
            }
        }

        let mut current_operating_index = self.find_simple_fsm_can_be_hashed();
        while current_operating_index != -1 {
            self.visited[current_operating_index as usize] = true;
            let hash_value = self.hash_fsm(current_operating_index);
            self.per_rule_fsm_hashes[current_operating_index as usize] = Some(hash_value);
            for &referer in &self.ref_graph_from_referee_to_referrer[current_operating_index as usize] {
                self.ref_graph_from_referrer_to_referee[referer as usize]
                    .retain(|&rule_id| rule_id != current_operating_index);
            }
            current_operating_index = self.find_simple_fsm_can_be_hashed();
        }

        let mut partial_hashed_list = Vec::new();
        for rule_id in 0..self.grammar.num_rules() {
            if self.per_rule_fsm_hashes[rule_id as usize].is_some() {
                continue;
            }
            if self.grammar.per_rule_fsm(rule_id).is_none() {
                continue;
            }
            let start = self.grammar.per_rule_fsm(rule_id).expect("per-rule FSM").fsm().start();
            if self.has_inward_edges[start as usize] {
                continue;
            }
            if let Some(hash_value) = self.is_partial_hashable(rule_id) {
                partial_hashed_list.push((rule_id, hash_value));
            }
        }
        for (rule_id, hash_value) in partial_hashed_list {
            self.per_rule_fsm_hashes[rule_id as usize] = Some(hash_value);
        }

        let hashes = std::mem::take(&mut self.per_rule_fsm_hashes);
        let new_state_ids = std::mem::take(&mut self.per_rule_fsm_new_state_ids);
        self.grammar.set_fsm_hash_data(hashes, new_state_ids);
    }

    fn find_simple_cycle(&mut self) -> bool {
        let mut not_simple_cycle = self.visited.clone();
        for i in 0..not_simple_cycle.len() {
            if not_simple_cycle[i] {
                continue;
            }
            let mut dfs_stack = Vec::new();
            let mut simple_cycle = Vec::new();
            let mut in_stack = vec![false; self.ref_graph_from_referee_to_referrer.len()];
            dfs_stack.push(i as i32);
            let mut current_fsm_index = i as i32;
            in_stack[current_fsm_index as usize] = true;
            while self.ref_graph_from_referrer_to_referee[current_fsm_index as usize].len() == 1
                && !not_simple_cycle[current_fsm_index as usize]
            {
                let next = self.ref_graph_from_referrer_to_referee[current_fsm_index as usize][0];
                debug_assert_ne!(
                    current_fsm_index, next,
                    "Self-recursion cycle found in the reference graph, which is not allowed."
                );
                not_simple_cycle[current_fsm_index as usize] = true;
                current_fsm_index = next;
                if in_stack[current_fsm_index as usize] {
                    simple_cycle.push(current_fsm_index);
                    while dfs_stack.last().copied() != Some(current_fsm_index) {
                        simple_cycle.push(dfs_stack.pop().expect("non-empty stack"));
                    }
                    break;
                }
                dfs_stack.push(current_fsm_index);
                in_stack[current_fsm_index as usize] = true;
            }
            if !simple_cycle.is_empty() {
                self.hash_simple_cycle(&simple_cycle);
                return true;
            }
        }
        false
    }

    fn hash_simple_cycle(
        &mut self,
        simple_cycle: &[i32],
    ) {
        for &cycle_id in simple_cycle {
            self.visited[cycle_id as usize] = true;
            self.per_rule_fsm_hashes[cycle_id as usize] = Some(SIMPLE_CYCLE_FLAG as u64);
        }

        let mut local_cycle_hash: Vec<u64> = simple_cycle.iter().map(|&cycle_id| self.hash_fsm(cycle_id)).collect();
        let local_cycle_hash_copy = local_cycle_hash.clone();
        for i in 0..local_cycle_hash.len() {
            let mut current_hash = 0u64;
            for j in 0..local_cycle_hash.len() {
                current_hash = hash_combine(current_hash, &[local_cycle_hash_copy[(i + j) % local_cycle_hash.len()]]);
            }
            local_cycle_hash[i] = current_hash;
        }

        for (i, &cycle_id) in simple_cycle.iter().enumerate() {
            self.per_rule_fsm_hashes[cycle_id as usize] = Some(local_cycle_hash[i]);
            for &referer in &self.ref_graph_from_referee_to_referrer[cycle_id as usize] {
                self.ref_graph_from_referrer_to_referee[referer as usize].retain(|&rule_id| rule_id != cycle_id);
            }
        }
    }

    fn find_simple_fsm_can_be_hashed(&mut self) -> i32 {
        loop {
            for (i, visited) in self.visited.iter().enumerate() {
                if *visited {
                    continue;
                }
                if self.ref_graph_from_referrer_to_referee[i].is_empty() {
                    return i as i32;
                }
                if self.ref_graph_from_referrer_to_referee[i].len() == 1
                    && self.ref_graph_from_referrer_to_referee[i][0] == i as i32
                {
                    return i as i32;
                }
            }
            if !self.find_simple_cycle() {
                return -1;
            }
        }
    }

    fn is_partial_hashable(
        &mut self,
        fsm_index: i32,
    ) -> Option<u64> {
        let mut hash_result = 0u64;
        let fsm = self.grammar.per_rule_fsm(fsm_index).expect("per-rule FSM").fsm();
        let mut original_state_id_to_new_id = BTreeMap::from([(fsm.start(), 0)]);
        let mut bfs_queue = VecDeque::from([fsm.start()]);
        let mut hash_and_target: BTreeSet<(u64, i32)> = BTreeSet::new();

        while let Some(current_old_state_id) = bfs_queue.pop_front() {
            let is_start = current_old_state_id == fsm.start();
            let current_new_state_id = *original_state_id_to_new_id.get(&current_old_state_id).expect("mapped");

            if fsm.is_end_state(current_old_state_id) {
                hash_result = hash_combine_i32(
                    hash_result,
                    &[current_new_state_id, END_STATE_FLAG, END_STATE_FLAG, current_new_state_id],
                );
            } else {
                hash_result = hash_combine_i32(
                    hash_result,
                    &[current_new_state_id, NOT_END_STATE_FLAG, NOT_END_STATE_FLAG, current_new_state_id],
                );
            }

            let mut unhashed_rules_count = 0i32;
            let mut hash_rule_like_edge = |ref_rule_id: i32, target: i32| -> bool {
                if ref_rule_id == fsm_index {
                    hash_and_target.insert((SELF_RECURSION_FLAG as u64, target));
                    return true;
                }
                if self.per_rule_fsm_hashes[ref_rule_id as usize].is_none() {
                    if !is_start {
                        return false;
                    }
                    unhashed_rules_count += 1;
                    if unhashed_rules_count > 1 {
                        return false;
                    }
                    hash_and_target.insert((UNKNOWN_FLAG as u64, target));
                    return true;
                }
                hash_and_target.insert((self.per_rule_fsm_hashes[ref_rule_id as usize].expect("hashed rule"), target));
                true
            };

            for edge in &self.sorted_edges[current_old_state_id as usize] {
                if edge.is_rule_ref() {
                    if !hash_rule_like_edge(edge.ref_rule_id(), edge.target) {
                        return None;
                    }
                } else if edge.is_repeat_ref() {
                    let info = self.grammar.complete_fsm().repeat_edge_info(edge.aux_index());
                    if !hash_rule_like_edge(info.rule_id(), edge.target) {
                        return None;
                    }
                }
            }

            for &(hash, target) in &hash_and_target {
                if !original_state_id_to_new_id.contains_key(&target) {
                    original_state_id_to_new_id.insert(target, original_state_id_to_new_id.len() as i32);
                    bfs_queue.push_back(target);
                }
                let target_new_id = *original_state_id_to_new_id.get(&target).expect("mapped");
                hash_result = hash_combine(hash_result, &[current_new_state_id as u64, hash, target_new_id as u64]);
            }

            for edge in &self.sorted_edges[current_old_state_id as usize] {
                if !original_state_id_to_new_id.contains_key(&edge.target) {
                    original_state_id_to_new_id.insert(edge.target, original_state_id_to_new_id.len() as i32);
                    bfs_queue.push_back(edge.target);
                }
                let target_new_id = *original_state_id_to_new_id.get(&edge.target).expect("mapped");
                if edge.is_rule_ref() || edge.is_repeat_ref() {
                    continue;
                }
                hash_result = hash_combine_i32(hash_result, &[current_new_state_id, edge.min, edge.max, target_new_id]);
            }
        }

        let new_id_mapping: Vec<(i32, i32)> = original_state_id_to_new_id.into_iter().collect();
        self.per_rule_fsm_new_state_ids[fsm_index as usize] = new_id_mapping;
        Some(hash_result)
    }

    fn hash_fsm(
        &mut self,
        fsm_index: i32,
    ) -> u64 {
        let mut hash_result = 0u64;
        let fsm = self.grammar.per_rule_fsm(fsm_index).expect("per-rule FSM").fsm();
        let mut original_state_id_to_new_id = BTreeMap::from([(fsm.start(), 0)]);
        let mut bfs_queue = VecDeque::from([fsm.start()]);
        let mut hash_and_target: BTreeSet<(i32, i32)> = BTreeSet::new();

        while let Some(current_old_state_id) = bfs_queue.pop_front() {
            let current_new_state_id = *original_state_id_to_new_id.get(&current_old_state_id).expect("mapped");

            if fsm.is_end_state(current_old_state_id) {
                hash_result = hash_combine_i32(
                    hash_result,
                    &[current_new_state_id, END_STATE_FLAG, END_STATE_FLAG, current_new_state_id],
                );
            } else {
                hash_result = hash_combine_i32(
                    hash_result,
                    &[current_new_state_id, NOT_END_STATE_FLAG, NOT_END_STATE_FLAG, current_new_state_id],
                );
            }

            for edge in &self.sorted_edges[current_old_state_id as usize] {
                if edge.is_rule_ref() {
                    let ref_rule_id = edge.ref_rule_id();
                    if ref_rule_id == fsm_index {
                        hash_and_target.insert((SELF_RECURSION_FLAG, edge.target));
                    } else {
                        debug_assert!(self.per_rule_fsm_hashes[ref_rule_id as usize].is_some());
                        hash_and_target.insert((
                            self.per_rule_fsm_hashes[ref_rule_id as usize].expect("hashed rule") as i32,
                            edge.target,
                        ));
                    }
                } else if edge.is_repeat_ref() {
                    let info = self.grammar.complete_fsm().repeat_edge_info(edge.aux_index());
                    let ref_rule_id = info.rule_id();
                    if ref_rule_id == fsm_index {
                        let repeat_hash = hash_combine_i32(SELF_RECURSION_FLAG as u64, &[info.lower(), info.upper()]);
                        hash_and_target.insert((repeat_hash as i32, edge.target));
                    } else {
                        debug_assert!(self.per_rule_fsm_hashes[ref_rule_id as usize].is_some());
                        let base_hash = self.per_rule_fsm_hashes[ref_rule_id as usize].expect("hashed rule");
                        let repeat_hash = hash_combine_i32(base_hash, &[info.lower(), info.upper()]);
                        hash_and_target.insert((repeat_hash as i32, edge.target));
                    }
                }
            }

            for &(hash, target) in &hash_and_target {
                if !original_state_id_to_new_id.contains_key(&target) {
                    original_state_id_to_new_id.insert(target, original_state_id_to_new_id.len() as i32);
                    bfs_queue.push_back(target);
                }
                let target_new_id = *original_state_id_to_new_id.get(&target).expect("mapped");
                hash_result = hash_combine_i32(hash_result, &[current_new_state_id, hash, target_new_id]);
            }

            for edge in &self.sorted_edges[current_old_state_id as usize] {
                if !original_state_id_to_new_id.contains_key(&edge.target) {
                    original_state_id_to_new_id.insert(edge.target, original_state_id_to_new_id.len() as i32);
                    bfs_queue.push_back(edge.target);
                }
                let target_new_id = *original_state_id_to_new_id.get(&edge.target).expect("mapped");
                if edge.is_rule_ref() || edge.is_repeat_ref() {
                    continue;
                }
                hash_result = hash_combine_i32(hash_result, &[current_new_state_id, edge.min, edge.max, target_new_id]);
            }
        }

        let new_id_mapping: Vec<(i32, i32)> = original_state_id_to_new_id.into_iter().collect();
        self.per_rule_fsm_new_state_ids[fsm_index as usize] = new_id_mapping;
        hash_result
    }
}
