//! A parsed Lark expression.

use std::rc::Rc;

use super::{lark_document::LarkDocument, lark_location::LarkLocation, lark_node_kind::LarkNodeKind};

/// A Lark expression tree node. Which fields are meaningful depends on [`LarkNodeKind`]:
/// `text` holds the literal, pattern, name, JSON value, or token text; `text2` the end of a
/// character range; `flags` string or regex flags; `min_repeat`/`max_repeat` the repetition
/// bounds (`-1` is unbounded); `nested` an inline `%lark` document.
#[derive(Debug, Clone)]
pub struct LarkNode {
    /// The node kind.
    pub kind: LarkNodeKind,
    /// Where the node starts.
    pub location: LarkLocation,
    /// The primary text payload.
    pub text: String,
    /// The end of a character range.
    pub text2: String,
    /// String or regex flags.
    pub flags: String,
    /// Minimum repetition count.
    pub min_repeat: i32,
    /// Maximum repetition count (`-1` is unbounded).
    pub max_repeat: i32,
    /// Child expressions.
    pub children: Vec<LarkNode>,
    /// The document of an inline `%lark { ... }` grammar.
    pub nested: Option<Rc<LarkDocument>>,
}

impl LarkNode {
    /// Creates a node of `kind` with empty payloads.
    #[must_use]
    pub fn new(
        kind: LarkNodeKind,
        location: LarkLocation,
    ) -> Self {
        Self {
            kind,
            location,
            text: String::new(),
            text2: String::new(),
            flags: String::new(),
            min_repeat: 0,
            max_repeat: 0,
            children: Vec::new(),
            nested: None,
        }
    }

    /// Creates a node of `kind` whose primary payload is `text`.
    #[must_use]
    pub fn with_text(
        kind: LarkNodeKind,
        location: LarkLocation,
        text: impl Into<String>,
    ) -> Self {
        Self {
            text: text.into(),
            ..Self::new(kind, location)
        }
    }

    /// Creates a node of `kind` over `children`.
    #[must_use]
    pub fn with_children(
        kind: LarkNodeKind,
        location: LarkLocation,
        children: Vec<LarkNode>,
    ) -> Self {
        Self {
            children,
            ..Self::new(kind, location)
        }
    }

    /// Creates a repetition of `child` between `min_repeat` and `max_repeat` (`-1` unbounded).
    #[must_use]
    pub fn repeat(
        location: LarkLocation,
        child: LarkNode,
        min_repeat: i32,
        max_repeat: i32,
    ) -> Self {
        Self {
            min_repeat,
            max_repeat,
            ..Self::with_children(LarkNodeKind::Repeat, location, vec![child])
        }
    }
}
