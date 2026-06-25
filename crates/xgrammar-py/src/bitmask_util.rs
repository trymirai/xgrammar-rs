//! Helpers for reading/writing int32 bitmask buffers from Python array-likes.

use pyo3::{buffer::PyBuffer, exceptions::PyRuntimeError, prelude::*};

/// Invokes `f` with a mutable view of the int32 data backing `obj`.
///
/// Accepts CPU torch tensors, numpy arrays, or any object convertible via `numpy.asarray`.
pub fn with_writable_i32_buffer<R>(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
    f: impl FnOnce(&mut [i32]) -> PyResult<R>,
) -> PyResult<R> {
    let arr = resolve_i32_array(py, obj)?;
    ensure_writeable(&arr)?;
    let flat = arr.call_method0("ravel")?;
    let buffer = PyBuffer::<i32>::get(&flat)?;
    let cells = buffer.as_mut_slice(py).ok_or_else(|| {
        PyRuntimeError::new_err("bitmask must be C-contiguous int32")
    })?;
    let mut scratch: Vec<i32> = cells.iter().map(|cell| cell.get()).collect();
    let result = f(&mut scratch)?;
    for (cell, value) in cells.iter().zip(scratch.iter()) {
        cell.set(*value);
    }
    Ok(result)
}

/// Reads int32 data from a tensor or numpy array.
pub fn read_i32_buffer(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<Vec<i32>> {
    let arr = resolve_i32_array(py, obj)?;
    let flat = arr.call_method0("ravel")?;
    let buffer = PyBuffer::<i32>::get(&flat)?;
    let cells = buffer.as_slice(py).ok_or_else(|| {
        PyRuntimeError::new_err("bitmask must be C-contiguous int32")
    })?;
    Ok(cells.iter().map(|cell| cell.get()).collect())
}

fn resolve_i32_array<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let np = py.import("numpy")?;
    let arr = if obj.hasattr("numpy")? {
        let device = obj.getattr("device")?;
        let device_type = device.getattr("type")?.extract::<String>()?;
        if device_type != "cpu" {
            return Err(PyRuntimeError::new_err("bitmask must be on CPU"));
        }
        let contiguous = obj.call_method0("contiguous")?;
        let numpy = contiguous.call_method0("numpy")?;
        ensure_int32(&numpy)?
    } else {
        let converted = np.call_method1("asarray", (obj,))?;
        ensure_int32(&converted)?
    };
    Ok(arr)
}

fn ensure_int32<'py>(arr: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let dtype = arr.getattr("dtype")?;
    let kind = dtype.getattr("kind")?.extract::<String>()?;
    if kind != "i" {
        return Err(PyRuntimeError::new_err("bitmask must be int32"));
    }
    let itemsize = dtype.getattr("itemsize")?.extract::<i32>()?;
    if itemsize != 4 {
        return Err(PyRuntimeError::new_err("bitmask must be int32"));
    }
    Ok(arr.clone())
}

fn ensure_writeable(arr: &Bound<'_, PyAny>) -> PyResult<()> {
    let writeable = if arr.hasattr("flags")? {
        arr.getattr("flags")?.getattr("writeable")?.extract::<bool>()?
    } else {
        arr.getattr("writeable")?.extract::<bool>()?
    };
    if !writeable {
        return Err(PyRuntimeError::new_err("bitmask buffer is read-only"));
    }
    Ok(())
}

/// Returns a slice of one batch row within a flattened 2-D bitmask buffer.
pub fn bitmask_row_slice<'a>(
    data: &'a [i32],
    shape: &[i64],
    index: i32,
) -> PyResult<&'a [i32]> {
    if shape.len() != 2 {
        return Err(PyRuntimeError::new_err("bitmask must be 2-dimensional"));
    }
    let row_words = shape[1] as usize;
    let idx = index as usize;
    let start = idx * row_words;
    let end = start + row_words;
    data.get(start..end).ok_or_else(|| {
        PyRuntimeError::new_err(format!(
            "bitmask index {index} out of range for shape {shape:?}"
        ))
    })
}
