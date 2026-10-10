//! The finite-state-machine engine: edges, NFAs/DFAs, and the regex/trie/suffix-automaton
//! builders that back grammar compilation.
//!
//! One dedicated type per file; re-exported here.

mod compact_fsm;
mod compact_fsm_with_start_end;
mod compact_fsm_with_start_end_with_size;
mod fsm;
mod fsm_edge;
mod fsm_with_start_end;
mod fsm_with_start_end_with_size;
mod packed_utf8;
mod regex_escape;
mod regex_fsm_builder;
mod regex_ir;
mod regex_node;
mod regex_stack_entry;
mod rewrite_regex_dots;
mod serialization;
mod suffix_automata;
mod trie_fsm_builder;

pub use compact_fsm::CompactFsm;
pub use compact_fsm_with_start_end::CompactFsmWithStartEnd;
pub use compact_fsm_with_start_end_with_size::CompactFsmWithStartEndWithSize;
pub use fsm::{EdgeKind, Fsm, NO_NEXT_STATE};
pub use fsm_edge::{ExcludeTokenEdgeRef, FsmEdge, MAX_CHAR, RepeatEdgeRef, TokenEdgeRef, edge_type};
pub use fsm_with_start_end::FsmWithStartEnd;
pub use fsm_with_start_end_with_size::FsmWithStartEndWithSize;
pub use packed_utf8::{add_packed_utf8_range_edges, codepoint_to_packed_utf8};
pub use regex_fsm_builder::RegexFsmBuilder;
pub use rewrite_regex_dots::rewrite_regex_dots;
pub use serialization::{deserialize_compact_2d_fsm_edges, serialize_compact_2d_fsm_edges};
pub use suffix_automata::SuffixAutomata;
pub use trie_fsm_builder::TrieFsmBuilder;

/// An inclusive range of Unicode codepoints.
type CodepointRange = (u32, u32);
