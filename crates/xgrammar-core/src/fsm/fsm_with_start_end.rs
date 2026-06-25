//! An FSM paired with a start state and a set of accepting states — a port of
//! `FSMWithStartEnd` (over `FSM`) in `cpp/fsm.{h,cc}`.
//!
//! This carries the regex/grammar building algebra (`concat`, `union`, `star`, `plus`,
//! `optional`) plus string acceptance used to test built machines.

use std::collections::HashSet;

use super::fsm::{EdgeKind, Fsm};

/// A finite-state machine with a designated start state and accepting-state set.
#[derive(Debug, Clone)]
pub struct FsmWithStartEnd {
    fsm: Fsm,
    start: i32,
    ends: Vec<bool>,
    is_dfa: bool,
}

impl FsmWithStartEnd {
    /// Creates an FSM-with-start/end from its parts.
    #[must_use]
    pub fn new(
        fsm: Fsm,
        start: i32,
        ends: Vec<bool>,
        is_dfa: bool,
    ) -> Self {
        Self {
            fsm,
            start,
            ends,
            is_dfa,
        }
    }

    /// The underlying FSM.
    #[must_use]
    pub fn fsm(&self) -> &Fsm {
        &self.fsm
    }

    /// A mutable reference to the underlying FSM.
    pub fn fsm_mut(&mut self) -> &mut Fsm {
        &mut self.fsm
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

    /// Whether `state` is an accepting state.
    #[must_use]
    pub fn is_end_state(
        &self,
        state: i32,
    ) -> bool {
        self.ends[state as usize]
    }

    /// Whether this machine is known to be a DFA.
    #[must_use]
    pub fn is_dfa(&self) -> bool {
        self.is_dfa
    }

    /// The number of states.
    #[must_use]
    pub fn num_states(&self) -> i32 {
        self.fsm.num_states()
    }

    /// Sets the start state.
    pub fn set_start_state(
        &mut self,
        state: i32,
    ) {
        self.start = state;
    }

    /// Marks `state` as accepting.
    pub fn add_end_state(
        &mut self,
        state: i32,
    ) {
        self.ends[state as usize] = true;
    }

    /// Replaces the accepting-state set.
    pub fn set_end_states(
        &mut self,
        ends: Vec<bool>,
    ) {
        self.ends = ends;
    }

    /// Adds a new (non-accepting) state, returning its id.
    pub fn add_state(&mut self) -> i32 {
        self.ends.push(false);
        self.fsm.add_state()
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
            self.fsm.advance(
                &states,
                i32::from(byte),
                &mut result,
                EdgeKind::CharRange,
                false,
            );
            if result.is_empty() {
                return false;
            }
            std::mem::swap(&mut states, &mut result);
        }
        states.iter().any(|&s| self.ends[s as usize])
    }

    /// The set of states reachable from the start state.
    #[must_use]
    pub fn reachable_states(&self) -> HashSet<i32> {
        self.fsm.reachable_states(&[self.start])
    }

    /// Whether the FSM is a leaf (contains no rule or repeat references).
    #[must_use]
    pub fn is_leaf(&self) -> bool {
        self.reachable_states().iter().all(|&state| {
            self.fsm
                .state_edges(state)
                .iter()
                .all(|e| !e.is_rule_ref() && !e.is_repeat_ref())
        })
    }

    /// Returns a copy.
    #[must_use]
    pub fn copy(&self) -> Self {
        self.clone()
    }

    /// `self*` — zero or more repetitions (a fresh accepting start state loops back).
    #[must_use]
    pub fn star(&self) -> Self {
        let mut fsm = self.fsm.clone();
        let new_start = fsm.add_state();
        for end in 0..self.num_states() {
            if self.is_end_state(end) {
                fsm.add_epsilon_edge(end, new_start);
            }
        }
        fsm.add_epsilon_edge(new_start, self.start);
        let mut is_end = vec![false; self.num_states() as usize + 1];
        is_end[new_start as usize] = true;
        Self::new(fsm, new_start, is_end, false)
    }

    /// `self+` — one or more repetitions (each end loops back to the start).
    #[must_use]
    pub fn plus(&self) -> Self {
        let mut fsm = self.fsm.clone();
        for end in 0..self.num_states() {
            if self.is_end_state(end) {
                fsm.add_epsilon_edge(end, self.start);
            }
        }
        Self::new(fsm, self.start, self.ends.clone(), false)
    }

    /// `self?` — zero or one repetition (start may epsilon-skip to an end).
    #[must_use]
    pub fn optional(&self) -> Self {
        let mut fsm = self.fsm.clone();
        for end in 0..self.num_states() {
            if self.is_end_state(end) {
                fsm.add_epsilon_edge(self.start, end);
                break;
            }
        }
        Self::new(fsm, self.start, self.ends.clone(), false)
    }

    /// The union of `fsms` (a fresh start state epsilon-links to each).
    ///
    /// # Panics
    /// Panics if `fsms` is empty.
    #[must_use]
    pub fn union(fsms: &[FsmWithStartEnd]) -> Self {
        assert!(!fsms.is_empty(), "union of 0 FSMs is not allowed");
        if fsms.len() == 1 {
            return fsms[0].clone();
        }
        let mut fsm = Fsm::new(1);
        let start = 0;
        let mut ends = vec![false];
        for sub in fsms {
            let offset = fsm.add_fsm(sub.fsm());
            fsm.add_epsilon_edge(start, offset + sub.start());
            for state in 0..sub.num_states() {
                ends.push(sub.is_end_state(state));
            }
        }
        Self::new(fsm, start, ends, false)
    }

    /// The concatenation of `fsms` (each machine's ends epsilon-link to the next's start).
    ///
    /// # Panics
    /// Panics if `fsms` is empty.
    #[must_use]
    pub fn concat(fsms: &[FsmWithStartEnd]) -> Self {
        assert!(!fsms.is_empty(), "concatenation of 0 FSMs is not allowed");
        if fsms.len() == 1 {
            return fsms[0].clone();
        }
        let mut fsm = Fsm::default();
        let mut start = 0;
        let mut ends: Vec<bool> = Vec::new();
        let mut previous_ends: Vec<i32> = Vec::new();
        let last = fsms.len() - 1;
        for (i, sub) in fsms.iter().enumerate() {
            let offset = fsm.add_fsm(sub.fsm());
            if i == 0 {
                start = offset + sub.start();
            } else {
                let this_start = offset + sub.start();
                for &end in &previous_ends {
                    fsm.add_epsilon_edge(end, this_start);
                }
            }
            if i == last {
                ends = vec![false; fsm.num_states() as usize];
                for end in 0..sub.num_states() {
                    if sub.is_end_state(end) {
                        ends[(offset + end) as usize] = true;
                    }
                }
            } else {
                previous_ends.clear();
                for end in 0..sub.num_states() {
                    if sub.is_end_state(end) {
                        previous_ends.push(offset + end);
                    }
                }
            }
        }
        Self::new(fsm, start, ends, false)
    }
}
