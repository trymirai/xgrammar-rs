//! CPU bitmask-apply kernel exposed to Python (`_core.kernels`).
//!
//! The Python CPU kernel passes raw `data_ptr()` values (usize) and shape/stride tuples
//! from torch tensors. We write to memory-mapped logit buffers in-place.

#[cfg(feature = "bindings-pyo3")]
use pyo3::{exceptions::PyValueError, prelude::*, wrap_pyfunction};

#[cfg(feature = "bindings-pyo3")]
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    use pyo3::types::PyModuleMethods;
    m.add_function(wrap_pyfunction!(apply_token_bitmask_inplace_cpu, m)?)?;
    Ok(())
}

/// Apply the bitmask to logits in-place on CPU, operating on raw tensor memory.
///
/// Called by `python/xgrammar/kernels/apply_token_bitmask_inplace_cpu.py` with:
/// - `logits_ptr`: `logits.data_ptr()` — pointer to logit memory
/// - `logits_shape`: `(batch, vocab)` or `(1, vocab)` for 1-D logits
/// - `logits_stride`: stride tuple
/// - `bitmask_ptr`: `bitmask.data_ptr()` — pointer to int32 bitmask memory
/// - `bitmask_shape`: `(batch, words)` or `(1, words)` for 1-D bitmask
/// - `bitmask_stride`: stride tuple
/// - `vocab_size`: effective vocabulary size
/// - `indices`: optional list of batch indices to mask
/// - `dtype`: `"float32"`, `"bfloat16"`, or `"float16"`
#[cfg(feature = "bindings-pyo3")]
#[pyfunction]
#[allow(clippy::too_many_arguments)]
fn apply_token_bitmask_inplace_cpu(
    _py: Python<'_>,
    logits_ptr: usize,
    logits_shape: (usize, usize),
    logits_stride: (usize, usize),
    bitmask_ptr: usize,
    bitmask_shape: (usize, usize),
    _bitmask_stride: (usize, usize),
    vocab_size: usize,
    indices: Option<Vec<usize>>,
    dtype: &str,
) -> PyResult<()> {
    let (batch, _vocab) = logits_shape;
    let (log_stride_batch, log_stride_elem) = logits_stride;
    let (_bm_batch, bm_words) = bitmask_shape;

    let rows: Box<dyn Iterator<Item = usize>> = if let Some(idx) = indices {
        Box::new(idx.into_iter())
    } else {
        Box::new(0..batch)
    };

    match dtype {
        "float32" => {
            for row in rows {
                // SAFETY: caller guarantees ptr/shape/stride describe a live CPU f32 tensor.
                let logit_ptr = unsafe {
                    (logits_ptr as *mut f32).add(row * log_stride_batch)
                };
                let bitmask_row = unsafe {
                    let bm_row = row.min(bm_words.saturating_sub(1));
                    let base =
                        (bitmask_ptr as *const i32).add(bm_row * bm_words);
                    std::slice::from_raw_parts(base, bm_words)
                };
                apply_row_f32(
                    logit_ptr,
                    bitmask_row,
                    vocab_size,
                    log_stride_elem,
                );
            }
        },
        "bfloat16" => {
            for row in rows {
                // SAFETY: caller guarantees ptr/shape/stride describe a live CPU bf16 tensor.
                let logit_ptr = unsafe {
                    (logits_ptr as *mut u16).add(row * log_stride_batch)
                };
                let bitmask_row = unsafe {
                    let bm_row = row.min(bm_words.saturating_sub(1));
                    let base =
                        (bitmask_ptr as *const i32).add(bm_row * bm_words);
                    std::slice::from_raw_parts(base, bm_words)
                };
                apply_row_bf16(
                    logit_ptr,
                    bitmask_row,
                    vocab_size,
                    log_stride_elem,
                );
            }
        },
        "float16" => {
            for row in rows {
                // SAFETY: caller guarantees ptr/shape/stride describe a live CPU f16 tensor.
                let logit_ptr = unsafe {
                    (logits_ptr as *mut u16).add(row * log_stride_batch)
                };
                let bitmask_row = unsafe {
                    let bm_row = row.min(bm_words.saturating_sub(1));
                    let base =
                        (bitmask_ptr as *const i32).add(bm_row * bm_words);
                    std::slice::from_raw_parts(base, bm_words)
                };
                apply_row_f16(
                    logit_ptr,
                    bitmask_row,
                    vocab_size,
                    log_stride_elem,
                );
            }
        },
        other => {
            return Err(PyValueError::new_err(format!(
                "unsupported dtype: {other}"
            )));
        },
    }
    Ok(())
}

fn bit_is_set(
    bitmask: &[i32],
    token: usize,
) -> bool {
    let word = token / 32;
    let bit = token % 32;
    word < bitmask.len() && (bitmask[word] >> bit) & 1 == 1
}

fn apply_row_f32(
    ptr: *mut f32,
    bitmask: &[i32],
    vocab_size: usize,
    stride: usize,
) {
    for token in 0..vocab_size {
        if !bit_is_set(bitmask, token) {
            // SAFETY: caller guarantees ptr + token*stride is in-bounds for the logit tensor.
            unsafe {
                *ptr.add(token * stride) = f32::NEG_INFINITY;
            }
        }
    }
}

fn apply_row_bf16(
    ptr: *mut u16,
    bitmask: &[i32],
    vocab_size: usize,
    stride: usize,
) {
    // bf16 -inf = 0xFF80
    for token in 0..vocab_size {
        if !bit_is_set(bitmask, token) {
            // SAFETY: caller guarantees ptr + token*stride is in-bounds.
            unsafe { *ptr.add(token * stride) = 0xFF80u16 };
        }
    }
}

fn apply_row_f16(
    ptr: *mut u16,
    bitmask: &[i32],
    vocab_size: usize,
    stride: usize,
) {
    // fp16 -inf = 0xFC00
    for token in 0..vocab_size {
        if !bit_is_set(bitmask, token) {
            // SAFETY: caller guarantees ptr + token*stride is in-bounds.
            unsafe { *ptr.add(token * stride) = 0xFC00u16 };
        }
    }
}
