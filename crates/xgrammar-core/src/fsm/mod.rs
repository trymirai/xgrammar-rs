//! The finite-state-machine engine: edges, NFAs/DFAs, and the regex/trie builders that
//! back grammar compilation. Ported from `cpp/fsm.{h,cc}` and `cpp/fsm_builder.{h,cc}`.
//!
//! One dedicated type per file; re-exported here.

mod fsm;
mod fsm_edge;
mod fsm_with_start_end;

pub use fsm::{EdgeKind, Fsm, NO_NEXT_STATE};
pub use fsm_edge::{
    ExcludeTokenEdgeRef, FsmEdge, MAX_CHAR, RepeatEdgeRef, TokenEdgeRef,
    edge_type,
};
pub use fsm_with_start_end::FsmWithStartEnd;
