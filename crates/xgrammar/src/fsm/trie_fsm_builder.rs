//! Builds a trie / Aho-Corasick FSM from a set of byte patterns.
//!
//! Without back edges this is a plain prefix trie; with `add_back_edges` it is completed into
//! an Aho-Corasick automaton whose every non-end state has a transition for all 256 byte values
//! (a failing match resumes from the longest proper suffix that is still a trie path).

use std::collections::HashSet;

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
        patterns: &[&[u8]],
        excluded_patterns: &[&[u8]],
        end_states: Option<&mut Vec<i32>>,
        allow_overlap: bool,
        add_back_edges: bool,
    ) -> Option<FsmWithStartEnd> {
        let mut fsm = Fsm::new(1);
        let start = 0;
        let mut ends: HashSet<i32> = HashSet::new();
        let mut collected_ends: Vec<i32> = Vec::new();

        for pattern in patterns {
            let end = Self::insert_pattern(&mut fsm, pattern, &ends, allow_overlap)?;
            ends.insert(end);
            collected_ends.push(end);
        }

        let mut dead_states: HashSet<i32> = HashSet::new();
        if add_back_edges {
            for pattern in excluded_patterns {
                let end = Self::insert_pattern(&mut fsm, pattern, &ends, allow_overlap)?;
                ends.insert(end);
                dead_states.insert(end);
            }

            Self::add_back_edges(&mut fsm, start, &ends, &mut dead_states);

            if !dead_states.is_empty() {
                for state in 0..fsm.num_states() {
                    fsm.state_edges_mut(state).retain(|e| !dead_states.contains(&e.target));
                }
            }
        } else if !excluded_patterns.is_empty() {
            eprintln!("Excluded patterns are ignored when back edges are not added.");
        }

        if let Some(out) = end_states {
            *out = collected_ends;
        }

        Some(FsmWithStartEnd::new(fsm, start, ends.into_iter().collect(), false))
    }

    /// Walks/extends the trie along `pattern`'s bytes, returning its terminal state, or `None`
    /// if the overlap constraint is violated.
    fn insert_pattern(
        fsm: &mut Fsm,
        pattern: &[u8],
        ends: &HashSet<i32>,
        allow_overlap: bool,
    ) -> Option<i32> {
        if !allow_overlap && pattern.is_empty() {
            return None;
        }
        let mut current = 0;
        for &byte in pattern {
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
    ///
    /// When matching fails at state `u` on byte `b`, the matcher must resume from the longest
    /// proper suffix of `u`'s prefix that is still a trie path (the failure state) and retry `b`
    /// there. Falling back only to the start state loses matches that begin inside an
    /// already-followed branch: with patterns `{"bcd", "abce"}` on input `"abcd"`, after `"abc"`
    /// the byte `'d'` must reach the `"bcd"` end through the failure state `"bc"`.
    ///
    /// States whose failure state is dead (an excluded suffix completed at an explicit trie
    /// node) are added to `dead_states`.
    fn add_back_edges(
        fsm: &mut Fsm,
        start: i32,
        ends: &HashSet<i32>,
        dead_states: &mut HashSet<i32>,
    ) {
        let num_states = fsm.num_states() as usize;

        // Step 1. The BFS order of the trie (a tree here), so shallower states come first.
        let mut bfs_order: Vec<i32> = vec![start];
        let mut head = 0;
        while head < bfs_order.len() {
            let state = bfs_order[head];
            bfs_order.extend(fsm.state_edges(state).iter().map(|e| e.target));
            head += 1;
        }
        debug_assert_eq!(bfs_order.len(), num_states);

        // Step 2. The failure links and the fully resolved transition table: `delta[u][b]` is the
        // trie child when it exists and `delta[fail[u]][b]` otherwise. `fail[u]` is strictly
        // shallower than `u`, so its row is already final when `u` is processed in BFS order.
        let mut fail = vec![start; num_states];
        let mut delta = vec![[NO_NEXT_STATE; 256]; num_states];
        for (u, row) in delta.iter_mut().enumerate() {
            for e in fsm.state_edges(u as i32) {
                row[e.min as usize] = e.target;
            }
        }
        for &u in &bfs_order {
            // A trie child can complete an excluded suffix without entering its terminal state:
            // with excludes {"abc", "xabcq"}, the state for "xabc" has failure state "abc".
            if dead_states.contains(&fail[u as usize]) {
                dead_states.insert(u);
            }
            let fail_row = (u != start).then(|| delta[fail[u as usize] as usize]);
            for (byte, entry) in delta[u as usize].iter_mut().enumerate() {
                // Rows of deeper states are untouched so far, so a set entry is a trie child.
                let fallback = fail_row.map_or(start, |row| row[byte]);
                if *entry == NO_NEXT_STATE {
                    *entry = fallback;
                } else {
                    fail[*entry as usize] = fallback;
                }
            }
        }

        // Step 3. Replace the edges of every non-end state with its resolved row, compressing
        // runs of bytes with the same target into range edges.
        for (u, row) in delta.iter().enumerate() {
            if u as i32 != start && ends.contains(&(u as i32)) {
                continue;
            }
            let mut new_edges = Vec::new();
            let mut byte = 0;
            while byte < 256 {
                let mut range_end = byte;
                while range_end + 1 < 256 && row[range_end + 1] == row[byte] {
                    range_end += 1;
                }
                new_edges.push(FsmEdge::new(byte as i32, range_end as i32, row[byte]));
                byte = range_end + 1;
            }
            *fsm.state_edges_mut(u as i32) = new_edges;
        }
    }
}
