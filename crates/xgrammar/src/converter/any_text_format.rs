//! The `any_text` structural-tag format.

/// `{"type":"any_text","excludes":[..],"max_tokens":..,"max_chars":..}`; `detected_end_strs` is
/// filled by the analyzer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct AnyTextFormat {
    pub excludes: Vec<String>,
    /// Token budget of the text.
    pub max_tokens: Option<i32>,
    /// Character budget of the text.
    pub max_chars: Option<i32>,
    pub detected_end_strs: Vec<String>,
}
