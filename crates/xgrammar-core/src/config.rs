//! Global configuration constants — a port of `cpp/config.cc` / `include/xgrammar/config.h`.

/// The serialization format version stamped into every serialized object's `__VERSION__`.
pub const SERIALIZATION_VERSION: &str = "v11";

/// Returns the serialization format version (the C++ `GetSerializationVersion`).
#[must_use]
pub fn get_serialization_version() -> &'static str {
    SERIALIZATION_VERSION
}
