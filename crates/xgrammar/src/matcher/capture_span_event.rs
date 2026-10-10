//! A capture event flattened out of the parser history, with its marker resolved to bytes.

use crate::parser::CaptureOccurrence;

/// A capture event positioned by history rows, used while materializing captures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSpanEvent {
    /// The id of the completed rule.
    pub rule_id: i32,
    /// The history row where the captured span starts.
    pub start_row: i32,
    /// The unadjusted start position of the rule occurrence.
    pub occurrence_start_pos: i32,
    /// The history row where the event was recorded.
    pub end_row: i32,
    /// Trailing bytes hidden only from this rule's own capture.
    pub hidden_suffix_bytes: i32,
    /// Trailing bytes hidden from every containing capture.
    pub hidden_stop_bytes: i32,
    /// The captured occurrences whose parent chains contain this stop completion.
    pub stop_capture_targets: Vec<CaptureOccurrence>,
    /// Whether the suffix/stop marker was located in the accepted bytes.
    pub marker_present: bool,
    /// The first byte of the marker, or `-1`.
    pub marker_start_byte: i64,
}
