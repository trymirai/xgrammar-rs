//! `GrammarMatcher` binding.

use std::sync::{Arc, Mutex, MutexGuard};

// Used by the `new` constructor signature. Only PyO3 emits a constructor companion; the other
// backends drop the (extracted) constructor, leaving this import unused there.
#[cfg_attr(not(feature = "bindings-pyo3"), allow(unused_imports))]
use crate::compiler::CompiledGrammar;

/// Drives constrained decoding over a compiled grammar.
#[bindings::export(Class)]
#[derive(Debug, Clone)]
pub struct GrammarMatcher {
    pub(crate) inner: Arc<Mutex<xgrammar::matcher::GrammarMatcher>>,
}

impl GrammarMatcher {
    pub(crate) fn lock(
        &self
    ) -> MutexGuard<'_, xgrammar::matcher::GrammarMatcher> {
        self.inner.lock().expect("grammar matcher mutex poisoned")
    }
}

#[bindings::export(Implementation)]
impl GrammarMatcher {
    /// Creates a matcher over a compiled grammar.
    ///
    /// `override_stop_tokens` and `max_rollback_tokens` are accepted for API parity; the
    /// rollback history is currently unbounded.
    #[bindings::export(Method(Constructor))]
    pub fn new(
        compiled_grammar: &CompiledGrammar,
        _override_stop_tokens: Option<Vec<i32>>,
        terminate_without_stop_token: bool,
        _max_rollback_tokens: i32,
    ) -> GrammarMatcher {
        GrammarMatcher {
            inner: Arc::new(Mutex::new(
                xgrammar::matcher::GrammarMatcher::from_compiled_grammar(
                    &compiled_grammar.inner,
                    terminate_without_stop_token,
                ),
            )),
        }
    }

    /// Accepts a single token id, advancing the matcher. Returns whether it was accepted.
    #[bindings::export(Method)]
    pub fn accept_token(
        &self,
        token_id: i32,
        _debug_print: bool,
    ) -> bool {
        self.lock().accept_token(token_id)
    }

    /// Accepts a UTF-8 string, advancing the matcher. Returns whether it was accepted.
    #[bindings::export(Method)]
    pub fn accept_string(
        &self,
        input: String,
        _debug_print: bool,
    ) -> bool {
        self.lock().accept_string(&input)
    }

    /// Accepts raw bytes, advancing the matcher. Returns whether it was accepted.
    #[bindings::export(Method)]
    pub fn accept_bytes(
        &self,
        input: Vec<u8>,
        _debug_print: bool,
    ) -> bool {
        self.lock().accept_bytes(&input)
    }

    /// Whether the matcher has reached an accepting terminal state.
    #[bindings::export(Method)]
    pub fn is_terminated(&self) -> bool {
        self.lock().is_terminated()
    }

    /// Whether the grammar is fully matched (root completed).
    #[bindings::export(Method)]
    pub fn is_completed(&self) -> bool {
        self.lock().is_completed()
    }

    /// Resets the matcher to its initial state.
    #[bindings::export(Method)]
    pub fn reset(&self) {
        self.lock().reset();
    }

    /// Rolls back the last `num_tokens` accepted tokens.
    #[bindings::export(Method)]
    pub fn rollback(
        &self,
        num_tokens: i32,
    ) {
        self.lock().rollback(num_tokens);
    }

    /// Returns a deep copy of the matcher at its current state.
    #[bindings::export(Method)]
    pub fn fork(&self) -> GrammarMatcher {
        GrammarMatcher {
            inner: Arc::new(Mutex::new(self.lock().fork())),
        }
    }

    /// The stop token ids the matcher accepts as terminators.
    #[bindings::export(Method)]
    pub fn stop_token_ids(&self) -> Vec<i32> {
        self.lock().stop_token_ids().to_vec()
    }
}

#[cfg(feature = "bindings-pyo3")]
mod matcher_pyo3_ext {
    use pyo3::{exceptions::PyRuntimeError, prelude::*};

    use super::GrammarMatcher;
    use crate::{
        bitmask_util::{i32_shape_2d, read_i64_1d, with_writable_i32_buffer},
        tokenizer_info::TokenizerInfo,
    };

    #[pyo3::pymethods]
    impl GrammarMatcher {
        #[pyo3(name = "fill_next_token_bitmask")]
        fn fill_next_token_bitmask_py(
            &self,
            py: Python<'_>,
            bitmask: &Bound<'_, PyAny>,
            index: i32,
            _debug_print: bool,
        ) -> PyResult<bool> {
            with_writable_i32_buffer(py, bitmask, |buf| {
                self.lock().fill_next_token_bitmask(buf, index).map_err(
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
            TokenizerInfo::wrap(self.lock().tokenizer_info().clone())
        }

        #[pyo3(name = "accept_stop_token")]
        fn accept_stop_token_py(&self) -> bool {
            self.lock().accept_stop_token()
        }

        #[pyo3(name = "_debug_print_internal_state")]
        fn debug_print_internal_state_py(&self) -> String {
            self.lock().debug_print_internal_state()
        }

        #[pyo3(name = "find_jump_forward_string")]
        fn find_jump_forward_string_py(&self) -> PyResult<String> {
            let bytes =
                self.lock().find_jump_forward_string().map_err(|error| {
                    pyo3::exceptions::PyRuntimeError::new_err(error.to_string())
                })?;
            String::from_utf8(bytes).map_err(|error| {
                pyo3::exceptions::PyRuntimeError::new_err(error.to_string())
            })
        }

        #[pyo3(name = "traverse_draft_tree")]
        #[allow(clippy::too_many_arguments)]
        fn traverse_draft_tree_py(
            &self,
            py: Python<'_>,
            retrieve_next_token: &Bound<'_, PyAny>,
            retrieve_next_sibling: &Bound<'_, PyAny>,
            draft_tokens: &Bound<'_, PyAny>,
            token_bitmask: &Bound<'_, PyAny>,
            time_threshold: f64,
        ) -> PyResult<bool> {
            let next_token =
                read_i64_1d(py, retrieve_next_token, "retrieve_next_token")?;
            let next_sibling = read_i64_1d(
                py,
                retrieve_next_sibling,
                "retrieve_next_sibling",
            )?;
            let tokens = read_i64_1d(py, draft_tokens, "draft_tokens")?;
            let shape = i32_shape_2d(py, token_bitmask, "token_bitmask")?;
            if shape[0] != next_token.len() as i64 {
                return Err(PyRuntimeError::new_err(
                    "the token_bitmask batch size must match the number of nodes in the tree",
                ));
            }
            with_writable_i32_buffer(py, token_bitmask, |bitmask| {
                self.lock()
                    .traverse_draft_tree(
                        &next_token,
                        &next_sibling,
                        &tokens,
                        bitmask,
                        time_threshold,
                    )
                    .map_err(PyRuntimeError::new_err)
            })
        }
    }
}
