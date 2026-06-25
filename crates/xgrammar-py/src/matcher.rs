//! `GrammarMatcher` binding.

use crate::compiler::CompiledGrammar;

/// Drives constrained decoding over a compiled grammar.
#[bindings::export(Class)]
#[derive(Debug, Clone)]
pub struct GrammarMatcher {
    pub(crate) inner: xgrammar::matcher::GrammarMatcher,
}

#[bindings::export(Implementation)]
impl GrammarMatcher {
    /// Creates a matcher over a compiled grammar.
    ///
    /// `override_stop_tokens` and `max_rollback_tokens` are accepted for API parity; the
    /// rollback history is currently unbounded.
    #[bindings::export(Method(Constructor))]
    pub fn new(
        compiled_grammar: CompiledGrammar,
        _override_stop_tokens: Option<Vec<i32>>,
        terminate_without_stop_token: bool,
        _max_rollback_tokens: i32,
    ) -> GrammarMatcher {
        GrammarMatcher {
            inner: xgrammar::matcher::GrammarMatcher::from_compiled_grammar(
                &compiled_grammar.inner,
                terminate_without_stop_token,
            ),
        }
    }

    /// Accepts a single token id, advancing the matcher. Returns whether it was accepted.
    #[bindings::export(Method)]
    pub fn accept_token(
        &mut self,
        token_id: i32,
        _debug_print: bool,
    ) -> bool {
        self.inner.accept_token(token_id)
    }

    /// Whether the matcher has reached an accepting terminal state.
    #[bindings::export(Method)]
    pub fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }

    /// Whether the grammar is fully matched (root completed).
    #[bindings::export(Method)]
    pub fn is_completed(&self) -> bool {
        self.inner.is_completed()
    }

    /// Resets the matcher to its initial state.
    #[bindings::export(Method)]
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Rolls back the last `num_tokens` accepted tokens.
    #[bindings::export(Method)]
    pub fn rollback(
        &mut self,
        num_tokens: i32,
    ) {
        self.inner.rollback(num_tokens);
    }

    /// Returns a deep copy of the matcher at its current state.
    #[bindings::export(Method)]
    pub fn fork(&self) -> GrammarMatcher {
        GrammarMatcher {
            inner: self.inner.fork(),
        }
    }

    /// The stop token ids the matcher accepts as terminators.
    #[bindings::export(Method)]
    pub fn stop_token_ids(&self) -> Vec<i32> {
        self.inner.stop_token_ids().to_vec()
    }
}

#[cfg(feature = "bindings-pyo3")]
mod matcher_pyo3_ext {
    use pyo3::{exceptions::PyNotImplementedError, prelude::*};

    use super::GrammarMatcher;
    use crate::{
        bitmask_util::with_writable_i32_buffer, tokenizer_info::TokenizerInfo,
    };

    #[pyo3::pymethods]
    impl GrammarMatcher {
        #[pyo3(name = "accept_string")]
        fn accept_string_py(
            &mut self,
            input: &Bound<'_, PyAny>,
            _debug_print: bool,
        ) -> PyResult<bool> {
            if input.is_instance_of::<pyo3::types::PyBytes>() {
                Ok(self.inner.accept_bytes(&input.extract::<Vec<u8>>()?))
            } else {
                Ok(self.inner.accept_string(&input.extract::<String>()?))
            }
        }

        #[pyo3(name = "fill_next_token_bitmask")]
        fn fill_next_token_bitmask_py(
            &mut self,
            py: Python<'_>,
            bitmask: &Bound<'_, PyAny>,
            index: i32,
            _debug_print: bool,
        ) -> PyResult<bool> {
            with_writable_i32_buffer(py, bitmask, |buf| {
                self.inner.fill_next_token_bitmask(buf, index).map_err(
                    |error| {
                        pyo3::exceptions::PyRuntimeError::new_err(
                            error.to_string(),
                        )
                    },
                )
            })
        }

        #[pyo3(name = "tokenizer_info")]
        fn tokenizer_info_py(&self) -> TokenizerInfo {
            TokenizerInfo::wrap(self.inner.tokenizer_info().clone())
        }

        #[pyo3(name = "accept_stop_token")]
        fn accept_stop_token_py(&mut self) -> bool {
            self.inner.accept_stop_token()
        }

        #[pyo3(name = "_debug_print_internal_state")]
        fn debug_print_internal_state_py(&self) -> String {
            self.inner.debug_print_internal_state()
        }

        #[pyo3(name = "find_jump_forward_string")]
        fn find_jump_forward_string_py(&mut self) -> PyResult<String> {
            let bytes =
                self.inner.find_jump_forward_string().map_err(|error| {
                    pyo3::exceptions::PyRuntimeError::new_err(error.to_string())
                })?;
            String::from_utf8(bytes).map_err(|error| {
                pyo3::exceptions::PyRuntimeError::new_err(error.to_string())
            })
        }

        #[pyo3(name = "traverse_draft_tree")]
        #[allow(clippy::too_many_arguments)]
        fn traverse_draft_tree_py(
            &mut self,
            _retrieve_next_token: &Bound<'_, PyAny>,
            _retrieve_next_sibling: &Bound<'_, PyAny>,
            _draft_tokens: &Bound<'_, PyAny>,
            _token_bitmask: &Bound<'_, PyAny>,
            _time_threshold: f64,
        ) -> PyResult<bool> {
            Err(PyNotImplementedError::new_err(
                "traverse_draft_tree is not yet implemented in the pure-Rust core",
            ))
        }
    }
}
