//! `BatchGrammarMatcher` PyO3 binding matching the upstream C++ API shape.

use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyModuleMethods};

use crate::{bitmask_util::with_writable_i32_buffer, matcher::GrammarMatcher};

/// Batched matcher front-end (thread pool deferred to the perf milestone).
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
        let _ = self.max_threads;
        with_writable_i32_buffer(py, bitmask, |buf| {
            for (i, m) in matchers.iter_mut().enumerate() {
                let index = indices.as_ref().map_or(i as i32, |idx| idx[i]);
                m.inner.fill_next_token_bitmask(buf, index).map_err(|error| {
                    PyRuntimeError::new_err(error.to_string())
                })?;
            }
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
