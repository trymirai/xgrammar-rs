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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults_lookahead_to_none() {
        let rule = Rule::new("root", 3);
        assert_eq!(rule.name, "root");
        assert_eq!(rule.body_expr_id, 3);
        assert_eq!(rule.lookahead_assertion_id, NO_EXPR);
        assert!(!rule.is_exact_lookahead);
    }

    #[test]
    fn empty_has_no_body() {
        assert_eq!(Rule::empty("r").body_expr_id, NO_EXPR);
    }

    #[test]
    fn serde_roundtrip() {
        let rule = Rule {
            name: "x".into(),
            body_expr_id: 1,
            lookahead_assertion_id: 4,
            is_exact_lookahead: true,
        };
        let json = serde_json::to_string(&rule).unwrap();
        assert_eq!(serde_json::from_str::<Rule>(&json).unwrap(), rule);
    }
}
