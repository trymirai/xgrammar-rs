//! The state of the Earley parser — a port of `ParserState` in `cpp/earley_parser.h`.
//!
//! A live state walks a rule's compiled FSM: `element_id` is the current FSM node,
//! `rule_start_pos` the input position the rule was predicted at. `Eq`/`Hash` cover all
//! fields (the C++ `StateEqualForParsing` / `StateHashForParsing`), which is what the
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
}

impl fmt::Display for ParserState {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(
            f,
            "ParserState(rule_id={}, sequence_id={}, element_id={}, rule_start_pos={}, sub_element_id={}",
            self.rule_id,
            self.sequence_id,
            self.element_id,
            self.rule_start_pos,
            self.sub_element_id
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
