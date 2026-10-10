//! A named grammar rule.

/// Sentinel id meaning "no expression / no lookahead assertion".
pub const NO_EXPR: i32 = -1;

/// A production rule: a name plus the id of the grammar expression that forms its body.
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    /// The rule name.
    pub name: String,
    /// The grammar-expression id of the rule body, or [`NO_EXPR`] if not yet set.
    pub body_expr_id: i32,
    /// The id of the associated lookahead-assertion expression (a sequence expr), or
    /// [`NO_EXPR`] if there is none.
    pub lookahead_assertion_id: i32,
    /// Whether the lookahead assertion is exact.
    pub is_exact_lookahead: bool,
    /// The token budget of the rule. When non-negative, the matcher bounds each occurrence of
    /// this rule to at most `max_tokens` LLM tokens. `-1` means no budget.
    pub max_tokens: i32,
    /// The Unicode codepoint budget of the rule. When non-negative, the matcher bounds each
    /// occurrence of this rule to at most `max_chars` codepoints. `-1` means no budget.
    pub max_chars: i32,
    /// The capture group name of the rule. When non-empty, the matcher records the input span
    /// matched by this rule on every completion. Empty means no capture.
    pub capture_name: String,
    /// Whether the rule matches with committed-shortest (lazy) semantics.
    pub is_lazy: bool,
    /// The sampling temperature to use while matching this rule.
    pub temperature: Option<f32>,
}

impl Rule {
    /// Creates a rule with the given name and body expression, no lookahead assertion.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        body_expr_id: i32,
    ) -> Self {
        Self {
            name: name.into(),
            body_expr_id,
            lookahead_assertion_id: NO_EXPR,
            is_exact_lookahead: false,
            max_tokens: -1,
            max_chars: -1,
            capture_name: String::new(),
            is_lazy: false,
            temperature: None,
        }
    }

    /// Creates a rule with no body yet (body set later via the builder).
    #[must_use]
    pub fn empty(name: impl Into<String>) -> Self {
        Self::new(name, NO_EXPR)
    }
}
