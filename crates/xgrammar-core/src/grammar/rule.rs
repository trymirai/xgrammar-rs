//! A named grammar rule — a port of `Grammar::Impl::Rule` in `cpp/grammar_impl.h`.

use serde::{Deserialize, Serialize};

/// Sentinel id meaning "no expression / no lookahead assertion".
pub const NO_EXPR: i32 = -1;

/// A production rule: a name plus the id of the grammar expression that forms its body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// The rule name.
    pub name: String,
    /// The grammar-expression id of the rule body, or [`NO_EXPR`] if not yet set.
    pub body_expr_id: i32,
    /// The id of the associated lookahead-assertion expression (a sequence expr), or
    /// [`NO_EXPR`] if there is none.
    #[serde(default = "no_expr")]
    pub lookahead_assertion_id: i32,
    /// Whether the lookahead assertion is exact.
    #[serde(default)]
    pub is_exact_lookahead: bool,
}

fn no_expr() -> i32 {
    NO_EXPR
}

impl Rule {
    /// Creates a rule with the given name and body expression, no lookahead assertion.
    #[must_use]
    pub fn new(name: impl Into<String>, body_expr_id: i32) -> Self {
        Self {
            name: name.into(),
            body_expr_id,
            lookahead_assertion_id: NO_EXPR,
            is_exact_lookahead: false,
        }
    }

    /// Creates a rule with no body yet (body set later via the builder).
    #[must_use]
    pub fn empty(name: impl Into<String>) -> Self {
        Self::new(name, NO_EXPR)
    }
}
