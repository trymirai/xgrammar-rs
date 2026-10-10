//! The adaptive token-mask cache key of a parser state.

/// Equality/hash over `rule_id`, `sequence_id`, `element_id`, and `sub_element_id` only: the
/// token mask of a state does not depend on its start position, repeat count, partial
/// codepoint, budgets, or temperature (mirrors C++ `StateHashForCache`/`StateEqualForCache`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParserStateCacheKey {
    /// The rule's id.
    pub rule_id: i32,
    /// The rule body expression id.
    pub sequence_id: i32,
    /// The FSM node.
    pub element_id: i32,
    /// The sub element id.
    pub sub_element_id: i32,
}
