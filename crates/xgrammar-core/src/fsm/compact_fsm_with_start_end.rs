//! A [`CompactFsm`] paired with a start state and accepting-state set — a port of
//! `CompactFSMWithStartEnd` in `cpp/fsm.{h,cc}`.

use std::collections::HashSet;

use super::compact_fsm::CompactFsm;

/// A compact finite-state machine with a designated start state and accepting-state set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompactFsmWithStartEnd {
    fsm: CompactFsm,
    start: i32,
    ends: Vec<bool>,
    is_dfa: bool,
    edge_num: usize,
}

impl CompactFsmWithStartEnd {
    /// Creates a compact FSM-with-start/end from its parts.
    #[must_use]
    pub fn new(
        fsm: CompactFsm,
        start: i32,
        ends: Vec<bool>,
        is_dfa: bool,
    ) -> Self {
        let edge_num = fsm.num_edges();
        Self {
            fsm,
            start,
            ends,
            is_dfa,
            edge_num,
        }
    }

    /// The underlying compact FSM.
    #[must_use]
    pub fn fsm(&self) -> &CompactFsm {
        &self.fsm
    }

    /// The start state.
    #[must_use]
    pub fn start(&self) -> i32 {
        self.start
    }

    /// The accepting-state flags, indexed by state id.
    #[must_use]
    pub fn ends(&self) -> &[bool] {
        &self.ends
    }

    /// Whether this machine is known to be a DFA.
    #[must_use]
    pub fn is_dfa(&self) -> bool {
        self.is_dfa
    }

    /// Edge count captured when this view was built (matches C++ `edge_num_`).
    #[must_use]
    pub fn edge_num(&self) -> usize {
        self.edge_num
    }

    /// The number of states.
    #[must_use]
    pub fn num_states(&self) -> i32 {
        self.fsm.num_states()
    }

    /// Whether `state` is an accepting state.
    #[must_use]
    pub fn is_end_state(
        &self,
        state: i32,
    ) -> bool {
        self.ends[state as usize]
    }

    /// Whether `state` has an outgoing character/token edge (can consume input).
    #[must_use]
    pub fn is_scanable_state(
        &self,
        state: i32,
    ) -> bool {
        self.fsm
            .state_edges(state)
            .iter()
            .any(|e| e.is_char_range() || e.is_token() || e.is_exclude_token())
    }

    /// Whether `state` has an outgoing rule/epsilon/repeat edge (is non-terminal).
    #[must_use]
    pub fn is_non_terminal_state(
        &self,
        state: i32,
    ) -> bool {
        self.fsm
            .state_edges(state)
            .iter()
            .any(|e| e.is_rule_ref() || e.is_epsilon() || e.is_repeat_ref())
    }

    /// Whether the FSM accepts `input` (treating it as a byte sequence).
    #[must_use]
    pub fn accept_string(
        &self,
        input: &str,
    ) -> bool {
        let mut states: HashSet<i32> = HashSet::from([self.start]);
        self.fsm.epsilon_closure(&mut states);
        let mut result = HashSet::new();
        for byte in input.bytes() {
            self.fsm.advance_char(&states, i32::from(byte), &mut result);
            if result.is_empty() {
                return false;
            }
            std::mem::swap(&mut states, &mut result);
        }
        states.iter().any(|&s| self.ends[s as usize])
    }
}
