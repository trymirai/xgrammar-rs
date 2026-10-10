//! Element types of the CPU buffers the bindings write into.

use pyo3::{prelude::*, sync::PyOnceLock};

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
