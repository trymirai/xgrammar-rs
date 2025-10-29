/// A structural tag item. See [`Grammar::from_structural_tag`] for more details.
///
/// The structural tag handles the dispatching of different grammars based on the
/// tags and triggers: it initially allows any output, until a trigger is encountered,
/// then dispatch to the corresponding tag; when the end tag is encountered, the grammar
/// will allow any following output, until the next trigger is encountered.
///
/// Fields
/// - `begin`: The begin tag.
/// - `schema`: The schema (JSON schema as a string).
/// - `end`: The end tag.
#[derive(Debug, Clone)]
pub struct StructuralTagItem {
    pub begin: String,
    pub schema: String,
    pub end: String,
}

impl StructuralTagItem {
    pub fn new(
        begin: impl Into<String>,
        schema: impl Into<String>,
        end: impl Into<String>,
    ) -> Self {
        Self {
            begin: begin.into(),
            schema: schema.into(),
            end: end.into(),
        }
    }
}
