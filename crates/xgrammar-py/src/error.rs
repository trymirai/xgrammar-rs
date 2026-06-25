//! Python exception types matching upstream `xgrammar.exception`.

use pyo3::{
    exceptions::{PyException, PyRuntimeError},
    prelude::*,
    types::PyModuleMethods,
};

/// Registers the custom exception types on the module.
#[cfg(feature = "bindings-pyo3")]
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

/// Maps a core/display error into the appropriate Python exception.
pub fn map_error<E: std::fmt::Display>(error: E) -> PyErr {
    map_error_str(&error.to_string())
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
    if message.contains("structural tag") || message.contains("StructuralTag") {
        return InvalidStructuralTagError::new_err(message.to_owned());
    }
    PyRuntimeError::new_err(message.to_owned())
}

/// Maps [`xgrammar::grammar::DeserializeError`] precisely.
pub fn map_deserialize_error(
    error: xgrammar::grammar::DeserializeError
) -> PyErr {
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
