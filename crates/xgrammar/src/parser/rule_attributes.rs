//! The attribute block of a rule definition, e.g. `name[max_tokens=10, capture] ::= ...`.

use crate::grammar::SuffixStopInfo;

/// Rule attributes attached to a rule-definition identifier by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleAttributes {
    /// The token budget, or `-1`.
    pub max_tokens: i32,
    /// The character budget, or `-1`.
    pub max_chars: i32,
    /// The capture name, or empty.
    pub capture_name: String,
    /// The capture-hidden suffix/stop metadata (the `rule_id` field is unused).
    pub suffix_stop_info: SuffixStopInfo,
    /// Whether the rule is lazy (committed-shortest).
    pub is_lazy: bool,
    /// The sampling temperature.
    pub temperature: Option<f32>,
}
