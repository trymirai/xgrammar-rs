//! Foundational utilities ported from `cpp/support/`: UTF-8 encoding, compact CSR
//! arrays, dynamic bitsets, integer-set operations, hashing helpers, and the
//! recursion-depth guard.
//!
//! One dedicated type per file; re-exported here.

mod compact_2d_array;

pub use compact_2d_array::{Compact2dArray, Compact2dArrayError};
