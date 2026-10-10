//! `GrammarMatcher` binding.

use std::sync::{Arc, Mutex, MutexGuard};

use xgrammar::matcher::{allocate_token_bitmask, get_bitmask_size};

use crate::{capture::Capture, error::map_error, token_bitmask::TokenBitmask, tokenizer_info::TokenizerInfo};

/// Drives constrained decoding over a compiled grammar.
#[bindings::export(Class)]
#[derive(Debug, Clone)]
pub struct GrammarMatcher {
    inner: Arc<Mutex<xgrammar::matcher::GrammarMatcher>>,
}

impl GrammarMatcher {
    /// Locks the core matcher.
    pub fn lock(&self) -> MutexGuard<'_, xgrammar::matcher::GrammarMatcher> {
        self.inner.lock().expect("grammar matcher mutex poisoned")
    }
}

impl AsRef<Arc<Mutex<xgrammar::matcher::GrammarMatcher>>> for GrammarMatcher {
    fn as_ref(&self) -> &Arc<Mutex<xgrammar::matcher::GrammarMatcher>> {
        &self.inner
    }
}

#[bindings::export(Implementation)]
impl GrammarMatcher {
    /// Creates a matcher over a compiled grammar.
    ///
    /// `max_rollback_tokens` is accepted for API parity; rollback history is unbounded
    /// (matching C++, which always reports `-1`). `default_temperature` is the sampling
    /// temperature used when no active grammar rule specifies one.
    #[bindings::export(Method(Constructor))]
    pub fn new(
        compiled_grammar: &crate::compiled_grammar::CompiledGrammar,
        override_stop_tokens: Option<Vec<i32>>,
        terminate_without_stop_token: bool,
        _max_rollback_tokens: i32,
        default_temperature: Option<f64>,
    ) -> Result<GrammarMatcher, crate::error::BindingError> {
        let matcher = xgrammar::matcher::GrammarMatcher::with_options(
            compiled_grammar.as_ref(),
            override_stop_tokens,
            terminate_without_stop_token,
            default_temperature.map(|temperature| temperature as f32),
        )
        .map_err(map_error)?;
        Ok(GrammarMatcher {
            inner: Arc::new(Mutex::new(matcher)),
        })
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

    /// Accepts the stop token, terminating the matcher. Returns whether it was accepted.
    #[bindings::export(Method)]
    pub fn accept_stop_token(&self) -> bool {
        self.lock().accept_stop_token()
    }

    /// The tokens acceptable next, as an owned bitmask row.
    #[bindings::export(Method)]
    pub fn next_token_bitmask(&self) -> Result<TokenBitmask, crate::error::BindingError> {
        let mut matcher = self.lock();
        let vocab_size = matcher.tokenizer_info().vocab_size();
        let mut words = allocate_token_bitmask(1, vocab_size);
        let shape = [1, i64::from(get_bitmask_size(vocab_size))];
        let need_apply = matcher.fill_next_token_bitmask(&mut words, &shape, 0).map_err(map_error)?;
        Ok(TokenBitmask {
            words,
            need_apply,
        })
    }

    /// The longest byte string that will be valid according to the current syntax, for
    /// jump-forward decoding. Does not change the matcher state.
    #[bindings::export(Method)]
    pub fn find_jump_forward_bytes(&self) -> Result<Vec<u8>, crate::error::BindingError> {
        self.lock().find_jump_forward_string().map_err(map_error)
    }

    /// The jump-forward bytes as a string; fails when they are not valid UTF-8.
    #[bindings::export(Method)]
    pub fn find_jump_forward_string(&self) -> Result<String, crate::error::BindingError> {
        String::from_utf8(self.find_jump_forward_bytes()?).map_err(map_error)
    }

    /// The values captured so far, in completion order. With `deduplicate`, only the last
    /// (longest) completion of each rule occurrence is kept.
    #[bindings::export(Method)]
    pub fn capture_values(
        &self,
        deduplicate: bool,
    ) -> Vec<Capture> {
        self.lock().get_captures(deduplicate).into_iter().map(Capture::from).collect()
    }

    /// The tokenizer info the matcher was built with.
    #[bindings::export(Method)]
    pub fn tokenizer_info(&self) -> TokenizerInfo {
        TokenizerInfo::from(self.lock().tokenizer_info().clone())
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

    /// The sampling temperature of the current position, if any rule or default specifies one.
    #[bindings::export(Method)]
    pub fn temperature(&self) -> Option<f64> {
        self.lock().temperature().map(f64::from)
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
    ) -> Result<(), crate::error::BindingError> {
        self.lock().rollback(num_tokens).map_err(map_error)
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
    use pyo3::{prelude::*, types::PyBytes};

    use super::GrammarMatcher;
    use crate::{
        bitmask_util::{read_i64_1d, with_writable_buffer},
        error::map_error,
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
            with_writable_buffer::<i32, _>(py, bitmask, "The provided bitmask", |buffer, shape| {
                self.lock().fill_next_token_bitmask(buffer, shape, index).map_err(map_error)
            })
        }

        #[pyo3(name = "_debug_print_internal_state")]
        fn debug_print_internal_state_py(&self) -> String {
            self.lock().debug_print_internal_state()
        }

        #[pyo3(name = "get_captures")]
        fn get_captures_py<'py>(
            &self,
            py: Python<'py>,
            deduplicate: bool,
        ) -> Vec<(String, Bound<'py, PyBytes>)> {
            self.lock()
                .get_captures(deduplicate)
                .into_iter()
                .map(|(name, value)| (name, PyBytes::new(py, &value)))
                .collect()
        }

        #[pyo3(name = "traverse_draft_tree")]
        #[pyo3(signature = (retrieve_next_token, retrieve_next_sibling, draft_tokens, token_bitmask, time_threshold, temperatures=None))]
        #[allow(clippy::too_many_arguments)]
        fn traverse_draft_tree_py(
            &self,
            py: Python<'_>,
            retrieve_next_token: &Bound<'_, PyAny>,
            retrieve_next_sibling: &Bound<'_, PyAny>,
            draft_tokens: &Bound<'_, PyAny>,
            token_bitmask: &Bound<'_, PyAny>,
            time_threshold: f64,
            temperatures: Option<&Bound<'_, PyAny>>,
        ) -> PyResult<bool> {
            let next_token = read_i64_1d(py, retrieve_next_token, "retrieve_next_token")?;
            let next_sibling = read_i64_1d(py, retrieve_next_sibling, "retrieve_next_sibling")?;
            let tokens = read_i64_1d(py, draft_tokens, "draft_tokens")?;
            with_writable_buffer::<i32, _>(py, token_bitmask, "The token_bitmask tensor", |bitmask, shape| {
                if shape.len() != 2 {
                    return Err(map_error("The token_bitmask tensor must be a 2D int32 tensor"));
                }
                let mut traverse = |temperatures: Option<&mut [f32]>| {
                    self.lock()
                        .traverse_draft_tree(&next_token, &next_sibling, &tokens, bitmask, time_threshold, temperatures)
                        .map_err(map_error)
                };
                match temperatures {
                    None => traverse(None),
                    Some(temperatures) => {
                        with_writable_buffer::<f32, _>(py, temperatures, "The temperatures tensor", |values, shape| {
                            if shape.len() != 1 {
                                return Err(map_error("The temperatures tensor must be a 1D float32 tensor"));
                            }
                            traverse(Some(values))
                        })
                    },
                }
            })
        }
    }
}
