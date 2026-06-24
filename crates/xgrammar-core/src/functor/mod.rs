//! Grammar transformation passes — the "functors": normalization, structure
//! normalization, inlining, dead-code elimination, optimization, etc. Ported from
//! `cpp/grammar_functor.{h,cc}`.
//!
//! [`GrammarMutator`] is the visitor/mutator framework; concrete passes implement it.

mod dead_code_eliminator;
mod mutator;
mod single_element_expr_eliminator;
mod structure_normalizer;

pub use dead_code_eliminator::dead_code_eliminator;
pub use mutator::{GrammarMutator, MutatorState};
pub use structure_normalizer::structure_normalizer;
