//! One history row of the Earley parser.

use super::{capture_event::CaptureEvent, parser_state::ParserState};

/// The parser state recorded at one input position: the scanable states, the completable
/// parent pairs, whether the root rule is completed, and the capture events.
#[derive(Debug, Clone, PartialEq)]
pub struct ParserRow {
    /// The scanable states.
    pub states: Vec<ParserState>,
    /// The `(referenced_rule_id, parent_state)` pairs awaiting completion.
    pub completable_states: Vec<(i32, ParserState)>,
    /// Whether the root rule is completed.
    pub completed: bool,
    /// The capture events recorded at this position.
    pub capture_events: Vec<CaptureEvent>,
}
