//! The grammar matcher: drives the Earley parser to accept input and (later) produce token
//! bitmasks. Ported from `cpp/grammar_matcher.cc`.
//!
//! One dedicated type per file; re-exported here.

mod grammar_matcher;

pub use grammar_matcher::GrammarMatcher;
