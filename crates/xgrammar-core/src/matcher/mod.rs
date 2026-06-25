//! The grammar matcher: drives the Earley parser to accept input and (later) produce token
//! bitmasks. Ported from `cpp/grammar_matcher.cc`.
//!
//! One dedicated type per file; re-exported here.

mod batch_grammar_matcher;
mod grammar_matcher;
mod token_bitmask;

pub use batch_grammar_matcher::BatchGrammarMatcher;
pub use grammar_matcher::GrammarMatcher;
pub use token_bitmask::{
    allocate_token_bitmask, apply_token_bitmask_inplace_cpu, get_bitmask_size,
    get_masked_tokens_from_bitmask, is_single_token_bitmask,
    reset_token_bitmask,
};
