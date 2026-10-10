//! The legacy structural-tag item.

/// Legacy structural-tag item (`begin` / `schema` / `end`).
///
/// Prefer the JSON structural-tag document + [`Grammar::from_structural_tag`](crate::grammar::Grammar::from_structural_tag); this exists
/// for API parity with `xgrammar::StructuralTagItem`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralTagItem {
    /// Opening tag text.
    pub begin: String,
    /// JSON schema (as a string) between the tags.
    pub schema: String,
    /// Closing tag text.
    pub end: String,
}
