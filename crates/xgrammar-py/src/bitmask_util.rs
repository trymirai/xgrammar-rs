//! Helpers for borrowing CPU tensor buffers (bitmasks, temperatures) from Python array-likes.

use pyo3::{exceptions::PyRuntimeError, intern, prelude::*, sync::PyOnceLock};

/// A tensor element type the bindings write into: its numpy dtype kind and item size, and the
/// matching torch dtype.
pub trait BufferElement: Copy {
    /// The numpy dtype kind (`"i"` or `"f"`).
    const KIND: &'static str;
    /// The torch dtype attribute (`"int32"` or `"float32"`).
    const TORCH_DTYPE: &'static str;
    /// The size of one element in bytes.
    const ITEM_SIZE: usize = std::mem::size_of::<Self>();
    /// The cached `torch.<TORCH_DTYPE>` object, compared by identity.
    fn torch_dtype_cell() -> &'static PyOnceLock<Py<PyAny>>;
}

impl BufferElement for i32 {
    const KIND: &'static str = "i";
    const TORCH_DTYPE: &'static str = "int32";

    fn torch_dtype_cell() -> &'static PyOnceLock<Py<PyAny>> {
        static DTYPE: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
        &DTYPE
    }
}

impl BufferElement for f32 {
    const KIND: &'static str = "f";
    const TORCH_DTYPE: &'static str = "float32";

    fn torch_dtype_cell() -> &'static PyOnceLock<Py<PyAny>> {
        static DTYPE: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
        &DTYPE
    }
}

fn dtype_error<T: BufferElement>(describe: &str) -> PyErr {
    PyRuntimeError::new_err(format!("{describe} must have dtype {}", T::TORCH_DTYPE))
}

/// Invokes `f` with a mutable view of the contiguous CPU buffer backing `obj` (a torch tensor or
/// a numpy array) and its shape. The view aliases the tensor memory, so writes are visible to
/// the caller without copying. Errors use `describe` as the subject, e.g. "The temperatures
/// tensor".
pub fn with_writable_buffer<T: BufferElement, R>(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
    describe: &str,
    f: impl FnOnce(&mut [T], &[i64]) -> PyResult<R>,
) -> PyResult<R> {
    // Torch tensors (the common case, once per decoded token) are validated with a few cheap
    // attribute reads instead of a numpy round trip, like upstream's DLPack conversion.
    if let Ok(data_ptr) = obj.getattr(intern!(py, "data_ptr")) {
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
        let address = data_ptr.call0()?.extract::<usize>()?;
        // SAFETY: `obj` is a live, contiguous CPU tensor of `len` elements of `T` (checked above;
        // `data_ptr` includes the storage offset), kept alive by the caller for this call. The
        // interpreter is attached and the slice does not escape `f`.
        let buffer = unsafe { std::slice::from_raw_parts_mut(address as *mut T, len) };
        return f(buffer, &shape);
    }
    let array = if obj.hasattr("numpy")? {
        let device_type = obj.getattr("device")?.getattr("type")?.extract::<String>()?;
        if device_type != "cpu" {
            return Err(PyRuntimeError::new_err(format!("{describe} must be on CPU")));
        }
        if !obj.call_method0("is_contiguous")?.extract::<bool>()? {
            return Err(PyRuntimeError::new_err(format!("{describe} must be contiguous")));
        }
        obj.call_method0("numpy")?
    } else {
        py.import("numpy")?.call_method1("asarray", (obj,))?
    };
    let dtype = array.getattr("dtype")?;
    if dtype.getattr("kind")?.extract::<String>()? != T::KIND
        || dtype.getattr("itemsize")?.extract::<usize>()? != T::ITEM_SIZE
    {
        return Err(dtype_error::<T>(describe));
    }
    let flags = array.getattr("flags")?;
    if !flags.getattr("writeable")?.extract::<bool>()? {
        return Err(PyRuntimeError::new_err(format!("{describe} is read-only")));
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
    // SAFETY: `array` is a live, writeable, C-contiguous CPU buffer of `len` elements of `T`
    // (checked above), kept alive by `obj`/`array` for the duration of this call. The GIL is held
    // and the slice does not escape `f`.
    let buffer = unsafe { std::slice::from_raw_parts_mut(address as *mut T, len) };
    f(buffer, &shape)
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
