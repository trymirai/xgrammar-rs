//! Grammar-guided generation: stateful matchers that drive constrained decoding.
//!
//! [`GrammarMatcher`] is the core logic of grammar-guided generation. It implements a
//! non-deterministic pushdown automaton (NPDA) matching algorithm to match characters to a BNF
//! grammar, maintains several internal stacks as possible paths in the NPDA, and supports
//! backtracking. It is particularly capable of finding the set of tokens acceptable for the
//! next step and storing them in a bitmask.

mod batch_grammar_matcher;
mod budget_body_match_progress;
mod capture_span_event;
mod error;
mod grammar_matcher;
mod token_bitmask;

pub use batch_grammar_matcher::BatchGrammarMatcher;
pub use error::MatcherError;
pub use grammar_matcher::GrammarMatcher;
pub use token_bitmask::{
    BitmaskDlType, allocate_token_bitmask, apply_token_bitmask_inplace_cpu, apply_token_bitmask_inplace_cpu_batch,
    bitmask_row_range, get_bitmask_dl_type, get_bitmask_size, get_masked_tokens_from_bitmask, is_single_token_bitmask,
    reset_token_bitmask,
};
