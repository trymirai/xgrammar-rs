//! Foundational utilities ported from `cpp/support/`: UTF-8 encoding, compact CSR
//! arrays, dynamic bitsets, integer-set operations, hashing helpers, and the
//! recursion-depth guard.
//!
//! One dedicated type per file; re-exported here.

mod compact_2d_array;
mod dynamic_bitset;
mod hash;
mod int_set;
mod recursion_guard;

pub use compact_2d_array::{Compact2dArray, Compact2dArrayError};
pub use dynamic_bitset::{BITS_PER_BLOCK, DynamicBitset};
pub use hash::{hash_combine, hash_combine_binary};
pub use int_set::{intset_complement, intset_difference, intset_intersection, intset_union};
pub use recursion_guard::{
    DEFAULT_MAX_RECURSION_DEPTH, MAX_REASONABLE_RECURSION_DEPTH, RecursionError, RecursionGuard,
    get_max_recursion_depth, reset_recursion_depth, set_max_recursion_depth,
};
