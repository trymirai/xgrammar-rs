//! Helpers for reading/writing int32 bitmask buffers from Python array-likes.

use pyo3::{
    exceptions::PyRuntimeError,
    prelude::*,
    types::{PyAnyMethods, PyEllipsis},
};

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
    let mut scratch = flat.call_method0("tolist")?.extract::<Vec<i32>>()?;
    let result = f(&mut scratch)?;
    flat.set_item(PyEllipsis::get(py), scratch)?;
    Ok(result)
}

/// Reads a one-dimensional CPU int64 tensor or array.
pub fn read_i64_1d(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
    name: &str,
) -> PyResult<Vec<i64>> {
    let np = py.import("numpy")?;
    let arr = if obj.hasattr("numpy")? {
        let device = obj.getattr("device")?;
        let device_type = device.getattr("type")?.extract::<String>()?;
        if device_type != "cpu" {
            return Err(PyRuntimeError::new_err(format!("{name} must be on CPU")));
        }
        obj.call_method0("contiguous")?.call_method0("numpy")?
    } else {
        np.call_method1("asarray", (obj,))?
    };
    let shape = arr.getattr("shape")?.extract::<Vec<i64>>()?;
    if shape.len() != 1 {
        return Err(PyRuntimeError::new_err(format!("{name} must be a 1D int64 tensor")));
    }
    let dtype = arr.getattr("dtype")?;
    let kind = dtype.getattr("kind")?.extract::<String>()?;
    let itemsize = dtype.getattr("itemsize")?.extract::<i32>()?;
    if kind != "i" || itemsize != 8 {
        return Err(PyRuntimeError::new_err(format!("{name} must be a 1D int64 tensor")));
    }
    arr.call_method0("tolist")?.extract::<Vec<i64>>()
}

/// Returns the shape of a two-dimensional CPU int32 tensor or array.
pub fn i32_shape_2d(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
    name: &str,
) -> PyResult<Vec<i64>> {
    let arr = resolve_i32_array(py, obj)?;
    let shape = arr.getattr("shape")?.extract::<Vec<i64>>()?;
    if shape.len() != 2 {
        return Err(PyRuntimeError::new_err(format!("{name} must be a 2D int32 tensor")));
    }
    Ok(shape)
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
