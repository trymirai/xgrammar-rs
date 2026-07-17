//! Global configuration for XGrammar.

/// The serialization format version stamped into every serialized object's `__VERSION__`.
pub const SERIALIZATION_VERSION: &str = "v14";

/// Returns the serialization version for the grammar.
///
/// This is used to check the compatibility of the serialized grammar.
#[must_use]
pub fn get_serialization_version() -> &'static str {
    SERIALIZATION_VERSION
}
