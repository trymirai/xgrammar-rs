//! Fuses consecutive byte strings within a sequence, in place.
//!
//! Only sequences that contain an empty byte string or two adjacent byte strings are rebuilt, with
//! each run merged into a single byte string; the rest keep their original ids. Fusing keeps the
//! matcher's hot path comparing whole literals instead of one byte at a time.

use super::in_place_grammar_rewriter::InPlaceGrammarRewriter;
use crate::grammar::{Grammar, GrammarBuilder, GrammarExprType};

/// Fuses adjacent byte strings in `grammar` in place (the `GrammarFunctor.byte_string_fuser` pass).
pub fn byte_string_fuser(grammar: &mut Grammar) {
    ByteStringFuser.apply(grammar);
}

struct ByteStringFuser;

impl InPlaceGrammarRewriter for ByteStringFuser {
    fn visit_sequence(
        &mut self,
        builder: &mut GrammarBuilder,
        _memo: &mut [i32],
        expr_id: i32,
    ) -> i32 {
        // Read-only probe, so the common unchanged case allocates nothing.
        let mut previous_is_byte_string = false;
        let needs_rewrite = builder.grammar_expr(expr_id).data.iter().any(|&element_id| {
            let element = builder.grammar_expr(element_id);
            let is_byte_string = element.ty == GrammarExprType::ByteString;
            let fusable = is_byte_string && (previous_is_byte_string || element.is_empty());
            previous_is_byte_string = is_byte_string;
            fusable
        });
        if !needs_rewrite {
            return expr_id;
        }
        // Copy the element ids first: fusing appends to the arena.
        let element_ids = builder.grammar_expr(expr_id).data.to_vec();
        let mut new_element_ids = Vec::new();
        let mut index = 0;
        while index < element_ids.len() {
            if builder.grammar_expr(element_ids[index]).ty != GrammarExprType::ByteString {
                new_element_ids.push(element_ids[index]);
                index += 1;
                continue;
            }
            let mut fused_bytes = Vec::new();
            let mut run_end = index;
            while let Some(&element_id) = element_ids.get(run_end) {
                let element = builder.grammar_expr(element_id);
                if element.ty != GrammarExprType::ByteString {
                    break;
                }
                fused_bytes.extend_from_slice(element.data);
                run_end += 1;
            }
            // Empty byte strings are epsilon and can be omitted from a sequence.
            if !fused_bytes.is_empty() {
                new_element_ids.push(if run_end - index == 1 {
                    element_ids[index]
                } else {
                    builder.add_byte_string_bytes(&fused_bytes)
                });
            }
            index = run_end;
        }
        builder.add_sequence(&new_element_ids)
    }
}
