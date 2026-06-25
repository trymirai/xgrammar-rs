//! The unified binding error type.

/// Errors raised by the xgrammar bindings (maps every core error family to one exported type).
#[bindings::export(Error)]
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum XgrammarError {
    /// A grammar, schema, regex, structural-tag, or serialization input was invalid.
    #[error("{message}")]
    Invalid {
        /// The underlying error message.
        message: String,
    },
}

impl XgrammarError {
    /// Wraps any `Display` error (the core error families) into [`XgrammarError::Invalid`].
    pub(crate) fn from_display<E: std::fmt::Display>(error: E) -> Self {
        Self::Invalid {
            message: error.to_string(),
        }
    }
}
