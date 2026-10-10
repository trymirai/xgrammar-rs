//! A completion event of a captured rule.

use super::capture_occurrence::CaptureOccurrence;

/// Recorded when a captured (or capture-hiding) rule is completed during parsing. The matched
/// span is `[start_pos, r)` in input positions, where `r` is the history row at which the event
/// is recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureEvent {
    /// The id of the completed rule.
    pub rule_id: i32,
    /// The position where the rule started matching. `NO_PREV_INPUT_POS` means position 0 (the
    /// rule acts as the root).
    pub start_pos: i32,
    /// The unadjusted start position of this rule occurrence. Differs from `start_pos` for the
    /// zero-width event inserted after a dynamic-dispatch marker.
    pub occurrence_start_pos: i32,
    /// Number of trailing bytes hidden only from this rule's own capture.
    pub hidden_suffix_bytes: i32,
    /// Number of trailing bytes hidden from every containing capture.
    pub hidden_stop_bytes: i32,
    /// The captured rule occurrences whose concrete Earley parent chains contain this stop
    /// completion. Includes this rule's occurrence when the rule itself is captured.
    pub stop_capture_targets: Vec<CaptureOccurrence>,
}
