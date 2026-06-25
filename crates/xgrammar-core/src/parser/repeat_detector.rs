//! Tracks which parser states have already been enqueued this round — a port of
//! `RepeatDetector` in `cpp/earley_parser.h`.
//!
//! The C++ switches from a linear scan to a hash set past a size threshold; that is a pure
//! performance optimization, so this uses a [`HashSet`] throughout (equivalent semantics
//! under the full-field `Eq`/`Hash` of [`ParserState`]).

use std::collections::HashSet;

use super::parser_state::ParserState;

/// A set of visited parser states used to deduplicate the processing queue.
#[derive(Debug, Default)]
pub struct RepeatDetector {
    visited: HashSet<ParserState>,
}

impl RepeatDetector {
    /// Creates an empty detector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            visited: HashSet::new(),
        }
    }

    /// Whether `state` has already been inserted.
    #[must_use]
    pub fn is_visited(
        &self,
        state: &ParserState,
    ) -> bool {
        self.visited.contains(state)
    }

    /// Records `state` as visited.
    pub fn insert(
        &mut self,
        state: ParserState,
    ) {
        self.visited.insert(state);
    }

    /// Clears all recorded states.
    pub fn clear(&mut self) {
        self.visited.clear();
    }
}
