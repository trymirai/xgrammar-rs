use std::pin::Pin;

use autocxx::prelude::*;

use crate::{DLTensor, cxx_utils};

use super::GrammarMatcher;

/// A batch version of GrammarMatcher that can fill the next token bitmask for multiple
/// matchers in parallel. It utilizes multiple threads to speed up the computation. It is
/// especially useful when the batch size is large.
pub struct BatchGrammarMatcher {
    inner: Pin<Box<crate::FFIBatchGrammarMatcher>>,
}

impl BatchGrammarMatcher {
    /// Construct the batch grammar matcher.
    ///
    /// # Parameters
    /// - `max_threads`: The maximum number of threads to use for parallel processing.
    ///   Use -1 for automatic thread count (hardware_concurrency / 2).
    pub fn new(max_threads: i32) -> Self {
        let ffi_pin = cxx_utils::make_batch_grammar_matcher(max_threads).within_box();
        Self { inner: ffi_pin }
    }

    /// Create a batch grammar matcher with automatic thread count.
    pub fn new_auto() -> Self {
        Self::new(-1)
    }

    /// Fill the next token bitmask for multiple matchers.
    ///
    /// # Parameters
    /// - `matchers`: The list of matchers to fill the bitmask for.
    /// - `bitmask`: Must be a 2-dimensional int32 tensor with shape (bitmask_batch_size, bitmask_size).
    ///   Bitmask_batch_size could be larger than the actual batch size to allow padding.
    ///   Bitmask_size equals to ceil(vocab_size/32).
    /// - `indices`: A list of indices to specify which rows in the bitmask to fill.
    ///   If None, fill the bitmask [0..matchers.len()).
    /// - `debug_print`: Whether to print information about generated bitmask (default: false).
    pub fn batch_fill_next_token_bitmask(
        &mut self,
        matchers: &[GrammarMatcher],
        bitmask: &mut DLTensor,
        indices: Option<&[i32]>,
        debug_print: bool,
    ) {
        // Create a C++ vector of GrammarMatcher objects
        let mut ffi_matcher_vec = cxx_utils::new_grammar_matcher_vector();
        {
            let mut vec_pin = ffi_matcher_vec.pin_mut();
            cxx_utils::grammar_matcher_vec_reserve(vec_pin.as_mut(), matchers.len());
            for matcher in matchers {
                cxx_utils::grammar_matcher_vec_push(
                    vec_pin.as_mut(),
                    matcher.ffi_ref(),
                );
            }
        }

        let (has_indices, indices_ptr, indices_len) = match indices {
            Some(slice) if !slice.is_empty() => (true, slice.as_ptr(), slice.len()),
            _ => (false, std::ptr::null(), 0usize),
        };

        unsafe {
            cxx_utils::batch_matcher_batch_fill_next_token_bitmask(
                self.inner.as_mut(),
                ffi_matcher_vec.as_mut().unwrap().get_unchecked_mut(),
                bitmask as *mut _,
                has_indices,
                indices_ptr,
                indices_len,
                debug_print,
            );
        }
    }

    /// Accept a batch of tokens for multiple matchers.
    ///
    /// # Parameters
    /// - `matchers`: The list of matchers to accept tokens for.
    /// - `tokens`: The list of tokens to accept.
    /// - `debug_print`: Whether to print information about generated bitmask (default: false).
    ///
    /// # Returns
    /// A vector of booleans indicating whether each token was accepted by its corresponding matcher.
    pub fn batch_accept_token(
        matchers: &[GrammarMatcher],
        tokens: &[i32],
        debug_print: bool,
    ) -> Vec<bool> {
        assert_eq!(
            matchers.len(),
            tokens.len(),
            "matchers and tokens must have the same length"
        );

        let mut ffi_matcher_vec = cxx_utils::new_grammar_matcher_vector();
        {
            let mut vec_pin = ffi_matcher_vec.pin_mut();
            cxx_utils::grammar_matcher_vec_reserve(vec_pin.as_mut(), matchers.len());
            for matcher in matchers {
                cxx_utils::grammar_matcher_vec_push(
                    vec_pin.as_mut(),
                    matcher.ffi_ref(),
                );
            }
        }

        let result = unsafe {
            cxx_utils::batch_accept_token(
                ffi_matcher_vec.as_mut().unwrap().get_unchecked_mut(),
                tokens.as_ptr(),
                tokens.len(),
                debug_print,
            )
        };

        result.iter().map(|&b| b != 0).collect()
    }

    /// Accept a batch of strings for multiple matchers.
    ///
    /// # Parameters
    /// - `matchers`: The list of matchers to accept tokens for.
    /// - `strings`: The list of strings to accept.
    /// - `debug_print`: Whether to print information about generated bitmask (default: false).
    ///
    /// # Returns
    /// A vector of booleans indicating whether each string was accepted by its corresponding matcher.
    pub fn batch_accept_string(
        matchers: &[GrammarMatcher],
        strings: &[impl AsRef<str>],
        debug_print: bool,
    ) -> Vec<bool> {
        assert_eq!(
            matchers.len(),
            strings.len(),
            "matchers and strings must have the same length"
        );

        let mut ffi_matcher_vec = cxx_utils::new_grammar_matcher_vector();
        {
            let mut vec_pin = ffi_matcher_vec.pin_mut();
            cxx_utils::grammar_matcher_vec_reserve(vec_pin.as_mut(), matchers.len());
            for matcher in matchers {
                cxx_utils::grammar_matcher_vec_push(
                    vec_pin.as_mut(),
                    matcher.ffi_ref(),
                );
            }
        }

        let mut cxx_strings = cxx_utils::new_string_vector();
        {
            let mut cxx_vec_pin = cxx_strings.pin_mut();
            cxx_utils::string_vec_reserve(cxx_vec_pin.as_mut(), strings.len());
            for string in strings.iter() {
                let bytes = string.as_ref().as_bytes();
                unsafe {
                    cxx_utils::string_vec_push_bytes(
                        cxx_vec_pin.as_mut(),
                        bytes.as_ptr() as *const i8,
                        bytes.len(),
                    );
                }
            }
        }

        let result = unsafe {
            cxx_utils::batch_accept_string(
                ffi_matcher_vec.as_mut().unwrap().get_unchecked_mut(),
                cxx_strings.as_ref().unwrap(),
                debug_print,
            )
        };

        result.iter().map(|&b| b != 0).collect()
    }
}
