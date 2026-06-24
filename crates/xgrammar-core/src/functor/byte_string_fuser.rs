//! Fuses consecutive byte strings within a sequence — a port of `ByteStringFuser` in
//! `cpp/grammar_functor.cc`.
//!
//! Runs on a normalized grammar: within each sequence, runs of adjacent byte-string
//! elements are merged into a single byte string, which keeps the matcher's hot path
//! comparing whole literals instead of one byte at a time.

use super::mutator::{GrammarMutator, MutatorState};
use crate::grammar::{Grammar, GrammarExprType};

/// Fuses adjacent byte strings in `grammar` (the `GrammarFunctor.byte_string_fuser` pass).
#[must_use]
pub fn byte_string_fuser(grammar: &Grammar) -> Grammar {
    ByteStringFuser.apply(grammar)
}

struct ByteStringFuser;

impl GrammarMutator for ByteStringFuser {
    fn visit_sequence(&mut self, state: &mut MutatorState, data: &[i32]) -> i32 {
        let mut new_ids = Vec::new();
        let mut cur_byte_string: Vec<i32> = Vec::new();
        for &child in data {
            let (ty, edata) = {
                let expr = state.base.expr(child);
                (expr.ty, expr.data.to_vec())
            };
            if ty == GrammarExprType::ByteString {
                cur_byte_string.extend_from_slice(&edata);
            } else {
                if !cur_byte_string.is_empty() {
                    new_ids.push(state.builder.add_byte_string_bytes(&cur_byte_string));
                    cur_byte_string.clear();
                }
                new_ids.push(self.visit_expr(state, ty, &edata));
            }
        }
        if !cur_byte_string.is_empty() {
            new_ids.push(state.builder.add_byte_string_bytes(&cur_byte_string));
        }
        state.builder.add_sequence(&new_ids)
    }
}
