//! Eliminates single-element sequences/choices and single-char classes — a port of
//! `SingleElementExprEliminator` in `cpp/grammar_functor.cc`.

use super::mutator::{GrammarMutator, MutatorState};
use crate::grammar::GrammarExprType;
use crate::support::char_to_utf8_bytes;

/// A pass that collapses one-element sequences and choices to their single child, and
/// rewrites a single-codepoint positive character class as a byte string.
#[derive(Default)]
pub(crate) struct SingleElementExprEliminator;

impl GrammarMutator for SingleElementExprEliminator {
    fn visit_sequence(&mut self, state: &mut MutatorState, data: &[i32]) -> i32 {
        let mut ids = Vec::with_capacity(data.len());
        for &child in data {
            ids.push(self.visit_expr_id(state, child));
        }
        if ids.len() == 1 {
            ids[0]
        } else {
            state.builder.add_sequence(&ids)
        }
    }

    fn visit_choices(&mut self, state: &mut MutatorState, data: &[i32]) -> i32 {
        let mut ids = Vec::with_capacity(data.len());
        for &child in data {
            ids.push(self.visit_expr_id(state, child));
        }
        if ids.len() == 1 {
            ids[0]
        } else {
            state.builder.add_choices(&ids)
        }
    }

    fn visit_character_class(
        &mut self,
        state: &mut MutatorState,
        ty: GrammarExprType,
        data: &[i32],
    ) -> i32 {
        // `[c]` (a single, non-negated codepoint) becomes the byte string of that codepoint.
        if data.len() == 3 && data[0] == 0 && data[1] == data[2] {
            let bytes: Vec<i32> = char_to_utf8_bytes(data[1]).iter().map(|&b| i32::from(b)).collect();
            state.builder.add_byte_string_bytes(&bytes)
        } else {
            state.builder.add_grammar_expr(ty, data)
        }
    }
}
