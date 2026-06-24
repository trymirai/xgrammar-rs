//! One inclusive range of a character class — a port of
//! `GrammarBuilder::CharacterClassElement`.

use serde::{Deserialize, Serialize};

/// An inclusive `[lower, upper]` codepoint range within a character class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CharacterClassElement {
    /// Inclusive lower bound (codepoint).
    pub lower: i32,
    /// Inclusive upper bound (codepoint).
    pub upper: i32,
}

impl CharacterClassElement {
    /// Creates a range from `lower` to `upper`, inclusive.
    #[must_use]
    pub const fn new(lower: i32, upper: i32) -> Self {
        Self { lower, upper }
    }

    /// Whether `codepoint` falls within this range.
    #[must_use]
    pub const fn contains(self, codepoint: i32) -> bool {
        self.lower <= codepoint && codepoint <= self.upper
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_bounds_inclusive() {
        let e = CharacterClassElement::new(b'a' as i32, b'z' as i32);
        assert!(e.contains(b'a' as i32));
        assert!(e.contains(b'z' as i32));
        assert!(e.contains(b'm' as i32));
        assert!(!e.contains(b'A' as i32));
    }
}
