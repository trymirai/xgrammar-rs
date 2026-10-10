//! A concrete occurrence of a captured rule in an Earley parent chain.

/// A rule occurrence identified by its rule id and start position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CaptureOccurrence {
    /// The id of the captured rule.
    pub rule_id: i32,
    /// The position where the rule occurrence started.
    pub start_pos: i32,
}
