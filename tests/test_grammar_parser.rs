mod test_utils;

use serial_test::serial;
use xgrammar::Grammar;

#[test]
#[serial]
fn test_e2e_to_string_roundtrip() {
    let before = r#"root ::= ((b c) | (b root))
b ::= ((b_1 d))
c ::= ((c_1))
d ::= ((d_1))
b_1 ::= ("" | ("b" b_1)) (=(d))
c_1 ::= (([acep-z] c_1) | ([acep-z])) (=("d"))
d_1 ::= ("" | ("d"))
"#;
    let g1 = Grammar::from_ebnf(before, "root");
    let s1 = g1.to_string();
    let g2 = Grammar::from_ebnf(&s1, "root");
    let s2 = g2.to_string();
    assert_eq!(s1, s2);
}

// Note: Most tests from Python test_grammar_parser.py are not ported because:
// 1. They require _ebnf_to_grammar_no_normalization which is a testing-only function
//    for testing the parser without normalization functors
// 2. They require GrammarFunctor class methods (structure_normalizer, byte_string_fuser,
//    lookahead_assertion_analyzer, rule_inliner, dead_code_eliminator) which are
//    testing-only utilities not exposed in the Rust bindings yet
// 3. Error tests use pytest.raises(RuntimeError) which can't be caught in Rust when
//    C++ uses XGRAMMAR_LOG(FATAL)
//
// The main end-to-end functionality is tested via test_e2e_to_string_roundtrip which
// verifies that parsing and printing is idempotent.
//
// Python tests not ported:
// - test_basic_string_literal
// - test_empty_string
// - test_character_class
// - test_negated_character_class
// - test_complex_character_class
// - test_sequence
// - test_choice
// - test_grouping
// - test_star_quantifier_simple
// - test_plus_quantifier
// - test_question_quantifier
// - test_character_class_star
// - test_repetition_range_exact
// - test_repetition_range_min_max
// - test_repetition_range_min_only
// - test_lookahead_assertion_simple
// - test_complex_lookahead
// - test_escape_sequences
// - test_unicode_escape
// - test_complex_grammar
// - test_nested_quantifiers
// - test_combined_features
// - test_bnf_comment
// - test_star_quantifier
// - test_repetition_range
// - test_lookahead_assertion_with_normalizer
// - test_char
// - test_space
// - test_nest
// - test_empty_parentheses
// - test_lookahead_assertion_analyzer
// - test_flatten
// - test_rule_inliner (parametrized)
// - test_dead_code_eliminator (parametrized)
// - test_e2e_json_grammar
// - test_lexer_parser_errors (parametrized)
// - test_end_to_end_errors (parametrized)
// - test_error_consecutive_quantifiers
