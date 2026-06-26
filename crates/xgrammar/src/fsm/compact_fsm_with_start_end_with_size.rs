//! A [`CompactFsmWithStartEnd`] bundled with its pre-merge edge/node counts — a port of
//! `CompactFSMWithStartEndWithSize` in `cpp/fsm.h`. This is the per-rule FSM stored on a
//! compiled grammar.

use super::compact_fsm_with_start_end::CompactFsmWithStartEnd;

/// A compact per-rule FSM plus the edge/node counts of the rule's FSM before it was merged
/// into the shared complete FSM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactFsmWithStartEndWithSize {
    fsm: CompactFsmWithStartEnd,
    edge_num: i32,
    node_num: i32,
}

impl CompactFsmWithStartEndWithSize {
    /// Creates the wrapper from its parts.
    #[must_use]
    pub fn new(
        fsm: CompactFsmWithStartEnd,
        edge_num: i32,
        node_num: i32,
    ) -> Self {
        Self {
            fsm,
            edge_num,
            node_num,
        }
    }

    /// The compact FSM with its start/accepting states.
    #[must_use]
    pub fn fsm(&self) -> &CompactFsmWithStartEnd {
        &self.fsm
    }

    /// The rule's edge count before merging.
    #[must_use]
    pub fn edge_num(&self) -> i32 {
        self.edge_num
    }

    /// The rule's node count before merging.
    #[must_use]
    pub fn node_num(&self) -> i32 {
        self.node_num
    }
}
