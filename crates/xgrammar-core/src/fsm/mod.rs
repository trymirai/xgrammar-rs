//! The finite-state-machine engine: edges, NFAs/DFAs, and the regex/trie builders that
//! back grammar compilation. Ported from `cpp/fsm.{h,cc}` and `cpp/fsm_builder.{h,cc}`.
//!
//! One dedicated type per file; re-exported here.

mod compact_fsm;
mod compact_fsm_with_start_end;
mod fsm;
mod fsm_edge;
mod fsm_with_start_end;
mod regex_fsm_builder;
mod trie_fsm_builder;

pub use compact_fsm::CompactFsm;
pub use compact_fsm_with_start_end::CompactFsmWithStartEnd;
pub use fsm::{EdgeKind, Fsm, NO_NEXT_STATE};
pub use fsm_edge::{
    ExcludeTokenEdgeRef, FsmEdge, MAX_CHAR, RepeatEdgeRef, TokenEdgeRef,
    edge_type,
};
pub use fsm_with_start_end::FsmWithStartEnd;
pub use regex_fsm_builder::build_regex_fsm;
pub use trie_fsm_builder::TrieFsmBuilder;
