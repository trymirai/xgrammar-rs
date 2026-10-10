//! Helpers for borrowing CPU tensor buffers (bitmasks, temperatures) from Python array-likes.

mod buffer_element;

use std::{mem::size_of, ptr};

pub use buffer_element::BufferElement;
use pyo3::{
    exceptions::{PyBufferError, PyRuntimeError},
    intern,
    prelude::*,
    sync::PyOnceLock,
    types::{PyDict, PyMemoryView, PyType},
};

fn dtype_error<T: BufferElement>(describe: &str) -> PyErr {
    PyRuntimeError::new_err(format!("{describe} must have dtype {}", T::TORCH_DTYPE))
}

/// The cached `torch.Tensor` type, or `None` when its module is not loaded. Does not import torch.
fn torch_tensor_type(py: Python<'_>) -> PyResult<Option<&Py<PyType>>> {
    static TENSOR: PyOnceLock<Py<PyType>> = PyOnceLock::new();
    if let Some(tensor) = TENSOR.get(py) {
        return Ok(Some(tensor));
    }
    let modules = py.import("sys")?.getattr(intern!(py, "modules"))?.cast_into::<PyDict>()?;
    let Some(torch) = modules.get_item(intern!(py, "torch"))? else {
        return Ok(None);
    };
    TENSOR.get_or_try_init(py, || Ok(torch.getattr(intern!(py, "Tensor"))?.cast_into::<PyType>()?.unbind())).map(Some)
}

/// Runs `f` on the `len` elements of `T` at `address`. Rust slices must be non-null and aligned,
/// so a misaligned buffer (which numpy and `torch.frombuffer` allow) is filled through aligned
/// scratch that starts as a copy and is copied back after `f`, even when `f` fails: entries `f`
/// wrote and entries it left untouched read exactly as after a direct write.
///
/// # Safety
/// `address` must be valid for reads and writes of `len` elements of `T` for the whole call, with
/// no other access to that memory meanwhile. Its total byte size must not exceed `isize::MAX`.
unsafe fn with_buffer_at<T: BufferElement, R>(
    address: usize,
    len: usize,
    shape: &[i64],
    f: impl FnOnce(&mut [T], &[i64]) -> PyResult<R>,
) -> PyResult<R> {
    let data = address as *mut T;
    if data.is_null() {
        return Err(PyRuntimeError::new_err("The buffer has no data pointer"));
    }
    if data.is_aligned() {
        // SAFETY: non-null, aligned, and valid for `len` elements (caller contract).
        return f(unsafe { std::slice::from_raw_parts_mut(data, len) }, shape);
    }
    let bytes = len * size_of::<T>();
    let mut scratch = Vec::<T>::with_capacity(len);
    // SAFETY: byte copies have no alignment requirement; `data` is valid for `bytes` (caller
    // contract) and `scratch` has capacity for them. `BufferElement` types (`i32`, `f32`) are valid
    // for every bit pattern, so all `len` elements are initialized.
    unsafe {
        ptr::copy_nonoverlapping(data.cast::<u8>(), scratch.as_mut_ptr().cast::<u8>(), bytes);
        scratch.set_len(len);
    }
    let result = f(&mut scratch, shape);
    // SAFETY: as above, in the other direction.
    unsafe { ptr::copy_nonoverlapping(scratch.as_ptr().cast::<u8>(), data.cast::<u8>(), bytes) };
    result
}

/// Invokes `f` with a mutable view of the contiguous CPU buffer backing `obj` (a `torch.Tensor`, or
/// any object exporting a writable native-endian buffer, such as a numpy array) and its shape. The
/// view aliases the object's own memory, so writes are visible to the caller; only a misaligned
/// buffer goes through a copy-back scratch (see [`with_buffer_at`]). Inputs that could only be
/// converted by copying are rejected. The caller must not resize, free, or concurrently write the
/// buffer during the call (as with upstream's DLPack borrow). Errors use `describe` as the subject,
/// e.g. "The temperatures tensor".
pub fn with_writable_buffer<T: BufferElement, R>(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
    describe: &str,
    f: impl FnOnce(&mut [T], &[i64]) -> PyResult<R>,
) -> PyResult<R> {
    // Torch tensors (the common case, once per decoded token) are validated with a few cheap
    // attribute reads instead of a numpy round trip, like upstream's DLPack conversion. Only real
    // tensors qualify: objects imitating their attributes go through the buffer protocol below.
    let is_tensor = match torch_tensor_type(py)? {
        Some(tensor) => obj.is_instance(tensor.bind(py))?,
        None => false,
    };
    if is_tensor {
        if !obj.getattr(intern!(py, "is_cpu"))?.is_truthy()? {
            return Err(PyRuntimeError::new_err(format!("{describe} must be on CPU")));
        }
        let torch_dtype = T::torch_dtype_cell()
            .get_or_try_init(py, || py.import("torch")?.getattr(T::TORCH_DTYPE).map(Bound::unbind))?;
        if !obj.getattr(intern!(py, "dtype"))?.is(torch_dtype.bind(py)) {
            return Err(dtype_error::<T>(describe));
        }
        if obj.getattr(intern!(py, "requires_grad"))?.is_truthy()? {
            return Err(PyRuntimeError::new_err(format!("{describe} must not require grad")));
        }
        if !obj.call_method0(intern!(py, "is_contiguous"))?.is_truthy()? {
            return Err(PyRuntimeError::new_err(format!("{describe} must be contiguous")));
        }
        let shape = obj.getattr(intern!(py, "shape"))?.extract::<Vec<i64>>()?;
        let len = shape.iter().product::<i64>() as usize;
        if len == 0 {
            return f(&mut [], &shape);
        }
        let address = obj.call_method0(intern!(py, "data_ptr"))?.extract::<usize>()?;
        // SAFETY: `obj` is a live, contiguous CPU tensor of `len` elements of `T` (checked above;
        // `data_ptr` includes the storage offset), kept alive by the caller for this call.
        return unsafe { with_buffer_at(address, len, &shape, f) };
    }
    // Borrow the object's own buffer (a `TypeError` for objects without one, such as lists), then
    // view it through numpy only to read its dtype, flags, and address: no copy is ever filled.
    let view = PyMemoryView::from(obj)?;
    let array = py.import("numpy")?.call_method1("asarray", (view,))?;
    let dtype = array.getattr("dtype")?;
    if dtype.getattr("kind")?.extract::<String>()? != T::KIND
        || dtype.getattr("itemsize")?.extract::<usize>()? != T::ITEM_SIZE
    {
        return Err(dtype_error::<T>(describe));
    }
    if !dtype.getattr("isnative")?.extract::<bool>()? {
        return Err(PyBufferError::new_err(format!("{describe} must use the native byte order")));
    }
    let flags = array.getattr("flags")?;
    if !flags.getattr("writeable")?.extract::<bool>()? {
        return Err(PyBufferError::new_err(format!("{describe} is read-only")));
    }
    if !flags.getattr("c_contiguous")?.extract::<bool>()? {
        return Err(PyRuntimeError::new_err(format!("{describe} must be contiguous")));
    }
    let shape = array.getattr("shape")?.extract::<Vec<i64>>()?;
    let len = array.getattr("size")?.extract::<usize>()?;
    if len == 0 {
        return f(&mut [], &shape);
    }
    let address = array.getattr("__array_interface__")?.get_item("data")?.get_item(0)?.extract::<usize>()?;
    // SAFETY: `array` views the live, writeable, C-contiguous, native-endian buffer of `obj` holding
    // `len` elements of `T` (checked above), kept alive by `obj`/`view` for the duration of this
    // call.
    unsafe { with_buffer_at(address, len, &shape, f) }
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
            return Err(PyRuntimeError::new_err(format!("The {name} tensor must be on CPU")));
        }
        obj.call_method0("contiguous")?.call_method0("numpy")?
    } else {
        np.call_method1("asarray", (obj,))?
    };
    let shape = arr.getattr("shape")?.extract::<Vec<i64>>()?;
    let dtype = arr.getattr("dtype")?;
    let kind = dtype.getattr("kind")?.extract::<String>()?;
    let itemsize = dtype.getattr("itemsize")?.extract::<i32>()?;
    if shape.len() != 1 || kind != "i" || itemsize != 8 {
        return Err(PyRuntimeError::new_err(format!("The {name} tensor must be a 1D int64 tensor")));
    }
    arr.call_method0("tolist")?.extract::<Vec<i64>>()
}
