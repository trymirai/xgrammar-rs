//! Precomputes tag-dispatch second-slicing bitsets — a port of
//! `GrammarCompilerSub::TagDispatchOptimization` in `cpp/grammar_compiler.cc`.

use std::collections::HashMap;

use crate::{
    grammar::{Grammar, GrammarExprType},
    support::DynamicBitset,
    tokenizer::TokenizerInfo,
};

/// Maps tag-dispatch rule ids to bitsets over sorted-vocab indices: tokens whose suffix
/// (from the second byte onward) contains no trigger or exclude string are marked true.
pub type TagDispatchSecondSlicingBitsets = HashMap<i32, DynamicBitset>;

/// Builds the tag-dispatch second-slicing map for `grammar` and `tokenizer_info`.
#[must_use]
pub fn tag_dispatch_optimization(
    grammar: &Grammar,
    tokenizer_info: &TokenizerInfo,
) -> TagDispatchSecondSlicingBitsets {
    let sorted_decoded_vocab = tokenizer_info.sorted_decoded_vocab();
    let mut result = TagDispatchSecondSlicingBitsets::new();

    for rule_id in 0..grammar.num_rules() {
        let rule = grammar.rule(rule_id);
        let rule_body = grammar.expr(rule.body_expr_id);
        if rule_body.ty != GrammarExprType::TagDispatch {
            continue;
        }
        let tag_dispatch = grammar.tag_dispatch(rule.body_expr_id);
        let mut definite_accepted = DynamicBitset::new(sorted_decoded_vocab.len());

        for (j, (_, token)) in sorted_decoded_vocab.iter().enumerate() {
            if token.is_empty() {
                definite_accepted.set(j, true);
                continue;
            }

            let mut definite_accept_since_second_char = true;
            for (trigger, _) in &tag_dispatch.tag_rule_pairs {
                if contains_from_second(token, trigger) {
                    definite_accept_since_second_char = false;
                    break;
                }
            }
            if definite_accept_since_second_char {
                for excl in &tag_dispatch.excludes {
                    if contains_from_second(token, excl) {
                        definite_accept_since_second_char = false;
                        break;
                    }
                }
            }

            if definite_accept_since_second_char {
                definite_accepted.set(j, true);
            }
        }

        result.insert(rule_id, definite_accepted);
    }

    result
}

fn contains_from_second(
    token: &[u8],
    pattern: &[u8],
) -> bool {
    if pattern.is_empty() || token.len() <= 1 {
        return false;
    }
    token[1..].windows(pattern.len()).any(|window| window == pattern)
}
