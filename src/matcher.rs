use std::pin::Pin;

use autocxx::prelude::*;

use crate::{
    DLTensor, FFIGrammarMatcher, compiler::CompiledGrammar, cxx_utils,
};

/// Get the shape of the bitmask for next token prediction.
///
/// # Parameters
/// - `batch_size`: The batch size of the bitmask.
/// - `vocab_size`: The size of the vocabulary.
///
/// # Returns
/// A tuple of (batch_size, ceil(vocab_size / 32)).
pub fn get_bitmask_shape(
    batch_size: usize,
    vocab_size: usize,
) -> (usize, usize) {
    (batch_size, (vocab_size + 31) / 32)
}

/// Allocate the bitmask for the next token prediction. The bitmask is an int32 tensor on
/// CPU with shape (batch_size, ceil(vocab_size / 32)).
///
/// The reason why we use int32 instead of uint32 is compatibility with various tensor libraries.
///
/// # Parameters
/// - `batch_size`: The batch size of the bitmask.
/// - `vocab_size`: The size of the vocabulary.
///
/// # Returns
/// A boxed slice containing the bitmask data, initialized to all bits set (no masking).
pub fn allocate_token_bitmask(
    batch_size: usize,
    vocab_size: usize,
) -> Box<[i32]> {
    let (_, bitmask_size) = get_bitmask_shape(batch_size, vocab_size);
    let total_size = batch_size * bitmask_size;
    vec![-1i32; total_size].into_boxed_slice()
}

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
    ///
    /// In the following cases, the matcher will not accept the token and return False:
    /// 1. The token does not match the grammar.
    /// 2. The matcher has terminated after accepting the stop token, but is trying to accept a new token.
    /// 3. The token id is out of range.
    /// 4. The token is a special token.
    ///
    /// The user should capture the return value and handle the cases where the token is not accepted.
    ///
    /// # Parameters
    /// - `token_id`: The id of the token to accept.
    /// - `debug_print`: Whether to print information about the internal state of the matcher (default: false).
    ///
    /// # Returns
    /// Whether the token is accepted.
    pub fn accept_token(
        &mut self,
        token_id: i32,
    ) -> bool {
        self.inner.as_mut().AcceptToken(token_id, false)
    }

    /// Accept one token with optional debug printing.
    pub fn accept_token_with_debug(
        &mut self,
        token_id: i32,
        debug_print: bool,
    ) -> bool {
        self.inner.as_mut().AcceptToken(token_id, debug_print)
    }

    /// Accept a string and update the state of the matcher.
    ///
    /// The whole string is considered as one step in rollback. It is used to complement the
    /// functionality of accept_token, and accept_token should always be used to accept tokens.
    ///
    /// # Parameters
    /// - `input`: The string to be accepted.
    /// - `debug_print`: Whether to print information about the internal state of the matcher (default: false).
    ///
    /// # Returns
    /// Whether the string is accepted.
    pub fn accept_string(
        &mut self,
        input: &str,
        debug_print: bool,
    ) -> bool {
        cxx::let_cxx_string!(input_cxx = input);
        self.inner.as_mut().AcceptString(&input_cxx, debug_print)
    }

    /// Fill the bitmask for the next token prediction.
    ///
    /// The input bitmask must be on CPU. bitmask[index] will be filled with the next token bitmask.
    /// This method does not change the matcher state.
    ///
    /// # Parameters
    /// - `bitmask`: The bitmask for the next token prediction.
    /// - `index`: The batch id of the bitmask (default: 0).
    /// - `debug_print`: Whether to print information about generated bitmask (default: false).
    ///
    /// # Returns
    /// Whether the bitmask need to be applied (not all-true). An optimization: if False,
    /// this means the bitmask is already all-true, so no need to apply it.
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
    ///
    /// This is the longest string that certainly conforms with the current grammar from the
    /// current matcher state. This string can become the output of the LLM without requiring
    /// LLM decoding.
    ///
    /// This method does not change the matcher state.
    ///
    /// # Returns
    /// The jump-forward string.
    pub fn find_jump_forward_string(&mut self) -> String {
        self.inner.as_mut().FindJumpForwardString().to_string()
    }

    /// Rollback the matcher to a previous state by several tokens.
    ///
    /// # Parameters
    /// - `num_tokens`: The number of tokens to rollback (default: 1). It cannot exceed the
    ///   current number of steps, nor can it exceed the specified maximum number of rollback tokens.
    pub fn rollback(
        &mut self,
        num_tokens: i32,
    ) {
        self.inner.as_mut().Rollback(autocxx::c_int(num_tokens));
    }

    /// Check if the matcher has terminated.
    ///
    /// If terminate_without_stop_token is False, the matcher will terminate if it has accepted
    /// the stop token. Otherwise, the matcher will terminate after matching the whole grammar.
    ///
    /// # Returns
    /// Whether the matcher has terminated.
    pub fn is_terminated(&self) -> bool {
        self.inner.IsTerminated()
    }

    /// Reset the matcher to the initial state.
    pub fn reset(&mut self) {
        self.inner.as_mut().Reset();
    }

    /// Get the maximum number of rollback tokens allowed.
    ///
    /// Deprecated. Now max_rollback_tokens is always unlimited (-1).
    ///
    /// # Returns
    /// The maximum number of rollback tokens.
    pub fn max_rollback_tokens(&self) -> i32 {
        -1
    }

    /// The ids of the stop tokens used in the matcher.
    ///
    /// If specified, the provided stop tokens will be used. Otherwise, the stop tokens will
    /// be detected from the vocabulary.
    ///
    /// # Returns
    /// The ids of the stop tokens.
    pub fn stop_token_ids(&self) -> Box<[i32]> {
        self.stored_stop_token_ids.clone()
    }

    /// Print the internal state of the matcher. This is used for debugging.
    ///
    /// The representation of the internal state is subject to change.
    ///
    /// # Returns
    /// The internal state of the matcher.
    pub fn debug_print_internal_state(&self) -> String {
        self.inner._DebugPrintInternalState().to_string()
    }
}
