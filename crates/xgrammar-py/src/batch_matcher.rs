//! `BatchGrammarMatcher` PyO3 binding matching the upstream C++ API shape.

use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex, MutexGuard, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyModuleMethods};
use xgrammar::{MatcherError, matcher::bitmask_row_range};

use crate::{bitmask_util::with_writable_buffer, error::map_error, matcher::GrammarMatcher};

/// Batched matcher front-end. `batch_fill_next_token_bitmask` fills bitmask rows in parallel on a
/// persistent rayon pool (upstream rebuilds its pool on every call) — each matcher writes its own
/// row (no aliasing), so concurrent writes are safe. With one thread the rows are filled serially.
#[pyclass]
pub struct BatchGrammarMatcher {
    pool: Option<rayon::ThreadPool>,
}

#[pymethods]
impl BatchGrammarMatcher {
    #[new]
    fn new(max_threads: &Bound<'_, PyAny>) -> PyResult<Self> {
        let max_threads = parse_max_threads(max_threads)?;
        let pool = if max_threads > 1 {
            Some(rayon::ThreadPoolBuilder::new().num_threads(max_threads as usize).build().map_err(map_error)?)
        } else {
            None
        };
        Ok(Self {
            pool,
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
        if let Some(indices) = &indices {
            if indices.len() != matchers.len() {
                return Err(PyRuntimeError::new_err(format!(
                    "The size of indices ({}) should be the same as the size of matchers ({}).",
                    indices.len(),
                    matchers.len()
                )));
            }
        }

        with_writable_buffer::<i32, _>(py, bitmask, "The provided bitmask", |buf, shape| {
            let rows = shape.first().copied().unwrap_or(0);
            let mut row_indices = Vec::with_capacity(matchers.len());
            for batch_id in 0..matchers.len() {
                let index = indices.as_ref().map_or(batch_id as i32, |indices| indices[batch_id]);
                if index < 0 || i64::from(index) >= rows {
                    return Err(PyRuntimeError::new_err(format!(
                        "The index {index} is out of range [0, {rows}) for batch_id {batch_id}."
                    )));
                }
                row_indices.push(index);
            }
            let handles: Vec<_> = matchers.iter().map(|matcher| Arc::clone(&matcher.inner)).collect();
            // Release the GIL while doing the Rust work.
            py.detach(|| fill_rows(&handles, &row_indices, buf, shape, self.pool.as_ref())).map_err(map_error)
        })
    }

    #[staticmethod]
    #[pyo3(name = "batch_fill_temperature")]
    fn batch_fill_temperature(
        py: Python<'_>,
        matchers: Vec<PyRef<'_, GrammarMatcher>>,
        temperatures: &Bound<'_, PyAny>,
        indices: Option<Vec<i32>>,
    ) -> PyResult<()> {
        with_writable_buffer::<f32, _>(py, temperatures, "The temperatures tensor", |values, shape| {
            if shape.len() != 1 {
                return Err(PyRuntimeError::new_err("The temperatures tensor must be a 1D float32 tensor"));
            }
            // Each matcher is locked only while its temperature is read, so a matcher listed twice
            // cannot deadlock.
            xgrammar::matcher::BatchGrammarMatcher::batch_fill_temperature(
                matchers.iter().map(|matcher| matcher.lock()),
                values,
                indices.as_deref(),
            )
            .map_err(map_error)
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
            return Err(PyRuntimeError::new_err(format!(
                "The size of matchers ({}) and token_ids ({}) should be the same.",
                matchers.len(),
                tokens.len()
            )));
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
            return Err(PyRuntimeError::new_err(format!(
                "The size of matchers ({}) and input_strs ({}) should be the same.",
                matchers.len(),
                strings.len()
            )));
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
            return Err(PyRuntimeError::new_err(format!(
                "The size of matchers ({}) and num_tokens ({}) should be the same.",
                matchers.len(),
                num_tokens.len()
            )));
        }
        for (m, &n) in matchers.iter().zip(&num_tokens) {
            m.lock().rollback(n).map_err(map_error)?;
        }
        Ok(())
    }
}

/// Fills bitmask row `indices[i]` with the mask of `matchers[i]`. Distinct rows of a 2D bitmask are
/// split into disjoint slices and filled in parallel on `pool`; otherwise the rows are filled in
/// order on this thread.
///
/// # Errors
/// Returns the first matcher error in batch order.
fn fill_rows(
    matchers: &[Arc<Mutex<xgrammar::matcher::GrammarMatcher>>],
    indices: &[i32],
    bitmask: &mut [i32],
    shape: &[i64],
    pool: Option<&rayon::ThreadPool>,
) -> Result<(), MatcherError> {
    let row_len = match *shape {
        [_, cols] => usize::try_from(cols).ok().filter(|&cols| cols > 0),
        _ => None,
    };
    let distinct = indices.iter().collect::<HashSet<_>>().len() == indices.len();
    let (Some(pool), Some(row_len), true) = (pool, row_len, distinct) else {
        for (matcher, &index) in matchers.iter().zip(indices) {
            lock(matcher).fill_next_token_bitmask(bitmask, shape, index)?;
        }
        return Ok(());
    };
    let mut rows: Vec<Option<&mut [i32]>> = bitmask.chunks_mut(row_len).map(Some).collect();
    // Each batch entry owns its (distinct) row. The caller starts filling right away and pool
    // helpers join in, claiming entries in order (upstream spawns a fresh pool on every call).
    let slots: Vec<Mutex<Option<&mut [i32]>>> =
        indices.iter().map(|&index| Mutex::new(rows[index as usize].take())).collect();
    let results: Vec<OnceLock<Result<bool, MatcherError>>> = indices.iter().map(|_| OnceLock::new()).collect();
    let next = AtomicUsize::new(0);
    let claim_rows = || {
        loop {
            let entry = next.fetch_add(1, Ordering::Relaxed);
            if entry >= indices.len() {
                return;
            }
            let row = slots[entry].lock().expect("row slot mutex poisoned").take().expect("each entry is claimed once");
            let _ = results[entry].set(fill_row(&matchers[entry], row, shape, indices[entry]));
        }
    };
    pool.in_place_scope(|scope| {
        for _ in 0..pool.current_num_threads().min(indices.len().saturating_sub(1)) {
            scope.spawn(|_| claim_rows());
        }
        claim_rows();
    });
    results.into_iter().try_for_each(|result| result.into_inner().expect("every entry is filled").map(|_| ()))
}

/// Fills `row`, which is row `index` of a bitmask of `shape`, with the mask of `matcher`.
fn fill_row(
    matcher: &Mutex<xgrammar::matcher::GrammarMatcher>,
    row: &mut [i32],
    shape: &[i64],
    index: i32,
) -> Result<bool, MatcherError> {
    let mut matcher = lock(matcher);
    // Validate against the whole bitmask so errors read exactly like a direct fill; the row
    // itself is then filled as a one-dimensional bitmask.
    if bitmask_row_range(shape, matcher.tokenizer_info().vocab_size(), index).is_err() {
        return matcher.fill_next_token_bitmask(&mut [], shape, index);
    }
    let row_shape = [row.len() as i64];
    matcher.fill_next_token_bitmask(row, &row_shape, 0)
}

fn lock(matcher: &Mutex<xgrammar::matcher::GrammarMatcher>) -> MutexGuard<'_, xgrammar::matcher::GrammarMatcher> {
    matcher.lock().expect("grammar matcher mutex poisoned")
}

fn parse_max_threads(value: &Bound<'_, PyAny>) -> PyResult<i32> {
    if let Ok(text) = value.extract::<String>() {
        if text == "auto" {
            return Ok(std::thread::available_parallelism().map(|p| (p.get() / 2).max(1) as i32).unwrap_or(1));
        }
        return Err(PyRuntimeError::new_err("max_threads must be an integer or \"auto\""));
    }
    let num_threads =
        value.extract::<i32>().map_err(|_| PyRuntimeError::new_err("max_threads must be an integer or \"auto\""))?;
    if num_threads < 1 {
        return Err(PyRuntimeError::new_err(format!("The num_threads should be at least 1, but got {num_threads}")));
    }
    Ok(num_threads)
}

/// Registers [`BatchGrammarMatcher`] on the root module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<BatchGrammarMatcher>()?;
    Ok(())
}
