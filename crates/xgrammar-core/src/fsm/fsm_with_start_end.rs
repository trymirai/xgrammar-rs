//! An FSM paired with a start state and a set of accepting states — a port of
//! `FSMWithStartEnd` (over `FSM`) in `cpp/fsm.{h,cc}`.
//!
//! This carries the regex/grammar building algebra (`concat`, `union`, `star`, `plus`,
//! `optional`) plus string acceptance used to test built machines.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    fmt,
};

use super::{
    fsm::{EdgeKind, Fsm},
    fsm_edge::edge_type,
};
use crate::support::{Compact2dArray, UnionFindSet};

/// State-count ceiling for [`FsmWithStartEnd::simplify_epsilon`] (`1e8`).
const SIMPLIFY_EPSILON_MAX_STATES: i32 = 100_000_000;
/// State-count ceiling for [`FsmWithStartEnd::merge_equivalent_states`] (`1e5`).
const MERGE_STATES_MAX_STATES: i32 = 100_000;
/// State-count ceiling for [`FsmWithStartEnd::to_dfa`] / [`FsmWithStartEnd::minimize_dfa`] (`1e3`).
const DFA_MAX_STATES: i32 = 1_000;
/// State-count ceiling for [`FsmWithStartEnd::not`] / [`FsmWithStartEnd::intersect`] (`1e6`).
const NOT_INTERSECT_MAX_STATES: i32 = 1_000_000;

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

    /// Appends this machine's states/edges into `complete` (a shared FSM), returning the
    /// resulting start state, accepting set, and pre-merge sizes.
    #[must_use]
    pub fn add_to_complete_fsm(
        &self,
        complete: &mut Fsm,
    ) -> super::fsm_with_start_end_with_size::FsmWithStartEndWithSize {
        let offset = complete.add_fsm(&self.fsm);
        let n = self.num_states();
        let new_start = offset + self.start;
        let mut new_ends = vec![false; complete.num_states() as usize];
        for end in 0..n {
            if self.is_end_state(end) {
                new_ends[(offset + end) as usize] = true;
            }
        }
        let edge_num: usize =
            (0..n).map(|i| self.fsm.state_edges(i).len()).sum();
        super::fsm_with_start_end_with_size::FsmWithStartEndWithSize::new(
            new_start,
            new_ends,
            edge_num as i32,
            n,
        )
    }

    /// Rebuilds this machine under `state_mapping` (old id → new id), collapsing the merged
    /// states. The result is not flagged as a DFA.
    #[must_use]
    pub fn rebuild_with_mapping(
        &self,
        state_mapping: &[i32],
        new_num_states: i32,
    ) -> Self {
        let new_fsm =
            self.fsm.rebuild_with_mapping(state_mapping, new_num_states);
        let new_start = state_mapping[self.start as usize];
        let mut new_ends = vec![false; new_num_states as usize];
        for end in 0..self.num_states() {
            if self.is_end_state(end) {
                new_ends[state_mapping[end as usize] as usize] = true;
            }
        }
        Self::new(new_fsm, new_start, new_ends, false)
    }

    /// The complement of this machine (a leaf DFA accepting exactly the strings it rejects).
    ///
    /// # Errors
    /// Returns an error if the machine contains rule references, or if DFA conversion exceeds
    /// the state limit.
    pub fn not(&self) -> Result<Self, String> {
        if !self.is_leaf() {
            return Err(
                "Not operation is not supported for FSM with rule references."
                    .to_owned(),
            );
        }
        let mut result = if self.is_dfa() {
            self.copy()
        } else {
            self.to_dfa()?
        };
        let mut new_final = vec![false; result.num_states() as usize + 1];
        for i in 0..result.num_states() {
            if !result.is_end_state(i) {
                new_final[i as usize] = true;
            }
        }
        let accept_all = result.add_state();
        new_final[accept_all as usize] = true;
        for i in 0..result.num_states() {
            let mut char_set = [false; 256];
            for e in result.fsm().state_edges(i) {
                if e.is_char_range() {
                    for c in e.min..=e.max {
                        char_set[c as usize] = true;
                    }
                }
            }
            let mut left = 0;
            while left < 256 {
                if char_set[left] {
                    left += 1;
                    continue;
                }
                let mut right = left + 1;
                while right < 256 && !char_set[right] {
                    right += 1;
                }
                result.fsm_mut().add_edge(
                    i,
                    accept_all,
                    left as i32,
                    (right - 1) as i32,
                );
                left = right;
            }
        }
        result.set_end_states(new_final);
        Ok(result)
    }

    /// The intersection of two leaf machines (the product DFA).
    ///
    /// # Errors
    /// Returns an error if either machine has rule references, or if DFA conversion fails.
    pub fn intersect(
        lhs: &Self,
        rhs: &Self,
    ) -> Result<Self, String> {
        if !lhs.is_leaf() || !rhs.is_leaf() {
            return Err("Intersect only support leaf fsm!".to_owned());
        }
        let lhs_dfa = lhs.to_dfa_with_limit(NOT_INTERSECT_MAX_STATES)?;
        let rhs_dfa = rhs.to_dfa_with_limit(NOT_INTERSECT_MAX_STATES)?;
        let mut result = Self::new(Fsm::new(0), 0, Vec::new(), true);
        let mut state_map: HashMap<(i32, i32), i32> = HashMap::new();
        let mut queue: VecDeque<(i32, i32)> = VecDeque::new();
        let start_pair = (lhs_dfa.start(), rhs_dfa.start());
        queue.push_back(start_pair);
        result.add_state();
        state_map.insert(start_pair, 0);
        while let Some((lhs_state, rhs_state)) = queue.pop_front() {
            let from = state_map[&(lhs_state, rhs_state)];
            if lhs_dfa.is_end_state(lhs_state)
                && rhs_dfa.is_end_state(rhs_state)
            {
                result.add_end_state(from);
            }
            for le in lhs_dfa.fsm().state_edges(lhs_state) {
                for re in rhs_dfa.fsm().state_edges(rhs_state) {
                    if le.min > re.max || re.min > le.max {
                        continue;
                    }
                    let min_v = le.min.max(re.min);
                    let max_v = le.max.min(re.max);
                    let key = (le.target, re.target);
                    let target = *state_map.entry(key).or_insert_with(|| {
                        queue.push_back(key);
                        result.add_state()
                    });
                    result.fsm_mut().add_edge(from, target, min_v, max_v);
                }
            }
        }
        Ok(result)
    }

    /// Simplifies epsilon transitions by merging chains of epsilon-only states.
    #[must_use]
    pub fn simplify_epsilon(&self) -> Self {
        if self.is_dfa() || self.num_states() > SIMPLIFY_EPSILON_MAX_STATES {
            return self.clone();
        }
        let n = self.num_states();
        let mut union_find: UnionFindSet<i32> = UnionFindSet::new();
        let mut in_degree = vec![0i32; n as usize];
        let mut epsilon_edges: Vec<(i32, i32)> = Vec::new();

        let mut has_exclude_token = vec![false; n as usize];
        for i in 0..n {
            if self.fsm.state_edges(i).iter().any(|e| e.is_exclude_token()) {
                has_exclude_token[i as usize] = true;
            }
        }

        for i in 0..n {
            let edges = self.fsm.state_edges(i);
            for e in edges {
                in_degree[e.target as usize] += 1;
                if e.is_epsilon() {
                    if edges.len() == 1
                        && !has_exclude_token[i as usize]
                        && !has_exclude_token[e.target as usize]
                    {
                        union_find.add(i);
                        union_find.add(e.target);
                        union_find.union(i, e.target);
                        in_degree[e.target as usize] -= 1;
                    } else {
                        epsilon_edges.push((i, e.target));
                    }
                }
            }
        }

        // Build the equivalent-node mapping, folding in-degree into representatives.
        let mut equiv_node = vec![0i32; n as usize];
        for i in 0..n {
            if union_find.count(i) {
                equiv_node[i as usize] = union_find.find(i);
                if equiv_node[i as usize] == i {
                    continue;
                }
                in_degree[equiv_node[i as usize] as usize] +=
                    in_degree[i as usize];
            } else {
                equiv_node[i as usize] = i;
            }
        }

        let start_rep = equiv_node[self.start as usize];
        for (from_raw, to_raw) in epsilon_edges {
            let from = equiv_node[from_raw as usize];
            let to = equiv_node[to_raw as usize];
            if in_degree[to as usize] == 1
                && start_rep != to
                && !has_exclude_token[from_raw as usize]
                && !has_exclude_token[to_raw as usize]
            {
                union_find.add(from);
                union_find.add(to);
                union_find.union(from, to);
            }
        }

        let eq_classes = union_find.get_all_sets();
        if eq_classes.is_empty() {
            return self.clone();
        }
        let mut new_to_old = vec![-1i32; n as usize];
        for (i, class) in eq_classes.iter().enumerate() {
            for &state in class {
                new_to_old[state as usize] = i as i32;
            }
        }
        let mut cnt = eq_classes.len() as i32;
        for i in 0..n {
            if new_to_old[i as usize] == -1 {
                new_to_old[i as usize] = cnt;
                cnt += 1;
            }
        }
        self.rebuild_with_mapping(&new_to_old, cnt)
    }

    /// Merges equivalent states (shared-prefix and shared-suffix collapsing), iterating to a
    /// fixed point. Returns `self` unchanged if it exceeds the state limit.
    #[must_use]
    pub fn merge_equivalent_states(&self) -> Self {
        if MERGE_STATES_MAX_STATES < self.num_states() {
            return self.clone();
        }
        if self.num_states() < 4 {
            return self.copy();
        }
        let mut result = self.copy();
        result.fsm_mut().sort_edges();
        let mut union_find: UnionFindSet<i32> = UnionFindSet::new();
        let mut changed = true;
        while changed {
            let n = result.num_states() as usize;
            union_find.clear();

            // CSR rows of incoming/outgoing endpoint edges (peer, min, max).
            let mut incoming_sizes = vec![0i32; n];
            let mut outgoing_sizes = vec![0i32; n];
            for (source, out_size) in outgoing_sizes.iter_mut().enumerate() {
                let edges = result.fsm().state_edges(source as i32);
                *out_size = edges.len() as i32;
                for e in edges {
                    incoming_sizes[e.target as usize] += 1;
                }
            }
            let mut incoming: Compact2dArray<EndpointEdge> =
                Compact2dArray::from_row_sizes(&incoming_sizes);
            let mut outgoing: Compact2dArray<EndpointEdge> =
                Compact2dArray::from_row_sizes(&outgoing_sizes);
            let mut incoming_pos = vec![0usize; n];
            let mut outgoing_pos = vec![0usize; n];
            for source in 0..n {
                let edges: Vec<_> =
                    result.fsm().state_edges(source as i32).to_vec();
                for e in &edges {
                    let t = e.target as usize;
                    incoming.row_mut(t)[incoming_pos[t]] = EndpointEdge {
                        peer: source as i32,
                        min: e.min,
                        max: e.max,
                    };
                    incoming_pos[t] += 1;
                    let row = outgoing.row_mut(source);
                    row[outgoing_pos[source]] = EndpointEdge {
                        peer: e.target,
                        min: e.min,
                        max: e.max,
                    };
                    outgoing_pos[source] += 1;
                }
                outgoing.row_mut(source).sort_unstable();
            }

            // States with exactly one distinct predecessor / successor.
            let mut in_distinct = vec![0i32; n];
            let mut out_distinct = vec![0i32; n];
            let mut single_in = vec![-1i32; n];
            let mut single_out = vec![-1i32; n];
            for state in 0..n {
                let in_row = incoming.row(state);
                if !in_row.is_empty() {
                    in_distinct[state] = 1;
                    single_in[state] = in_row[0].peer;
                    for w in in_row.windows(2) {
                        if w[1].peer != w[0].peer {
                            in_distinct[state] += 1;
                            single_in[state] = -1;
                        }
                    }
                }
                let out_row = outgoing.row(state);
                if !out_row.is_empty() {
                    out_distinct[state] = 1;
                    single_out[state] = out_row[0].peer;
                    for w in out_row.windows(2) {
                        if w[1].peer != w[0].peer {
                            out_distinct[state] += 1;
                            single_out[state] = -1;
                        }
                    }
                }
            }

            // Case 1: shared prefix — ab | ac | ad -> a(b | c | d).
            let mut equiv_successor = false;
            for i in 0..n as i32 {
                if in_distinct[i as usize] != 1 || union_find.count(i) {
                    continue;
                }
                let previous = single_in[i as usize];
                let edges_to_i = incoming.row(i as usize);
                let siblings = outgoing.row(previous as usize);
                let mut group_begin = 0;
                while group_begin < siblings.len() {
                    let sibling = siblings[group_begin].peer;
                    let mut group_end = group_begin + 1;
                    while group_end < siblings.len()
                        && siblings[group_end].peer == sibling
                    {
                        group_end += 1;
                    }
                    group_begin = group_end;
                    if sibling <= i
                        || in_distinct[sibling as usize] != 1
                        || result.is_end_state(sibling)
                            != result.is_end_state(i)
                    {
                        continue;
                    }
                    let edges_to_sibling = incoming.row(sibling as usize);
                    if edges_to_i.len() != edges_to_sibling.len() {
                        continue;
                    }
                    let is_equiv = edges_to_i
                        .iter()
                        .zip(edges_to_sibling)
                        .all(|(a, b)| a.min == b.min && a.max == b.max);
                    if is_equiv {
                        union_find.add(i);
                        union_find.add(sibling);
                        union_find.union(i, sibling);
                        equiv_successor = true;
                    }
                }
            }

            // Case 2: shared suffix — ba | ca | da -> (b | c | d)a.
            let mut equiv_precursor = false;
            let mut no_succ_end: Vec<i32> = Vec::new();
            let mut no_succ_non_end: Vec<i32> = Vec::new();
            for i in 0..n as i32 {
                let outgoing_count = out_distinct[i as usize];
                if outgoing_count == 0 {
                    if result.is_end_state(i) {
                        no_succ_end.push(i);
                    } else {
                        no_succ_non_end.push(i);
                    }
                    continue;
                }
                if outgoing_count != 1 || union_find.count(i) {
                    continue;
                }
                let next_state = single_out[i as usize];
                let node_edges = outgoing.row(i as usize);
                let siblings = incoming.row(next_state as usize);
                let mut group_begin = 0;
                while group_begin < siblings.len() {
                    let sibling = siblings[group_begin].peer;
                    while group_begin < siblings.len()
                        && siblings[group_begin].peer == sibling
                    {
                        group_begin += 1;
                    }
                    if sibling <= i
                        || union_find.count(sibling)
                        || out_distinct[sibling as usize] != 1
                        || result.is_end_state(i)
                            != result.is_end_state(sibling)
                    {
                        continue;
                    }
                    let sibling_edges = outgoing.row(sibling as usize);
                    if sibling_edges.len() != node_edges.len() {
                        continue;
                    }
                    let is_equiv = sibling_edges
                        .iter()
                        .zip(node_edges)
                        .all(|(a, b)| a.min == b.min && a.max == b.max);
                    if is_equiv {
                        union_find.add(i);
                        union_find.add(sibling);
                        union_find.union(i, sibling);
                        equiv_precursor = true;
                    }
                }
            }

            if no_succ_end.len() > 1 {
                for &state in &no_succ_end[1..] {
                    union_find.add(no_succ_end[0]);
                    union_find.add(state);
                    union_find.union(no_succ_end[0], state);
                    equiv_precursor = true;
                }
            }
            if no_succ_non_end.len() > 1 {
                for &state in &no_succ_non_end[1..] {
                    union_find.add(no_succ_non_end[0]);
                    union_find.add(state);
                    union_find.union(no_succ_non_end[0], state);
                    equiv_precursor = true;
                }
            }

            changed = equiv_successor || equiv_precursor;
            if changed {
                let eq_classes = union_find.get_all_sets();
                let mut old_to_new = vec![-1i32; result.num_states() as usize];
                for (i, class) in eq_classes.iter().enumerate() {
                    for &state in class {
                        old_to_new[state as usize] = i as i32;
                    }
                }
                let mut cnt = eq_classes.len() as i32;
                for i in 0..result.num_states() {
                    if old_to_new[i as usize] == -1 {
                        old_to_new[i as usize] = cnt;
                        cnt += 1;
                    }
                }
                result = result.rebuild_with_mapping(&old_to_new, cnt);
                result.fsm_mut().sort_edges();
            }
        }
        result
    }

    /// Converts this NFA into an equivalent DFA via subset construction.
    ///
    /// # Errors
    /// Returns an error if the state count exceeds `1e3`.
    pub fn to_dfa(&self) -> Result<Self, String> {
        self.to_dfa_with_limit(DFA_MAX_STATES)
    }

    fn to_dfa_with_limit(
        &self,
        max_num_states: i32,
    ) -> Result<Self, String> {
        if self.num_states() > max_num_states {
            return Err("The number of states exceeds the limit.".to_owned());
        }
        let mut dfa = Self::new(Fsm::new(0), 0, Vec::new(), true);
        let mut closures: Vec<HashSet<i32>> = Vec::new();
        let mut start_closure: HashSet<i32> = HashSet::from([self.start]);
        self.fsm.epsilon_closure(&mut start_closure);
        closures.push(start_closure);

        let closure_of = |target: i32| -> HashSet<i32> {
            let mut c = HashSet::from([target]);
            self.fsm.epsilon_closure(&mut c);
            c
        };

        let mut now_process = 0;
        while now_process < closures.len() {
            let mut rules: HashSet<i32> = HashSet::new();
            let mut repeat_aux: HashSet<i32> = HashSet::new();
            let mut token_aux: HashSet<i32> = HashSet::new();
            let mut exclude_aux: HashSet<i32> = HashSet::new();
            let mut interval_ends: BTreeSet<i32> = BTreeSet::new();
            let mut allowed = [false; 256];
            dfa.add_state();
            for &state in &closures[now_process] {
                if self.is_end_state(state) {
                    dfa.add_end_state(now_process as i32);
                }
                for e in self.fsm.state_edges(state) {
                    if e.is_char_range() {
                        interval_ends.insert(e.min);
                        interval_ends.insert(e.max + 1);
                        for c in e.min..=e.max {
                            allowed[c as usize] = true;
                        }
                    } else if e.is_rule_ref() {
                        rules.insert(e.ref_rule_id());
                    } else if e.is_repeat_ref() {
                        repeat_aux.insert(e.aux_index());
                    } else if e.is_token() {
                        token_aux.insert(e.aux_index());
                    } else if e.is_exclude_token() {
                        exclude_aux.insert(e.aux_index());
                    }
                }
            }

            // Reduce the character transitions to maximal all-allowed intervals.
            let mut intervals: Vec<(i32, i32)> = Vec::new();
            let mut last = -1;
            for &end in &interval_ends {
                if last == -1 {
                    last = end;
                    continue;
                }
                if (last..end).all(|c| allowed[c as usize]) {
                    intervals.push((last, end - 1));
                }
                last = end;
            }

            for (lo, hi) in intervals {
                let mut next_closure: HashSet<i32> = HashSet::new();
                for &state in &closures[now_process] {
                    for e in self.fsm.state_edges(state) {
                        if e.is_char_range()
                            && lo >= e.min
                            && hi <= e.max
                            && !next_closure.contains(&e.target)
                        {
                            next_closure.extend(closure_of(e.target));
                        }
                    }
                }
                let target =
                    Self::find_or_add_closure(&mut closures, &next_closure);
                dfa.fsm_mut().add_edge(now_process as i32, target, lo, hi);
            }

            let mut rules_vec: Vec<i32> = rules.into_iter().collect();
            rules_vec.sort_unstable();
            for rule in rules_vec {
                let mut next_closure: HashSet<i32> = HashSet::new();
                for &state in &closures[now_process] {
                    for e in self.fsm.state_edges(state) {
                        if e.is_rule_ref()
                            && e.ref_rule_id() == rule
                            && !next_closure.contains(&e.target)
                        {
                            next_closure.extend(closure_of(e.target));
                        }
                    }
                }
                let target =
                    Self::find_or_add_closure(&mut closures, &next_closure);
                dfa.fsm_mut().add_rule_edge(now_process as i32, target, rule);
            }

            Self::dfa_aux_transitions(
                &mut dfa,
                &mut closures,
                now_process,
                &self.fsm,
                repeat_aux,
                edge_type::REPEAT_REF,
                &closure_of,
            );
            Self::dfa_aux_transitions(
                &mut dfa,
                &mut closures,
                now_process,
                &self.fsm,
                token_aux,
                edge_type::TOKEN,
                &closure_of,
            );
            Self::dfa_aux_transitions(
                &mut dfa,
                &mut closures,
                now_process,
                &self.fsm,
                exclude_aux,
                edge_type::EXCLUDE_TOKEN,
                &closure_of,
            );

            now_process += 1;
        }
        dfa.fsm_mut().set_edge_aux_data(self.fsm.edge_aux_data().to_vec());
        Ok(dfa)
    }

    fn find_or_add_closure(
        closures: &mut Vec<HashSet<i32>>,
        next_closure: &HashSet<i32>,
    ) -> i32 {
        for (j, c) in closures.iter().enumerate() {
            if c == next_closure {
                return j as i32;
            }
        }
        closures.push(next_closure.clone());
        (closures.len() - 1) as i32
    }

    fn dfa_aux_transitions(
        dfa: &mut Self,
        closures: &mut Vec<HashSet<i32>>,
        now_process: usize,
        fsm: &Fsm,
        aux_indices: HashSet<i32>,
        kind: i32,
        closure_of: &impl Fn(i32) -> HashSet<i32>,
    ) {
        let mut aux_vec: Vec<i32> = aux_indices.into_iter().collect();
        aux_vec.sort_unstable();
        for aux_idx in aux_vec {
            let mut next_closure: HashSet<i32> = HashSet::new();
            for &state in &closures[now_process] {
                for e in fsm.state_edges(state) {
                    if e.min == kind
                        && e.aux_index() == aux_idx
                        && !next_closure.contains(&e.target)
                    {
                        next_closure.extend(closure_of(e.target));
                    }
                }
            }
            let target = Self::find_or_add_closure(closures, &next_closure);
            dfa.fsm_mut().add_edge(now_process as i32, target, kind, aux_idx);
        }
    }

    /// Minimizes a DFA (converting first if needed) via partition refinement.
    ///
    /// # Errors
    /// Returns an error if the state count exceeds `1e3`.
    pub fn minimize_dfa(&self) -> Result<Self, String> {
        if self.num_states() > DFA_MAX_STATES {
            return Err("The number of states exceeds the limit.".to_owned());
        }
        let now_fsm = if self.is_dfa() {
            self.copy()
        } else {
            self.to_dfa()?
        };
        let n = now_fsm.num_states();

        // precursors[target] = list of ((min, max), source).
        let mut precursors: Vec<Vec<((i32, i32), i32)>> =
            vec![Vec::new(); n as usize];
        for i in 0..n {
            for e in now_fsm.fsm().state_edges(i) {
                precursors[e.target as usize].push(((e.min, e.max), i));
            }
        }

        let mut partitions: Vec<HashSet<i32>> = Vec::new();
        let mut working: Vec<HashSet<i32>> = Vec::new();
        let mut final_states: HashSet<i32> = HashSet::new();
        let mut non_final: HashSet<i32> = HashSet::new();
        for i in 0..n {
            if now_fsm.is_end_state(i) {
                final_states.insert(i);
            } else {
                non_final.insert(i);
            }
        }
        partitions.push(final_states.clone());
        partitions.push(non_final.clone());
        working.push(final_states);
        working.push(non_final);

        while let Some(current) = working.pop() {
            let mut possible: BTreeMap<(i32, i32), HashSet<i32>> =
                BTreeMap::new();
            for &state in &current {
                for &(transition, source) in &precursors[state as usize] {
                    possible.entry(transition).or_default().insert(source);
                }
            }
            for precursors_set in possible.values() {
                let mut i = 0;
                while i < partitions.len() {
                    let mut intersection: Vec<i32> = Vec::new();
                    let mut difference: Vec<i32> = Vec::new();
                    for &s in &partitions[i] {
                        if precursors_set.contains(&s) {
                            intersection.push(s);
                        } else {
                            difference.push(s);
                        }
                    }
                    if !intersection.is_empty() && !difference.is_empty() {
                        let inter_set: HashSet<i32> =
                            intersection.iter().copied().collect();
                        let diff_set: HashSet<i32> =
                            difference.iter().copied().collect();
                        let in_working =
                            working.iter().position(|w| *w == partitions[i]);
                        if let Some(pos) = in_working {
                            working[pos] = inter_set.clone();
                            working.push(diff_set.clone());
                        } else if difference.len() < intersection.len() {
                            working.push(diff_set.clone());
                        } else {
                            working.push(inter_set.clone());
                        }
                        partitions[i] = inter_set;
                        partitions.push(diff_set);
                    }
                    i += 1;
                }
            }
        }

        let mut state_mapping = vec![-1i32; n as usize];
        for (i, partition) in partitions.iter().enumerate() {
            for &state in partition {
                state_mapping[state as usize] = i as i32;
            }
        }
        Ok(now_fsm
            .rebuild_with_mapping(&state_mapping, partitions.len() as i32))
    }

    /// Renders this machine the way the C++ `ToString` does (used by tests as the oracle).
    #[must_use]
    pub fn to_string_repr(&self) -> String {
        let mut reachable: Vec<i32> =
            self.reachable_states().into_iter().collect();
        reachable.sort_unstable();
        let ends: Vec<String> = (0..self.num_states())
            .filter(|&i| self.is_end_state(i))
            .map(|i| i.to_string())
            .collect();
        format!(
            "FSM(num_states={}, start={}, end=[{}], edges={})",
            self.num_states(),
            self.start,
            ends.join(", "),
            self.fsm.edges_to_string(Some(&reachable))
        )
    }
}

/// A compact endpoint-edge view used by [`FsmWithStartEnd::merge_equivalent_states`]; `peer`
/// is the source in incoming rows and the target in outgoing rows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
struct EndpointEdge {
    peer: i32,
    min: i32,
    max: i32,
}

impl fmt::Display for FsmWithStartEnd {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(&self.to_string_repr())
    }
}
