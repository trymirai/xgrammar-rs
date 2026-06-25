//! Error raised by the structural-tag converter — a port of the `StructuralTagError`
//! variant family in `cpp/include/xgrammar/exception.h`.

/// A structural-tag conversion failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StructuralTagError {
    /// The input was not valid JSON.
    #[error("Invalid JSON error: {0}")]
    InvalidJson(String),
    /// An embedded JSON schema was invalid.
    #[error("Invalid JSON schema error: {0}")]
    InvalidJsonSchema(String),
    /// The structural tag was well-formed JSON but semantically invalid.
    #[error("Invalid structural tag error: {0}")]
    InvalidStructuralTag(String),
}

impl StructuralTagError {
    /// Builds an [`StructuralTagError::InvalidStructuralTag`] error.
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidStructuralTag(message.into())
    }
}
