//! Foundational utilities ported from `cpp/support/`: UTF-8 encoding, compact CSR
//! arrays, dynamic bitsets, integer-set operations, hashing helpers, and the
//! recursion-depth guard.
//!
//! One dedicated type per file; re-exported here.

mod compact_2d_array;
mod dynamic_bitset;
mod int_set;

pub use compact_2d_array::{Compact2dArray, Compact2dArrayError};
pub use dynamic_bitset::{BITS_PER_BLOCK, DynamicBitset};
pub use int_set::{intset_complement, intset_difference, intset_intersection, intset_union};
