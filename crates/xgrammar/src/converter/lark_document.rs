//! A parsed Lark grammar.

use serde_json::Value;

use super::{
    lark_definition::LarkDefinition, lark_import::LarkImport, lark_location::LarkLocation, lark_node::LarkNode,
};

/// The items of a Lark grammar (or an inline `%lark { ... }` block).
#[derive(Debug, Clone, Default)]
pub struct LarkDocument {
    /// Rule and terminal definitions, in source order.
    pub definitions: Vec<LarkDefinition>,
    /// `%ignore` expressions.
    pub ignores: Vec<LarkNode>,
    /// `%import` directives.
    pub imports: Vec<LarkImport>,
    /// `%grammar_options` values with their locations.
    pub options: Vec<(Value, LarkLocation)>,
}
