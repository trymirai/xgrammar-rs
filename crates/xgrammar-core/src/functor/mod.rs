//! Grammar transformation passes — the "functors": normalization, structure
//! normalization, inlining, dead-code elimination, optimization, etc. Ported from
//! `cpp/grammar_functor.{h,cc}`.
//!
//! [`GrammarMutator`] is the visitor/mutator framework; concrete passes implement it.

mod mutator;

pub use mutator::{GrammarMutator, MutatorState};
