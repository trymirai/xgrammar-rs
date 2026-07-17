//! The state of the Earley parser.
//!
//! A live state walks a rule's compiled FSM: `element_id` is the current FSM node,
//! `rule_start_pos` the input position the rule was predicted at. `Eq`/`Hash` cover all
//! fields (equality/hash for parsing), which is what the
//! parser's queue de-duplication needs.

use std::fmt;

/// One Earley item: a position within a rule's FSM plus the input position it began at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParserState {
    /// The rule's id.
    pub rule_id: i32,
    /// Which choice/body of the rule is selected (its grammar-expr id).
    pub sequence_id: i32,
    /// The current FSM node (or sequence element).
    pub element_id: i32,
    /// The input position from which this rule started (`NO_PREV_INPUT_POS` for the root).
    pub rule_start_pos: i32,
    /// The sub-element index within the current element (UTF-8/byte-string progress).
    pub sub_element_id: i32,
    /// How many times the current repeat element has matched (`kRepeat`).
    pub repeat_count: i32,
    /// Partial codepoint accumulated during UTF-8 decoding (IR character-class path).
    pub partial_codepoint: i32,
}

impl ParserState {
    /// A `sequence_id` of this value marks a rule that has not yet been expanded.
    pub const UNEXPANDED_RULE_START_SEQUENCE_ID: i32 = 128_000;
    /// A `rule_start_pos` of this value marks the root of the parsing stack.
    pub const NO_PREV_INPUT_POS: i32 = -1;

    /// Creates a state (with `repeat_count` and `partial_codepoint` defaulted to 0).
    #[must_use]
    pub fn new(
        rule_id: i32,
        sequence_id: i32,
        element_id: i32,
        rule_start_pos: i32,
        sub_element_id: i32,
    ) -> Self {
        Self {
            rule_id,
            sequence_id,
            element_id,
            rule_start_pos,
            sub_element_id,
            repeat_count: 0,
            partial_codepoint: 0,
        }
    }

    /// Creates a state with an explicit `repeat_count`.
    #[must_use]
    pub fn with_repeat(
        rule_id: i32,
        sequence_id: i32,
        element_id: i32,
        rule_start_pos: i32,
        sub_element_id: i32,
        repeat_count: i32,
    ) -> Self {
        Self {
            rule_id,
            sequence_id,
            element_id,
            rule_start_pos,
            sub_element_id,
            repeat_count,
            partial_codepoint: 0,
        }
    }

    /// The invalid state (`sequence_id == -1`).
    #[must_use]
    pub fn invalid() -> Self {
        Self {
            rule_id: -1,
            sequence_id: -1,
            element_id: -1,
            rule_start_pos: -1,
            sub_element_id: -1,
            repeat_count: 0,
            partial_codepoint: 0,
        }
    }

    /// Whether the state is invalid.
    #[must_use]
    pub fn is_invalid(&self) -> bool {
        self.sequence_id == -1
    }

    /// The four FSM fields used as an adaptive token-mask cache key (mirrors C++
    /// `StateHashForCache`).
    #[must_use]
    pub fn cache_key(&self) -> ParserStateCacheKey {
        ParserStateCacheKey {
            rule_id: self.rule_id,
            sequence_id: self.sequence_id,
            element_id: self.element_id,
            sub_element_id: self.sub_element_id,
        }
    }

    /// Serializes the state as a member array (mirrors C++ `XGRAMMAR_MEMBER_ARRAY`).
    #[must_use]
    pub fn serialize_member_array(&self) -> [i32; 7] {
        [
            self.rule_id,
            self.sequence_id,
            self.element_id,
            self.rule_start_pos,
            self.sub_element_id,
            self.repeat_count,
            self.partial_codepoint,
        ]
    }

    /// Deserializes a member array into a parser state.
    ///
    /// # Panics
    /// Panics if `values` does not contain exactly seven integers.
    #[must_use]
    pub fn from_member_array(values: &[i32]) -> Self {
        assert_eq!(values.len(), 7, "ParserState member array must have 7 fields");
        Self {
            rule_id: values[0],
            sequence_id: values[1],
            element_id: values[2],
            rule_start_pos: values[3],
            sub_element_id: values[4],
            repeat_count: values[5],
            partial_codepoint: values[6],
        }
    }
}

/// Adaptive token-mask cache key — equality/hash over `rule_id`, `sequence_id`,
/// `element_id`, and `sub_element_id` only (mirrors C++ `StateHashForCache`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParserStateCacheKey {
    pub rule_id: i32,
    pub sequence_id: i32,
    pub element_id: i32,
    pub sub_element_id: i32,
}

impl fmt::Display for ParserState {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(
            f,
            "ParserState(rule_id={}, sequence_id={}, element_id={}, rule_start_pos={}, sub_element_id={}",
            self.rule_id, self.sequence_id, self.element_id, self.rule_start_pos, self.sub_element_id
        )?;
        if self.repeat_count != 0 {
            write!(f, ", repeat_count={}", self.repeat_count)?;
        }
        if self.partial_codepoint != 0 {
            write!(f, ", partial_codepoint={}", self.partial_codepoint)?;
        }
        f.write_str(")")
    }
}
