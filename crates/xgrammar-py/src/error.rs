//! Cross-backend error handling.
//!
//! Exported methods return `Result<T, BindingError>`. Under the PyO3 backend, `BindingError`
//! is [`pyo3::PyErr`] so we can raise the precise exception subtypes upstream Python tests
//! expect (`InvalidJSONError`, `DeserializeVersionError`, …). Under the other backends it is
//! the exported [`XgrammarError`] enum, which the `#[bindings::export(Error)]` macro converts
//! to each backend's native error type.

/// The error type carried by exported fallible methods, resolved per active backend.
#[cfg(feature = "bindings-pyo3")]
pub type BindingError = ::pyo3::PyErr;

/// The error type carried by exported fallible methods, resolved per active backend.
#[cfg(not(feature = "bindings-pyo3"))]
pub type BindingError = XgrammarError;

/// A backend-agnostic error wrapping a message — exported so NAPI / UniFFI / wasm get a
/// native error type. (Unused under the PyO3 backend, which uses [`pyo3::PyErr`] directly.)
#[bindings::export(Error)]
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum XgrammarError {
    /// A grammar / schema / serialization operation failed.
    #[error("{message}")]
    Invalid {
        /// The underlying error message.
        message: String,
    },
}

// ---------------------------------------------------------------------------
// PyO3 backend: precise Python exception subtypes.
// ---------------------------------------------------------------------------

#[cfg(feature = "bindings-pyo3")]
mod pyo3_errors {
    use pyo3::{
        exceptions::{PyException, PyRuntimeError},
        prelude::*,
        types::PyModuleMethods,
    };

    pyo3::create_exception!(
        xgrammar_rs,
        InvalidJSONError,
        PyException,
        "Raised when the JSON is invalid."
    );
    pyo3::create_exception!(
        xgrammar_rs,
        DeserializeVersionError,
        PyException,
        "Raised when the serialization version is invalid."
    );
    pyo3::create_exception!(
        xgrammar_rs,
        DeserializeFormatError,
        PyException,
        "Raised when the deserialization format is invalid."
    );
    pyo3::create_exception!(
        xgrammar_rs,
        InvalidStructuralTagError,
        PyException,
        "Raised when the structural tag is invalid."
    );

    /// Registers the custom exception types on the module.
    pub fn register_exceptions(m: &Bound<'_, PyModule>) -> PyResult<()> {
        m.add("InvalidJSONError", m.py().get_type::<InvalidJSONError>())?;
        m.add(
            "DeserializeVersionError",
            m.py().get_type::<DeserializeVersionError>(),
        )?;
        m.add(
            "DeserializeFormatError",
            m.py().get_type::<DeserializeFormatError>(),
        )?;
        m.add(
            "InvalidStructuralTagError",
            m.py().get_type::<InvalidStructuralTagError>(),
        )?;
        Ok(())
    }

    /// Maps an error message string into the appropriate Python exception.
    pub fn map_error_str(message: &str) -> PyErr {
        if message.starts_with("invalid JSON:") {
            return InvalidJSONError::new_err(message.to_owned());
        }
        if message.starts_with("version mismatch:") {
            return DeserializeVersionError::new_err(message.to_owned());
        }
        if message.starts_with("invalid format:") {
            return DeserializeFormatError::new_err(message.to_owned());
        }
        if message.contains("structural tag")
            || message.contains("StructuralTag")
        {
            return InvalidStructuralTagError::new_err(message.to_owned());
        }
        PyRuntimeError::new_err(message.to_owned())
    }
}

#[cfg(feature = "bindings-pyo3")]
pub use pyo3_errors::register_exceptions;

/// Maps any `Display` error into a [`BindingError`].
#[cfg(feature = "bindings-pyo3")]
pub fn map_error<E: std::fmt::Display>(error: E) -> BindingError {
    pyo3_errors::map_error_str(&error.to_string())
}

/// Maps any `Display` error into a [`BindingError`].
#[cfg(not(feature = "bindings-pyo3"))]
pub fn map_error<E: std::fmt::Display>(error: E) -> BindingError {
    XgrammarError::Invalid {
        message: error.to_string(),
    }
}

/// Maps [`xgrammar::grammar::DeserializeError`] to a [`BindingError`], preserving the
/// precise exception subtype under PyO3.
#[cfg(feature = "bindings-pyo3")]
pub fn map_deserialize_error(
    error: xgrammar::grammar::DeserializeError
) -> BindingError {
    use pyo3_errors::{
        DeserializeFormatError, DeserializeVersionError, InvalidJSONError,
    };
    match error {
        xgrammar::grammar::DeserializeError::InvalidJson(msg) => {
            InvalidJSONError::new_err(format!("invalid JSON: {msg}"))
        },
        xgrammar::grammar::DeserializeError::Version {
            expected,
            got,
        } => DeserializeVersionError::new_err(format!(
            "version mismatch: expected {expected}, got {got}"
        )),
        xgrammar::grammar::DeserializeError::Format(msg) => {
            DeserializeFormatError::new_err(format!("invalid format: {msg}"))
        },
    }
}

/// Maps [`xgrammar::grammar::DeserializeError`] to a [`BindingError`].
#[cfg(not(feature = "bindings-pyo3"))]
pub fn map_deserialize_error(
    error: xgrammar::grammar::DeserializeError
) -> BindingError {
    XgrammarError::Invalid {
        message: error.to_string(),
    }
}
