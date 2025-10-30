use std::pin::Pin;

use autocxx::prelude::*;

use crate::{
    DLTensor, FFIGrammarMatcher, compiler::CompiledGrammar, cxx_utils,
};

/// Match tokens/strings to a compiled grammar and generate next-token masks.
pub struct GrammarMatcher {
    inner: Pin<Box<FFIGrammarMatcher>>,
    stored_stop_token_ids: Box<[i32]>,
}

impl GrammarMatcher {
    /// Construct a GrammarMatcher.
    /// - compiled_grammar: The compiled grammar to match against.
    /// - override_stop_tokens: If Some, override the stop tokens used by the matcher.
    /// - terminate_without_stop_token: Whether to allow termination without consuming a stop token.
    /// - max_rollback_tokens: Maximum rollback tokens (-1 for unlimited).
    pub fn new(
        compiled_grammar: &CompiledGrammar,
        override_stop_tokens: Option<&[i32]>,
        terminate_without_stop_token: bool,
        max_rollback_tokens: i32,
    ) -> Self {
        let stored_stop_token_ids: Box<[i32]> = match override_stop_tokens {
            Some(slice) => slice.to_vec().into_boxed_slice(),
            None => compiled_grammar.tokenizer_info().stop_token_ids(),
        };
        let (has_override, ptr, len) = match override_stop_tokens {
            Some(slice) if !slice.is_empty() => {
                (true, slice.as_ptr(), slice.len())
            },
            _ => (false, std::ptr::null(), 0usize),
        };

        let ffi_pin = unsafe {
            cxx_utils::make_grammar_matcher(
                compiled_grammar.ffi_ref(),
                has_override,
                ptr,
                len,
                terminate_without_stop_token,
                autocxx::c_int(max_rollback_tokens),
            )
            .within_box()
        };
        Self {
            inner: ffi_pin,
            stored_stop_token_ids,
        }
    }

    /// Accept one token and update the state of the matcher.
    pub fn accept_token(
        &mut self,
        token_id: i32,
        debug_print: bool,
    ) -> bool {
        self.inner.as_mut().AcceptToken(token_id, debug_print)
    }

    /// Accept a string and update the state of the matcher.
    pub fn accept_string(
        &mut self,
        input: &str,
        debug_print: bool,
    ) -> bool {
        cxx::let_cxx_string!(input_cxx = input);
        self.inner.as_mut().AcceptString(&input_cxx, debug_print)
    }

    /// Fill the bitmask for the next token prediction.
    /// Returns whether the bitmask needs to be applied (not all-true).
    pub fn fill_next_token_bitmask(
        &mut self,
        bitmask: &mut DLTensor,
        index: i32,
        debug_print: bool,
    ) -> bool {
        unsafe {
            self.inner.as_mut().FillNextTokenBitmask(
                bitmask as *mut _,
                autocxx::c_int(index),
                debug_print,
            )
        }
    }

    /// Find the jump-forward string for jump-forward decoding.
    pub fn find_jump_forward_string(&mut self) -> String {
        self.inner.as_mut().FindJumpForwardString().to_string()
    }

    /// Rollback the matcher by several tokens.
    pub fn rollback(
        &mut self,
        num_tokens: i32,
    ) {
        self.inner.as_mut().Rollback(autocxx::c_int(num_tokens));
    }

    /// Check if the matcher has terminated.
    pub fn is_terminated(&self) -> bool {
        self.inner.IsTerminated()
    }

    /// Reset the matcher to the initial state.
    pub fn reset(&mut self) {
        self.inner.as_mut().Reset();
    }

    /// Maximum number of rollback tokens allowed.
    pub fn max_rollback_tokens(&self) -> i32 {
        self.inner.GetMaxRollbackTokens().0
    }

    /// Stop token ids currently used by the matcher.
    pub fn stop_token_ids(&self) -> Box<[i32]> {
        self.stored_stop_token_ids.clone()
    }

    /// Debug: print the internal state of the matcher (unstable, for testing).
    pub fn debug_print_internal_state(&self) -> String {
        self.inner._DebugPrintInternalState().to_string()
    }
}
