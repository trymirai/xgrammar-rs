//! The `json_schema` structural-tag format.

/// `{"type":"json_schema","json_schema":..,"style":..}` (the schema is stored serialized).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JsonSchemaFormat {
    pub json_schema: String,
    /// One of the [`super::json_format::JsonFormat`] names.
    pub style: String,
    pub any_order: bool,
    /// Per-tag cap on consecutive whitespace characters.
    pub max_whitespace_cnt: Option<i32>,
    /// Substrings forbidden within string values and property names (sorted, deduplicated).
    pub excludes: Vec<String>,
}
