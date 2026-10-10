//! Tracks which parser states have already been enqueued this round.
//!
//! Like the C++, small rounds use a linear scan over a vector and switch to a hash set once
//! more than [`TRANSITION_THRESHOLD`] states were inserted.

use ahash::AHashSet;

use super::parser_state::ParserState;

/// The number of states scanned linearly before switching to the hash set.
const TRANSITION_THRESHOLD: usize = 50;

/// A set of visited parser states used to deduplicate the processing queue.
#[derive(Debug, Default, Clone)]
pub struct RepeatDetector {
    visited_vector: Vec<ParserState>,
    visited_set: AHashSet<ParserState>,
}

impl RepeatDetector {
    /// Creates an empty detector.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether `state` has already been inserted.
    #[must_use]
    pub fn is_visited(
        &self,
        state: &ParserState,
    ) -> bool {
        if self.visited_vector.len() > TRANSITION_THRESHOLD {
            self.visited_set.contains(state)
        } else {
            // Compare whole states at once: states queued together usually share their leading
            // fields, so a field-by-field comparison branches deep and mispredicts.
            let key = state.serialize_member_array();
            self.visited_vector.iter().any(|visited| {
                let fields = visited.serialize_member_array();
                fields.iter().zip(&key).fold(0, |differences, (a, b)| differences | (a ^ b)) == 0
            })
        }
    }

    /// Records `state` as visited.
    pub fn insert(
        &mut self,
        state: ParserState,
    ) {
        if self.visited_vector.len() == TRANSITION_THRESHOLD {
            self.visited_set.extend(self.visited_vector.iter().copied());
        }
        if self.visited_vector.len() >= TRANSITION_THRESHOLD {
            self.visited_set.insert(state);
        }
        self.visited_vector.push(state);
    }

    /// Clears all recorded states.
    pub fn clear(&mut self) {
        if self.visited_vector.len() > TRANSITION_THRESHOLD {
            self.visited_set.clear();
        }
        self.visited_vector.clear();
    }
}
