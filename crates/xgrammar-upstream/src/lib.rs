//! The upstream xgrammar release this port tracks, downloaded by this crate's build script into the
//! gitignored `external/xgrammar`. Only tests and development tools depend on it, never a
//! published crate.

/// Absolute path of the upstream checkout.
pub const DIR: &str = env!("XGRAMMAR_UPSTREAM_DIR");
