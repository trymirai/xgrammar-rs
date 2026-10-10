//! A Lark rule or terminal definition.

use super::{lark_location::LarkLocation, lark_node::LarkNode};

/// A rule (`name: ...`) or terminal (`NAME: ...`) definition with its attributes.
#[derive(Debug, Clone)]
pub struct LarkDefinition {
    /// The name without a `!`/`?` prefix.
    pub name: String,
    /// Whether the name denotes a terminal (uppercase after an optional `_`).
    pub is_terminal: bool,
    /// The `lazy` attribute.
    pub lazy: bool,
    /// The `suffix=` marker.
    pub suffix: Option<LarkNode>,
    /// The `stop=` marker.
    pub stop: Option<LarkNode>,
    /// The `stop_capture=` name.
    pub stop_capture_name: Option<String>,
    /// The `temperature=` value.
    pub temperature: Option<f32>,
    /// The `max_tokens=` value and the location of its key.
    pub max_tokens: Option<(i32, LarkLocation)>,
    /// The `max_chars=` value.
    pub max_chars: Option<i32>,
    /// The capture name and the location of its value.
    pub capture_name: Option<(String, LarkLocation)>,
    /// The definition body.
    pub body: LarkNode,
    /// Where the name starts.
    pub location: LarkLocation,
}

impl LarkDefinition {
    /// Creates a definition without attributes.
    #[must_use]
    pub fn new(
        name: String,
        is_terminal: bool,
        body: LarkNode,
        location: LarkLocation,
    ) -> Self {
        Self {
            name,
            is_terminal,
            lazy: false,
            suffix: None,
            stop: None,
            stop_capture_name: None,
            temperature: None,
            max_tokens: None,
            max_chars: None,
            capture_name: None,
            body,
            location,
        }
    }

    /// Whether the rule is lazy or carries a suffix or stop marker.
    #[must_use]
    pub fn has_lazy_semantics(&self) -> bool {
        self.lazy || self.suffix.is_some() || self.stop.is_some()
    }
}
