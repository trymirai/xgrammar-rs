//! The finite-state-machine engine: edges, NFAs/DFAs, and the regex/trie builders that
//! back grammar compilation. Ported from `cpp/fsm.{h,cc}` and `cpp/fsm_builder.{h,cc}`.
//!
//! One dedicated type per file; re-exported here.

mod fsm_edge;

pub use fsm_edge::{
    ExcludeTokenEdgeRef, FsmEdge, MAX_CHAR, RepeatEdgeRef, TokenEdgeRef,
    edge_type,
};
