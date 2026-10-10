/// An error from [`super::Grammar::deserialize_json`] (and other deserializers).
/// `SerializationError` family.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DeserializeError {
    /// The input was not valid JSON.
    #[error("invalid JSON: {0}")]
    InvalidJson(String),
    /// The serialized `__VERSION__` did not match the current one.
    #[error("version mismatch: expected {expected}, got {got}")]
    Version {
        /// The version this build expects.
        expected: String,
        /// The version found in the input.
        got: String,
    },
    /// The JSON was structurally valid but missing/ill-typed for the target type.
    #[error("invalid format: {0}")]
    Format(String),
}
