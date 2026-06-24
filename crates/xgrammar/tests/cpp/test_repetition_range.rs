//! Port of `xgrammar/tests/cpp/test_repetition_range.cc`.
//!
//! Regression test: two unbounded repeats with a lower bound above the unzip threshold in
//! the same rule must not index out of bounds during expansion.

use xgrammar::functor::repetition_range_expander;
use xgrammar::grammar::Grammar;

#[test]
fn unbounded_repetition_above_threshold_does_not_crash() {
    // Two unbounded repeats with lower > 128 (the unzip threshold) in the same rule. The
    // first expansion inflates the builder's id space; expanding the second must still look
    // ids up in the builder, not the source grammar.
    let grammar = Grammar::from_ebnf("root ::= [a-z]{129,} [0-9]{129,}", "root").unwrap();
    let _ = repetition_range_expander(&grammar);
}
