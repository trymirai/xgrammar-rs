//! Per-rule FSM slice metadata after merging into a shared complete FSM — a port of
//! `FSMWithStartEndWithSize` in `cpp/fsm.h`.
//!
//! When each rule's FSM is folded into one shared `complete_fsm`, what the compiler needs to
//! keep per rule is its start state, accepting-state set (indexed into the complete FSM), and
//! its original edge/node counts. (The C++ also kept a redundant copy of the growing complete
//! FSM; that copy is never read, so it is omitted here.)

/// The start state, accepting set, and pre-merge sizes of a rule's FSM within a shared FSM.
#[derive(Debug, Clone)]
pub struct FsmWithStartEndWithSize {
    start: i32,
    ends: Vec<bool>,
    edge_num: i32,
    node_num: i32,
}

impl FsmWithStartEndWithSize {
    /// Creates the metadata from its parts.
    #[must_use]
    pub fn new(
        start: i32,
        ends: Vec<bool>,
        edge_num: i32,
        node_num: i32,
    ) -> Self {
        Self {
            start,
            ends,
            edge_num,
            node_num,
        }
    }

    /// The rule's start state (within the complete FSM).
    #[must_use]
    pub fn start(&self) -> i32 {
        self.start
    }

    /// The rule's accepting-state flags (indexed by state id within the complete FSM).
    #[must_use]
    pub fn ends(&self) -> &[bool] {
        &self.ends
    }

    /// The number of edges in the rule's FSM before merging.
    #[must_use]
    pub fn edge_num(&self) -> i32 {
        self.edge_num
    }

    /// The number of states in the rule's FSM before merging.
    #[must_use]
    pub fn node_num(&self) -> i32 {
        self.node_num
    }
}
