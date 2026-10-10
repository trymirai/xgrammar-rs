//! The state of the Earley parser.
//!
//! A live state walks a rule's compiled FSM: `element_id` is the current FSM node,
//! `rule_start_pos` the input position the rule was predicted at. `Eq`/`Hash` cover all
//! fields (equality/hash for parsing), which is what the parser's queue de-duplication needs.
//! The derived `Ord` is lexicographic over the fields in declaration order; it is only used to
//! sort states for deterministic serialization.

use std::fmt;

use super::parser_state_cache_key::ParserStateCacheKey;

/// One Earley item: a position within a rule's FSM plus the input position it began at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ParserState {
    /// The rule's id (`-1` for an IR-path lookahead state).
    pub rule_id: i32,
    /// The rule body (or lookahead sequence) expression id.
    pub sequence_id: i32,
    /// The current FSM node (or element index in an IR sequence).
    pub element_id: i32,
    /// The input position the rule started at, or [`Self::NO_PREV_INPUT_POS`] for the root.
    pub rule_start_pos: i32,
    /// The last token index this state's derivation may consume, from the token budget
    /// (`Rule::max_tokens`) of the rule it is inside; `-1` means unlimited.
    pub budget_deadline: i32,
    /// The id of the sub element in the current element of the sequence.
    pub sub_element_id: i32,
    /// The number of times the element is repeated (for repeat edges).
    pub repeat_count: i32,
    /// Partial codepoint accumulated during UTF-8 decoding for character classes.
    pub partial_codepoint: i32,
    /// The innermost active rule that specifies a sampling temperature, or `-1`.
    pub active_temperature_rule_id: i32,
    /// The absolute input codepoint position at which the active character budget expires;
    /// `-1` means unlimited.
    pub char_budget_deadline: i32,
}

impl ParserState {
    /// A `rule_start_pos` meaning the state is the root of the parsing stack.
    pub const NO_PREV_INPUT_POS: i32 = -1;

    /// Creates a state at `element_id` of `sequence_id` with no budgets, sub element, repeat
    /// count, partial codepoint, or temperature rule.
    #[must_use]
    pub fn new(
        rule_id: i32,
        sequence_id: i32,
        element_id: i32,
        rule_start_pos: i32,
    ) -> Self {
        Self {
            rule_id,
            sequence_id,
            element_id,
            rule_start_pos,
            budget_deadline: -1,
            sub_element_id: 0,
            repeat_count: 0,
            partial_codepoint: 0,
            active_temperature_rule_id: -1,
            char_budget_deadline: -1,
        }
    }

    /// The same occurrence moved to `element_id`, with the sub element, repeat count, and
    /// partial codepoint reset (budgets and the temperature rule are inherited).
    #[must_use]
    pub fn at_element(
        &self,
        element_id: i32,
    ) -> Self {
        self.at_element_with_repeat(element_id, 0)
    }

    /// Like [`Self::at_element`], keeping `repeat_count`.
    #[must_use]
    pub fn at_element_with_repeat(
        &self,
        element_id: i32,
        repeat_count: i32,
    ) -> Self {
        Self {
            element_id,
            sub_element_id: 0,
            repeat_count,
            partial_codepoint: 0,
            ..*self
        }
    }

    /// The adaptive token-mask cache key of this state.
    #[must_use]
    pub fn cache_key(&self) -> ParserStateCacheKey {
        ParserStateCacheKey {
            rule_id: self.rule_id,
            sequence_id: self.sequence_id,
            element_id: self.element_id,
            sub_element_id: self.sub_element_id,
        }
    }

    /// Serializes as the C++ `XGRAMMAR_MEMBER_ARRAY` order.
    #[inline]
    #[must_use]
    pub fn serialize_member_array(&self) -> [i32; 10] {
        [
            self.rule_id,
            self.sequence_id,
            self.element_id,
            self.rule_start_pos,
            self.budget_deadline,
            self.sub_element_id,
            self.repeat_count,
            self.partial_codepoint,
            self.active_temperature_rule_id,
            self.char_budget_deadline,
        ]
    }

    /// Deserializes a member array produced by [`Self::serialize_member_array`].
    ///
    /// Returns `None` unless `values` contains exactly ten integers.
    #[must_use]
    pub fn from_member_array(values: &[i32]) -> Option<Self> {
        let [
            rule_id,
            sequence_id,
            element_id,
            rule_start_pos,
            budget_deadline,
            sub_element_id,
            repeat_count,
            partial_codepoint,
            active_temperature_rule_id,
            char_budget_deadline,
        ] = *values
        else {
            return None;
        };
        Some(Self {
            rule_id,
            sequence_id,
            element_id,
            rule_start_pos,
            budget_deadline,
            sub_element_id,
            repeat_count,
            partial_codepoint,
            active_temperature_rule_id,
            char_budget_deadline,
        })
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
            self.rule_id, self.sequence_id, self.element_id, self.rule_start_pos, self.sub_element_id
        )?;
        if self.repeat_count != 0 {
            write!(f, ", repeat_count={}", self.repeat_count)?;
        }
        if self.partial_codepoint != 0 {
            write!(f, ", partial_codepoint={}", self.partial_codepoint)?;
        }
        if self.budget_deadline != -1 {
            write!(f, ", budget_deadline={}", self.budget_deadline)?;
        }
        if self.active_temperature_rule_id != -1 {
            write!(f, ", active_temperature_rule_id={}", self.active_temperature_rule_id)?;
        }
        if self.char_budget_deadline != -1 {
            write!(f, ", char_budget_deadline={}", self.char_budget_deadline)?;
        }
        f.write_str(")")
    }
}
