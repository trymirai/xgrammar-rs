//! Typed error categories for XGrammar's deserialization and structural-tag entry points.

use std::fmt;

/// Error returned when deserializing a serialized XGrammar object fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeserializeError {
    /// The serialized data was produced by an incompatible serialization version.
    VersionMismatch(String),
    /// The input was not valid JSON.
    InvalidJson(String),
    /// The JSON was well-formed but did not match the expected serialization format.
    Format(String),
    /// An unexpected error not covered by the categories above.
    Other(String),
}

impl DeserializeError {
    pub(crate) fn from_parts(
        kind: i32,
        message: String,
    ) -> Self {
        match kind {
            1 => Self::VersionMismatch(message),
            2 => Self::InvalidJson(message),
            3 => Self::Format(message),
            _ => Self::Other(message),
        }
    }

    /// The underlying error message.
    pub fn message(&self) -> &str {
        match self {
            Self::VersionMismatch(m)
            | Self::InvalidJson(m)
            | Self::Format(m)
            | Self::Other(m) => m,
        }
    }
}

impl fmt::Display for DeserializeError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for DeserializeError {}

/// Error returned when building a grammar from a structural tag fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralTagError {
    /// The structural tag was not valid JSON.
    InvalidJson(String),
    /// A JSON schema embedded in the structural tag was invalid.
    InvalidSchema(String),
    /// The structural tag was well-formed JSON but not a valid structural tag.
    Invalid(String),
    /// An unexpected error not covered by the categories above.
    Other(String),
}

impl StructuralTagError {
    pub(crate) fn from_parts(
        kind: i32,
        message: String,
    ) -> Self {
        match kind {
            2 => Self::InvalidJson(message),
            4 => Self::InvalidSchema(message),
            5 => Self::Invalid(message),
            _ => Self::Other(message),
        }
    }

    /// The underlying error message.
    pub fn message(&self) -> &str {
        match self {
            Self::InvalidJson(m)
            | Self::InvalidSchema(m)
            | Self::Invalid(m)
            | Self::Other(m) => m,
        }
    }
}

impl fmt::Display for StructuralTagError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for StructuralTagError {}
