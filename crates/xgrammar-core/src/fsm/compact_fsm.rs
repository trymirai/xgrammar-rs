//! A finite-state machine in compressed-sparse-row form — a port of `CompactFSM` in
//! `cpp/fsm.{h,cc}`.
//!
//! Same semantics as [`Fsm`](super::fsm::Fsm) but with the per-state edge lists packed into
//! one [`Compact2dArray`], which is cheaper to store/serialize and to scan. Build one with
//! [`CompactFsm::from_fsm`] and convert back with [`CompactFsm::to_fsm`].

use std::collections::{HashSet, VecDeque};

use super::{fsm::Fsm, fsm_edge::FsmEdge};
use crate::support::Compact2dArray;

/// A finite-state machine stored as CSR rows of edges plus a shared auxiliary-data buffer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompactFsm {
    edges: Compact2dArray<FsmEdge>,
    edge_aux_data: Vec<i32>,
}

impl CompactFsm {
    /// Builds a compact FSM from `fsm`, sorting each state's edges (matching the C++
    /// `ToCompact`, which sorts before packing).
    #[must_use]
    pub fn from_fsm(fsm: &Fsm) -> Self {
        let mut sorted = fsm.clone();
        sorted.sort_edges();
        let mut edges: Compact2dArray<FsmEdge> = Compact2dArray::new();
        for state in 0..sorted.num_states() {
            edges.push_row(sorted.state_edges(state));
        }
        Self {
            edges,
            edge_aux_data: sorted.edge_aux_data().to_vec(),
        }
    }

    /// Expands back into an adjacency-list [`Fsm`].
    #[must_use]
    pub fn to_fsm(&self) -> Fsm {
        let rows: Vec<Vec<FsmEdge>> =
            self.edges.iter().map(<[FsmEdge]>::to_vec).collect();
        Fsm::from_edges(rows, self.edge_aux_data.clone())
    }

    /// The number of states.
    #[must_use]
    pub fn num_states(&self) -> i32 {
        self.edges.len() as i32
    }

    /// The outgoing edges of `state`.
    #[must_use]
    pub fn state_edges(
        &self,
        state: i32,
    ) -> &[FsmEdge] {
        self.edges.row(state as usize)
    }

    /// The shared auxiliary-data buffer.
    #[must_use]
    pub fn edge_aux_data(&self) -> &[i32] {
        &self.edge_aux_data
    }

    /// Expands `state_set` in place with all states reachable via epsilon transitions.
    pub fn epsilon_closure(
        &self,
        state_set: &mut HashSet<i32>,
    ) {
        let mut queue: VecDeque<i32> = state_set.iter().copied().collect();
        while let Some(current) = queue.pop_front() {
            for e in self.state_edges(current) {
                if e.is_epsilon() && state_set.insert(e.target) {
                    queue.push_back(e.target);
                }
            }
        }
    }

    /// Advances the (epsilon-closed) set `from` on character `value`, storing the
    /// epsilon-closed successor set in `result`.
    pub fn advance_char(
        &self,
        from: &HashSet<i32>,
        value: i32,
        result: &mut HashSet<i32>,
    ) {
        result.clear();
        for &state in from {
            for e in self.state_edges(state) {
                if e.is_char_range() && e.min <= value && e.max >= value {
                    result.insert(e.target);
                }
            }
        }
        self.epsilon_closure(result);
    }
}
