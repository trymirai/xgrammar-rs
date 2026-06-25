//! Builds a trie / Aho-Corasick FSM from a set of byte patterns — a port of
//! `TrieFSMBuilder` in `cpp/fsm_builder.cc`.
//!
//! Without back edges this is a plain prefix trie; with `add_back_edges` it is completed into
//! an Aho-Corasick automaton whose every state has a transition for all 256 byte values
//! (failing matches fall back toward the start state).

use std::collections::{BTreeMap, HashSet};

use super::{
    fsm::{EdgeKind, Fsm, NO_NEXT_STATE},
    fsm_edge::FsmEdge,
    fsm_with_start_end::FsmWithStartEnd,
};

/// Options and entry point for trie construction.
#[derive(Debug, Clone, Default)]
pub struct TrieFsmBuilder;

impl TrieFsmBuilder {
    /// Builds a trie FSM from `patterns`.
    ///
    /// - `excluded_patterns`: patterns whose terminal states are pruned (only meaningful with
    ///   `add_back_edges`).
    /// - `end_states`: if `Some`, receives each pattern's terminal state id in input order.
    /// - `allow_overlap`: when `false`, returns `None` if any pattern is empty or is a prefix
    ///   of (or extends) another.
    /// - `add_back_edges`: completes the trie into an Aho-Corasick automaton.
    ///
    /// Returns `None` if construction fails the overlap constraint.
    #[must_use]
    pub fn build(
        patterns: &[&str],
        excluded_patterns: &[&str],
        end_states: Option<&mut Vec<i32>>,
        allow_overlap: bool,
        add_back_edges: bool,
    ) -> Option<FsmWithStartEnd> {
        let mut fsm = Fsm::new(1);
        let start = 0;
        let mut ends: HashSet<i32> = HashSet::new();
        let mut collected_ends: Vec<i32> = Vec::new();

        for pattern in patterns {
            let Some(end) =
                Self::insert_pattern(&mut fsm, pattern, &ends, allow_overlap)
            else {
                return None;
            };
            ends.insert(end);
            collected_ends.push(end);
        }

        let mut dead_states: HashSet<i32> = HashSet::new();
        if add_back_edges {
            for pattern in excluded_patterns {
                let Some(end) = Self::insert_pattern(
                    &mut fsm,
                    pattern,
                    &ends,
                    allow_overlap,
                ) else {
                    return None;
                };
                ends.insert(end);
                dead_states.insert(end);
            }

            Self::add_back_edges(&mut fsm, start, &ends);

            if !dead_states.is_empty() {
                for state in 0..fsm.num_states() {
                    fsm.state_edges_mut(state)
                        .retain(|e| !dead_states.contains(&e.target));
                }
            }
        }

        if let Some(out) = end_states {
            *out = collected_ends;
        }

        let mut is_end = vec![false; fsm.num_states() as usize];
        for &end in &ends {
            is_end[end as usize] = true;
        }
        Some(FsmWithStartEnd::new(fsm, start, is_end, false))
    }

    /// Walks/extends the trie along `pattern`'s bytes, returning its terminal state, or `None`
    /// if the overlap constraint is violated.
    fn insert_pattern(
        fsm: &mut Fsm,
        pattern: &str,
        ends: &HashSet<i32>,
        allow_overlap: bool,
    ) -> Option<i32> {
        if !allow_overlap && pattern.is_empty() {
            return None;
        }
        let mut current = 0;
        for byte in pattern.bytes() {
            let ch = i32::from(byte);
            let mut next = fsm.next_state(current, ch, EdgeKind::CharRange);
            if next == NO_NEXT_STATE {
                next = fsm.add_state();
                fsm.add_edge(current, next, ch, ch);
            }
            current = next;
            if !allow_overlap && ends.contains(&current) {
                return None;
            }
        }
        if !allow_overlap && !fsm.state_edges(current).is_empty() {
            return None;
        }
        Some(current)
    }

    /// Completes the trie into an Aho-Corasick automaton (the C++ `AddBackEdges`).
    fn add_back_edges(
        fsm: &mut Fsm,
        start: i32,
        ends: &HashSet<i32>,
    ) {
        let root_edges: Vec<FsmEdge> = fsm.state_edges(start).to_vec();
        for i in 0..fsm.num_states() {
            if i == start || ends.contains(&i) {
                continue;
            }
            // Range-keyed edge set: equal (min, max) keeps the first-inserted target.
            let mut edge_set: BTreeMap<(i32, i32), i32> = BTreeMap::new();
            for e in fsm.state_edges(i) {
                edge_set.entry((e.min, e.max)).or_insert(e.target);
            }
            // Step 1: inherit the root's transitions where this state has none.
            for e in &root_edges {
                edge_set.entry((e.min, e.max)).or_insert(e.target);
            }
            // Step 2: route every remaining byte back to the start.
            Self::fill_range_edges(&mut edge_set, start);
            // Step 3: install the completed edge list.
            *fsm.state_edges_mut(i) = edge_set
                .iter()
                .map(|(&(min, max), &t)| FsmEdge::new(min, max, t))
                .collect();
        }

        let mut start_set: BTreeMap<(i32, i32), i32> = BTreeMap::new();
        for e in fsm.state_edges(start) {
            start_set.entry((e.min, e.max)).or_insert(e.target);
        }
        Self::fill_range_edges(&mut start_set, start);
        *fsm.state_edges_mut(start) = start_set
            .iter()
            .map(|(&(min, max), &t)| FsmEdge::new(min, max, t))
            .collect();
    }

    /// Fills the gaps between the existing `[min, max]` ranges so the set covers all of
    /// `[0, 255]`, routing the filler ranges to `start`.
    fn fill_range_edges(
        edge_set: &mut BTreeMap<(i32, i32), i32>,
        start: i32,
    ) {
        edge_set.insert((-1, -1), 0);
        edge_set.insert((256, 256), 0);
        let keys: Vec<(i32, i32)> = edge_set.keys().copied().collect();
        for w in keys.windows(2) {
            let (_, prev_max) = w[0];
            let (cur_min, _) = w[1];
            if prev_max + 1 != cur_min {
                edge_set.insert((prev_max + 1, cur_min - 1), start);
            }
        }
        edge_set.remove(&(-1, -1));
        edge_set.remove(&(256, 256));
    }
}
