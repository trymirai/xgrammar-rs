//! One entry of the regex parsing stack.

use super::regex_node::RegexNode;

/// An entry of the regex parsing stack: a parsed atom with its source span, or a marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegexStackEntry {
    /// A parsed node spanning `regex[span_begin..span_end]`.
    Node {
        /// The parsed node.
        node: RegexNode,
        /// The byte offset where the atom starts.
        span_begin: usize,
        /// The byte offset right after the atom.
        span_end: usize,
    },
    /// An opening `(` at the given byte offset.
    GroupOpen(usize),
    /// An alternation `|`.
    Alternation,
}
