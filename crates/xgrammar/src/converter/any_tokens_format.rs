//! The `any_tokens` structural-tag format.

use super::structural_tag_format::IntOrString;

/// `{"type":"any_tokens","exclude_tokens":[..],"max_tokens":..}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct AnyTokensFormat {
    pub exclude_tokens: Vec<IntOrString>,
    /// Token budget of the token run.
    pub max_tokens: Option<i32>,
    pub resolved_exclude_token_ids: Vec<i32>,
    pub detected_end_token_ids: Vec<i32>,
}
