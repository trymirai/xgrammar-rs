//! A mutable finite-state machine stored as an adjacency list — a port of `FSM` in
//! `cpp/fsm.{h,cc}`.
//!
//! States are integer ids; each state owns a `Vec<FsmEdge>`. Aux-bearing edges (repeat,
//! token, exclude-token) index into the shared `edge_aux_data` buffer.

use std::{
    collections::{HashSet, VecDeque},
    fmt::Write,
};

use super::fsm_edge::{
    ExcludeTokenEdgeRef, FsmEdge, RepeatEdgeRef, TokenEdgeRef, edge_type,
};
use crate::support::escape_codepoint;

/// Sentinel returned by [`Fsm::next_state`] when no transition exists.
pub const NO_NEXT_STATE: i32 = -1;

/// The kind of transition to follow when advancing (epsilon is handled implicitly via the
/// epsilon closure, so it is not a query kind).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    /// Match a character value against `[min, max]` ranges.
    CharRange,
    /// Match a rule-reference edge by rule id.
    RuleRef,
    /// Follow the end-of-string edge.
    Eos,
    /// Follow the single repeat-reference edge.
    RepeatRef,
}

/// A finite-state machine: per-state edge lists plus a shared auxiliary-data buffer.
#[derive(Debug, Clone, Default)]
pub struct Fsm {
    edges: Vec<Vec<FsmEdge>>,
    edge_aux_data: Vec<i32>,
}

impl Fsm {
    /// Creates an FSM with `num_states` states and no edges.
    #[must_use]
    pub fn new(num_states: usize) -> Self {
        Self {
            edges: vec![Vec::new(); num_states],
            edge_aux_data: Vec::new(),
        }
    }

    /// Creates an FSM from explicit edge lists and aux data.
    #[must_use]
    pub fn from_edges(
        edges: Vec<Vec<FsmEdge>>,
        edge_aux_data: Vec<i32>,
    ) -> Self {
        Self {
            edges,
            edge_aux_data,
        }
    }

    /// The number of states.
    #[must_use]
    pub fn num_states(&self) -> i32 {
        self.edges.len() as i32
    }

    /// All states' edge lists.
    #[must_use]
    pub fn edges(&self) -> &[Vec<FsmEdge>] {
        &self.edges
    }

    /// The outgoing edges of `state`.
    #[must_use]
    pub fn state_edges(
        &self,
        state: i32,
    ) -> &[FsmEdge] {
        &self.edges[state as usize]
    }

    /// A mutable reference to the outgoing edge list of `state`.
    pub fn state_edges_mut(
        &mut self,
        state: i32,
    ) -> &mut Vec<FsmEdge> {
        &mut self.edges[state as usize]
    }

    /// The shared auxiliary-data buffer.
    #[must_use]
    pub fn edge_aux_data(&self) -> &[i32] {
        &self.edge_aux_data
    }

    /// Replaces the shared auxiliary-data buffer.
    pub fn set_edge_aux_data(
        &mut self,
        data: Vec<i32>,
    ) {
        self.edge_aux_data = data;
    }

    /// Renders the given `states` (or all states, if `None`) as a multi-line edge listing —
    /// a port of `EdgesToString`.
    #[must_use]
    pub fn edges_to_string(
        &self,
        states: Option<&[i32]>,
    ) -> String {
        let mut result = String::from("[\n");
        let render = |result: &mut String, state: i32| {
            let _ = write!(result, "{state}: [");
            let edges = &self.edges[state as usize];
            for (j, edge) in edges.iter().enumerate() {
                if edge.min >= 0 && edge.min != edge.max {
                    let lo = escape_codepoint(edge.min, &[]);
                    let hi = escape_codepoint(edge.max, &[]);
                    let _ = write!(result, "[{lo}-{hi}]->{}", edge.target);
                } else if edge.min >= 0 {
                    let c = escape_codepoint(edge.min, &[]);
                    let _ = write!(result, "'{c}'->{}", edge.target);
                } else if edge.is_rule_ref() {
                    let _ =
                        write!(result, "Rule({})->{}", edge.max, edge.target);
                } else if edge.is_epsilon() {
                    let _ = write!(result, "Eps->{}", edge.target);
                } else if edge.is_eos() {
                    let _ = write!(result, "EOS->{}", edge.target);
                } else if edge.is_repeat_ref() {
                    let info = self.repeat_edge_info(edge.max);
                    let _ = write!(
                        result,
                        "Repeat(rule={}, min={}, max={})->{}",
                        info.rule_id(),
                        info.lower(),
                        info.upper(),
                        edge.target
                    );
                } else if edge.is_token() {
                    let info = self.token_edge_info(edge.max);
                    result.push_str("Token(");
                    for (k, id) in info.token_ids().iter().enumerate() {
                        if k > 0 {
                            result.push_str(", ");
                        }
                        let _ = write!(result, "{id}");
                    }
                    let _ = write!(result, ")->{}", edge.target);
                } else if edge.is_exclude_token() {
                    let info = self.exclude_token_edge_info(edge.max);
                    result.push_str("ExcludeToken(");
                    for (k, id) in info.token_ids().iter().enumerate() {
                        if k > 0 {
                            result.push_str(", ");
                        }
                        let _ = write!(result, "{id}");
                    }
                    let _ = write!(result, ")->{}", edge.target);
                }
                if j < edges.len() - 1 {
                    result.push_str(", ");
                }
            }
            result.push_str("]\n");
        };
        match states {
            Some(states) => {
                for &state in states {
                    render(&mut result, state);
                }
            },
            None => {
                for state in 0..self.num_states() {
                    render(&mut result, state);
                }
            },
        }
        result.push(']');
        result
    }

    /// Adds a new state, returning its id.
    pub fn add_state(&mut self) -> i32 {
        self.edges.push(Vec::new());
        (self.edges.len() - 1) as i32
    }

    /// Adds a character-range edge `from -[min,max]-> to`.
    pub fn add_edge(
        &mut self,
        from: i32,
        to: i32,
        min: i32,
        max: i32,
    ) {
        self.edges[from as usize].push(FsmEdge::new(min, max, to));
    }

    /// Adds an epsilon edge.
    pub fn add_epsilon_edge(
        &mut self,
        from: i32,
        to: i32,
    ) {
        self.edges[from as usize].push(FsmEdge::new(edge_type::EPSILON, 0, to));
    }

    /// Adds a rule-reference edge.
    pub fn add_rule_edge(
        &mut self,
        from: i32,
        to: i32,
        rule_id: i32,
    ) {
        self.edges[from as usize].push(FsmEdge::new(
            edge_type::RULE_REF,
            rule_id,
            to,
        ));
    }

    /// Adds an end-of-string edge.
    pub fn add_eos_edge(
        &mut self,
        from: i32,
        to: i32,
    ) {
        self.edges[from as usize].push(FsmEdge::new(edge_type::EOS, 0, to));
    }

    /// Adds a repeat-reference edge (the source state must have no other outgoing edges).
    ///
    /// # Panics
    /// Panics (debug builds) if the source state already has edges.
    pub fn add_repeat_edge(
        &mut self,
        from: i32,
        to: i32,
        rule_id: i32,
        lower: i32,
        upper: i32,
    ) {
        debug_assert!(
            self.edges[from as usize].is_empty(),
            "a state with a repeat-ref edge must have no other outgoing edges"
        );
        let aux_index = self.edge_aux_data.len() as i32;
        self.edge_aux_data.extend_from_slice(&[rule_id, lower, upper]);
        self.edges[from as usize].push(FsmEdge::new(
            edge_type::REPEAT_REF,
            aux_index,
            to,
        ));
    }

    fn push_token_aux(
        &mut self,
        token_ids: &[i32],
    ) -> i32 {
        let aux_index = self.edge_aux_data.len() as i32;
        self.edge_aux_data.push(token_ids.len() as i32);
        self.edge_aux_data.extend_from_slice(token_ids);
        aux_index
    }

    /// Adds a token-set edge.
    ///
    /// # Panics
    /// Panics (debug builds) if `token_ids` is empty.
    pub fn add_token_edge(
        &mut self,
        from: i32,
        to: i32,
        token_ids: &[i32],
    ) {
        debug_assert!(!token_ids.is_empty(), "token set must not be empty");
        let aux_index = self.push_token_aux(token_ids);
        self.edges[from as usize].push(FsmEdge::new(
            edge_type::TOKEN,
            aux_index,
            to,
        ));
    }

    /// Adds an exclude-token-set edge.
    ///
    /// # Panics
    /// Panics (debug builds) if `token_ids` is empty.
    pub fn add_exclude_token_edge(
        &mut self,
        from: i32,
        to: i32,
        token_ids: &[i32],
    ) {
        debug_assert!(
            !token_ids.is_empty(),
            "token exclude set must not be empty"
        );
        let aux_index = self.push_token_aux(token_ids);
        self.edges[from as usize].push(FsmEdge::new(
            edge_type::EXCLUDE_TOKEN,
            aux_index,
            to,
        ));
    }

    /// The repeat-edge aux view at `idx`.
    #[must_use]
    pub fn repeat_edge_info(
        &self,
        idx: i32,
    ) -> RepeatEdgeRef<'_> {
        RepeatEdgeRef {
            data: &self.edge_aux_data[idx as usize..],
        }
    }

    /// The token-edge aux view at `idx`.
    #[must_use]
    pub fn token_edge_info(
        &self,
        idx: i32,
    ) -> TokenEdgeRef<'_> {
        TokenEdgeRef {
            data: &self.edge_aux_data[idx as usize..],
        }
    }

    /// The exclude-token-edge aux view at `idx`.
    #[must_use]
    pub fn exclude_token_edge_info(
        &self,
        idx: i32,
    ) -> ExcludeTokenEdgeRef<'_> {
        ExcludeTokenEdgeRef {
            data: &self.edge_aux_data[idx as usize..],
        }
    }

    /// The first state reachable from `from` on `value` via an edge of `kind`, or
    /// [`NO_NEXT_STATE`].
    #[must_use]
    pub fn next_state(
        &self,
        from: i32,
        value: i32,
        kind: EdgeKind,
    ) -> i32 {
        let edges = &self.edges[from as usize];
        match kind {
            EdgeKind::CharRange => {
                for e in edges {
                    if e.is_char_range() && e.min <= value && e.max >= value {
                        return e.target;
                    }
                }
                NO_NEXT_STATE
            },
            EdgeKind::RuleRef => {
                for e in edges {
                    if e.is_rule_ref() && e.ref_rule_id() == value {
                        return e.target;
                    }
                }
                NO_NEXT_STATE
            },
            EdgeKind::Eos => {
                for e in edges {
                    if e.is_eos() {
                        return e.target;
                    }
                }
                NO_NEXT_STATE
            },
            EdgeKind::RepeatRef => {
                debug_assert!(edges.len() == 1 && edges[0].is_repeat_ref());
                edges[0].target
            },
        }
    }

    /// Expands `state_set` in place with all states reachable via epsilon transitions.
    pub fn epsilon_closure(
        &self,
        state_set: &mut HashSet<i32>,
    ) {
        let mut queue: VecDeque<i32> = state_set.iter().copied().collect();
        while let Some(current) = queue.pop_front() {
            for e in &self.edges[current as usize] {
                if e.is_epsilon() && state_set.insert(e.target) {
                    queue.push_back(e.target);
                }
            }
        }
    }

    /// Advances the (epsilon-closed) set `from` on `value` via edges of `kind`, storing the
    /// epsilon-closed successor set in `result`.
    pub fn advance(
        &self,
        from: &HashSet<i32>,
        value: i32,
        result: &mut HashSet<i32>,
        kind: EdgeKind,
        from_is_closure: bool,
    ) {
        let mut owned_closure;
        let start_closure = if from_is_closure {
            from
        } else {
            owned_closure = from.clone();
            self.epsilon_closure(&mut owned_closure);
            &owned_closure
        };

        result.clear();
        for &state in start_closure {
            for e in &self.edges[state as usize] {
                let matches = match kind {
                    EdgeKind::CharRange => {
                        e.is_char_range() && e.min <= value && e.max >= value
                    },
                    EdgeKind::RuleRef => {
                        e.is_rule_ref() && e.ref_rule_id() == value
                    },
                    EdgeKind::Eos => e.is_eos(),
                    EdgeKind::RepeatRef => e.is_repeat_ref(),
                };
                if matches {
                    result.insert(e.target);
                }
            }
        }
        self.epsilon_closure(result);
    }

    /// The set of rule ids referenced by `state`'s outgoing edges.
    #[must_use]
    pub fn possible_rules(
        &self,
        state: i32,
    ) -> HashSet<i32> {
        self.edges[state as usize]
            .iter()
            .filter(|e| e.is_rule_ref())
            .map(FsmEdge::ref_rule_id)
            .collect()
    }

    /// All states reachable from `from` (following every edge as a plain transition).
    #[must_use]
    pub fn reachable_states(
        &self,
        from: &[i32],
    ) -> HashSet<i32> {
        let mut result: HashSet<i32> = from.iter().copied().collect();
        let mut queue: VecDeque<i32> = from.iter().copied().collect();
        while let Some(current) = queue.pop_front() {
            for e in &self.edges[current as usize] {
                if result.insert(e.target) {
                    queue.push_back(e.target);
                }
            }
        }
        result
    }

    /// Appends a copy of `other`'s states and edges, returning the state-id offset (so
    /// `other`'s state `s` becomes `offset + s`). Aux indices are rebased onto this FSM's
    /// `edge_aux_data`.
    pub fn add_fsm(
        &mut self,
        other: &Fsm,
    ) -> i32 {
        let old_num_states = self.num_states();
        let aux_offset = self.edge_aux_data.len() as i32;
        self.edge_aux_data.extend_from_slice(&other.edge_aux_data);
        self.edges.resize(self.edges.len() + other.edges.len(), Vec::new());
        for (i, state_edges) in other.edges.iter().enumerate() {
            let from = old_num_states + i as i32;
            for e in state_edges {
                let max = if e.is_aux_edge() && aux_offset > 0 {
                    e.max + aux_offset
                } else {
                    e.max
                };
                self.edges[from as usize].push(FsmEdge::new(
                    e.min,
                    max,
                    e.target + old_num_states,
                ));
            }
        }
        old_num_states
    }

    /// Rebuilds the FSM under `state_mapping` (old id → new id), dropping epsilon self-loops
    /// and de-duplicating edges per state.
    #[must_use]
    pub fn rebuild_with_mapping(
        &self,
        state_mapping: &[i32],
        new_num_states: i32,
    ) -> Fsm {
        let mut new_edges: Vec<Vec<FsmEdge>> =
            vec![Vec::new(); new_num_states as usize];
        for (i, state_edges) in self.edges.iter().enumerate() {
            for e in state_edges {
                if e.is_epsilon()
                    && state_mapping[i] == state_mapping[e.target as usize]
                {
                    continue;
                }
                new_edges[state_mapping[i] as usize].push(FsmEdge::new(
                    e.min,
                    e.max,
                    state_mapping[e.target as usize],
                ));
            }
        }
        for edges in &mut new_edges {
            edges.sort_unstable();
            edges.dedup();
        }
        Fsm::from_edges(new_edges, self.edge_aux_data.clone())
    }

    /// Sorts each state's outgoing edges lexicographically.
    pub fn sort_edges(&mut self) {
        for edges in &mut self.edges {
            edges.sort_unstable();
        }
    }
}
