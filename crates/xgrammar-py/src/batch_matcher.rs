//! `BatchGrammarMatcher` PyO3 binding matching the upstream C++ API shape.

use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyModuleMethods};

use crate::{bitmask_util::with_writable_i32_buffer, matcher::GrammarMatcher};

/// Batched matcher front-end. `batch_fill_next_token_bitmask` uses rayon to
/// fill bitmask rows in parallel — each matcher operates on its own row (no
/// aliasing), so concurrent writes are safe.
#[pyclass]
pub struct BatchGrammarMatcher {
    max_threads: i32,
}

#[pymethods]
impl BatchGrammarMatcher {
    #[new]
    fn new(max_threads: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            max_threads: parse_max_threads(max_threads)?,
        })
    }

    #[pyo3(name = "batch_fill_next_token_bitmask")]
    fn batch_fill_next_token_bitmask(
        &self,
        py: Python<'_>,
        matchers: Vec<PyRefMut<'_, GrammarMatcher>>,
        bitmask: &Bound<'_, PyAny>,
        indices: Option<Vec<i32>>,
        _debug_print: bool,
    ) -> PyResult<()> {
        let n_threads = self.max_threads as usize;

        with_writable_i32_buffer(py, bitmask, |buf| {
            // Collect matcher handles and bitmask row indices. The matcher locks are independent,
            // and each task writes to its own bitmask row.
            struct Work {
                matcher: std::sync::Arc<std::sync::Mutex<xgrammar::matcher::GrammarMatcher>>,
                buf: *mut i32,
                buf_len: usize,
                index: i32,
            }

            // SAFETY: the only raw pointer is the shared buffer, and each work item writes to a
            // distinct row. Matcher state is protected by its own mutex.
            unsafe impl Send for Work {}

            let buf_ptr = buf.as_mut_ptr();
            let buf_len = buf.len();

            let work: Vec<Work> = matchers
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    let index = indices.as_ref().map_or(i as i32, |idx| idx[i]);
                    Work {
                        matcher: m.inner.clone(),
                        buf: buf_ptr,
                        buf_len,
                        index,
                    }
                })
                .collect();

            // Release the GIL while doing parallel Rust work.
            // In pyo3 0.28, `Python::detach` is the GIL-release API
            // (renamed from `allow_threads` in earlier versions).
            py.detach(|| {
                use rayon::prelude::*;

                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(n_threads)
                    .build()
                    .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().build().expect("rayon pool"));

                pool.install(|| {
                    work.into_par_iter().for_each(|w| {
                        // SAFETY: no two Work items share the same bitmask row.
                        unsafe {
                            let buf_slice = std::slice::from_raw_parts_mut(w.buf, w.buf_len);
                            let _ = w
                                .matcher
                                .lock()
                                .expect("grammar matcher mutex poisoned")
                                .fill_next_token_bitmask(buf_slice, w.index);
                        }
                    });
                });
            });

            Ok(())
        })
    }

    #[staticmethod]
    #[pyo3(name = "batch_accept_token")]
    fn batch_accept_token(
        matchers: Vec<PyRefMut<'_, GrammarMatcher>>,
        tokens: Vec<i32>,
        _debug_print: bool,
    ) -> PyResult<Vec<bool>> {
        if matchers.len() != tokens.len() {
            return Err(PyRuntimeError::new_err("matchers and tokens length mismatch"));
        }
        Ok(matchers.iter().zip(tokens).map(|(m, token)| m.lock().accept_token(token)).collect())
    }

    #[staticmethod]
    #[pyo3(name = "batch_accept_string")]
    fn batch_accept_string(
        matchers: Vec<PyRefMut<'_, GrammarMatcher>>,
        strings: Vec<Bound<'_, PyAny>>,
        _debug_print: bool,
    ) -> PyResult<Vec<bool>> {
        if matchers.len() != strings.len() {
            return Err(PyRuntimeError::new_err("matchers and strings length mismatch"));
        }
        let mut results = Vec::with_capacity(matchers.len());
        for (m, s) in matchers.iter().zip(strings) {
            let ok = if let Ok(text) = s.extract::<String>() {
                m.lock().accept_string(&text)
            } else {
                m.lock().accept_bytes(&s.extract::<Vec<u8>>()?)
            };
            results.push(ok);
        }
        Ok(results)
    }

    #[staticmethod]
    #[pyo3(name = "batch_rollback")]
    fn batch_rollback(
        matchers: Vec<PyRefMut<'_, GrammarMatcher>>,
        num_tokens: Vec<i32>,
    ) -> PyResult<()> {
        if matchers.len() != num_tokens.len() {
            return Err(PyRuntimeError::new_err("matchers and num_tokens length mismatch"));
        }
        for (m, &n) in matchers.iter().zip(&num_tokens) {
            m.lock().rollback(n);
        }
        Ok(())
    }
}

fn parse_max_threads(value: &Bound<'_, PyAny>) -> PyResult<i32> {
    if let Ok(text) = value.extract::<String>() {
        if text == "auto" {
            return Ok(std::thread::available_parallelism().map(|p| (p.get() / 2).max(1) as i32).unwrap_or(1));
        }
    }
    value.extract::<i32>().map_err(|_| PyRuntimeError::new_err("max_threads must be an integer or \"auto\""))
}

/// Registers [`BatchGrammarMatcher`] on the root module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<BatchGrammarMatcher>()?;
    Ok(())
}
