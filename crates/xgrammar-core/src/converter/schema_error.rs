//! Error raised by the JSON-schema converter — a port of `SchemaError` /
//! `SchemaErrorType` in `cpp/json_schema_converter.*`.

/// The category of a [`SchemaError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaErrorKind {
    /// The schema is structurally invalid (wrong types, bad keywords).
    InvalidSchema,
    /// The schema is well-formed but cannot match any value.
    UnsatisfiableSchema,
}

/// A JSON-schema conversion failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct SchemaError {
    /// The error category.
    pub kind: SchemaErrorKind,
    /// Human-readable description.
    pub message: String,
}

impl SchemaError {
    /// Builds an [`SchemaErrorKind::InvalidSchema`] error.
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self {
            kind: SchemaErrorKind::InvalidSchema,
            message: message.into(),
        }
    }

    /// Builds an [`SchemaErrorKind::UnsatisfiableSchema`] error.
    pub(crate) fn unsatisfiable(message: impl Into<String>) -> Self {
        Self {
            kind: SchemaErrorKind::UnsatisfiableSchema,
            message: message.into(),
        }
    }
}
