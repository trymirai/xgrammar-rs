//! Sparse per-rule metadata used to materialize suffix and stop captures.

/// Suffix/stop capture metadata attached to one rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuffixStopInfo {
    /// The rule carrying this metadata.
    pub rule_id: i32,
    /// Trailing bytes hidden only from this rule's own capture.
    pub hidden_suffix_bytes: i32,
    /// Trailing bytes hidden from this rule and every enclosing capture.
    pub hidden_stop_bytes: i32,
    /// Helper rule matching the body before a variable-length marker. A self-reference marks a
    /// zero-width event immediately following a fixed dynamic-dispatch marker.
    pub body_rule_id: i32,
    /// Helper rule matching a variable-length marker.
    pub marker_rule_id: i32,
    /// Capture name for the marker bytes.
    pub stop_capture_name: String,
}

impl Default for SuffixStopInfo {
    fn default() -> Self {
        Self {
            rule_id: -1,
            hidden_suffix_bytes: 0,
            hidden_stop_bytes: 0,
            body_rule_id: -1,
            marker_rule_id: -1,
            stop_capture_name: String::new(),
        }
    }
}

impl SuffixStopInfo {
    /// Whether the metadata carries no information (and is therefore not stored).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hidden_suffix_bytes == 0
            && self.hidden_stop_bytes == 0
            && self.body_rule_id == -1
            && self.marker_rule_id == -1
            && self.stop_capture_name.is_empty()
    }
}
