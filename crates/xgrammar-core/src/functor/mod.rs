//! Grammar transformation passes — the "functors": normalization, structure
//! normalization, inlining, dead-code elimination, optimization, etc. Ported from
//! `cpp/grammar_functor.{h,cc}`.
//!
//! [`GrammarMutator`] is the visitor/mutator framework; concrete passes implement it.

mod byte_string_fuser;
mod dead_code_eliminator;
mod grammar_normalizer;
mod grammar_union_concat;
mod lookahead_assertion_analyzer;
mod mutator;
mod repetition_range_expander;
mod root_rule_renamer;
mod rule_inliner;
mod single_element_expr_eliminator;
mod structure_normalizer;

pub use byte_string_fuser::byte_string_fuser;
pub use dead_code_eliminator::dead_code_eliminator;
pub use grammar_normalizer::grammar_normalizer;
pub use lookahead_assertion_analyzer::lookahead_assertion_analyzer;
pub use mutator::{GrammarMutator, MutatorState};
pub use repetition_range_expander::repetition_range_expander;
pub use root_rule_renamer::root_rule_renamer;
pub use rule_inliner::rule_inliner;
pub use structure_normalizer::structure_normalizer;
