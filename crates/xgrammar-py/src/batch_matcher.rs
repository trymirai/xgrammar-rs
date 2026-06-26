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
        mut matchers: Vec<PyRefMut<'_, GrammarMatcher>>,
        bitmask: &Bound<'_, PyAny>,
        indices: Option<Vec<i32>>,
        _debug_print: bool,
    ) -> PyResult<()> {
        let n_threads = self.max_threads as usize;

        with_writable_i32_buffer(py, bitmask, |buf| {
            // Collect (raw_matcher_ptr, bitmask_row_index) pairs.
            // SAFETY invariants maintained throughout:
            //  1. Each pointer is unique — they come from distinct `PyRefMut`
            //     borrows, which are exclusive references.
            //  2. Each matcher writes to a distinct row of `buf` (unique index).
            //  3. `buf` lives for the entire duration of this closure.
            struct Work {
                matcher: *mut xgrammar::matcher::GrammarMatcher,
                buf: *mut i32,
                buf_len: usize,
                index: i32,
            }

            // SAFETY: GrammarMatcher contains only Rust types (Arc<Grammar>,
            // per-matcher parser state). It is Send. The buf pointer is also
            // only accessed through non-overlapping ranges.
            unsafe impl Send for Work {}

            let buf_ptr = buf.as_mut_ptr();
            let buf_len = buf.len();

            let work: Vec<Work> = matchers
                .iter_mut()
                .enumerate()
                .map(|(i, m)| {
                    let index = indices.as_ref().map_or(i as i32, |idx| idx[i]);
                    Work {
                        matcher: &mut m.inner as *mut _,
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
                    .unwrap_or_else(|_| {
                        rayon::ThreadPoolBuilder::new()
                            .build()
                            .expect("rayon pool")
                    });

                pool.install(|| {
                    work.into_par_iter().for_each(|w| {
                        // SAFETY: no two Work items share the same matcher ptr
                        // or the same bitmask row (index is unique per item).
                        unsafe {
                            let matcher = &mut *w.matcher;
                            let buf_slice = std::slice::from_raw_parts_mut(
                                w.buf, w.buf_len,
                            );
                            let _ = matcher
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
        mut matchers: Vec<PyRefMut<'_, GrammarMatcher>>,
        tokens: Vec<i32>,
        _debug_print: bool,
    ) -> PyResult<Vec<bool>> {
        if matchers.len() != tokens.len() {
            return Err(PyRuntimeError::new_err(
                "matchers and tokens length mismatch",
            ));
        }
        Ok(matchers
            .iter_mut()
            .zip(tokens)
            .map(|(m, token)| m.inner.accept_token(token))
            .collect())
    }

    #[staticmethod]
    #[pyo3(name = "batch_accept_string")]
    fn batch_accept_string(
        mut matchers: Vec<PyRefMut<'_, GrammarMatcher>>,
        strings: Vec<Bound<'_, PyAny>>,
        _debug_print: bool,
    ) -> PyResult<Vec<bool>> {
        if matchers.len() != strings.len() {
            return Err(PyRuntimeError::new_err(
                "matchers and strings length mismatch",
            ));
        }
        let mut results = Vec::with_capacity(matchers.len());
        for (m, s) in matchers.iter_mut().zip(strings) {
            let ok = if let Ok(text) = s.extract::<String>() {
                m.inner.accept_string(&text)
            } else {
                m.inner.accept_bytes(&s.extract::<Vec<u8>>()?)
            };
            results.push(ok);
        }
        Ok(results)
    }

    #[staticmethod]
    #[pyo3(name = "batch_rollback")]
    fn batch_rollback(
        mut matchers: Vec<PyRefMut<'_, GrammarMatcher>>,
        num_tokens: Vec<i32>,
    ) -> PyResult<()> {
        if matchers.len() != num_tokens.len() {
            return Err(PyRuntimeError::new_err(
                "matchers and num_tokens length mismatch",
            ));
        }
        for (m, &n) in matchers.iter_mut().zip(&num_tokens) {
            m.inner.rollback(n);
        }
        Ok(())
    }
}

fn parse_max_threads(value: &Bound<'_, PyAny>) -> PyResult<i32> {
    if let Ok(text) = value.extract::<String>() {
        if text == "auto" {
            return Ok(std::thread::available_parallelism()
                .map(|p| (p.get() / 2).max(1) as i32)
                .unwrap_or(1));
        }
    }
    value.extract::<i32>().map_err(|_| {
        PyRuntimeError::new_err("max_threads must be an integer or \"auto\"")
    })
}

/// Registers [`BatchGrammarMatcher`] on the root module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<BatchGrammarMatcher>()?;
    Ok(())
}
