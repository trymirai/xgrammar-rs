//! A `%import` of a common Lark terminal.

use super::lark_location::LarkLocation;

/// One imported terminal: its dotted path and the local name it is bound to.
#[derive(Debug, Clone)]
pub struct LarkImport {
    /// The dotted import path, e.g. `common.INT`.
    pub path: String,
    /// The local terminal name.
    pub local_name: String,
    /// Where the `%import` directive starts.
    pub location: LarkLocation,
}
