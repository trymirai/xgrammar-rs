//! A grammar that a Lark source can reference as `@name`.

use crate::grammar::Grammar;

/// A grammar or Lark source that can be referenced by name from another Lark grammar.
#[derive(Debug, Clone)]
pub enum NamedGrammar {
    /// An existing grammar.
    Grammar {
        /// The reference name, without the leading `@`.
        name: String,
        /// The referenced grammar.
        grammar: Box<Grammar>,
    },
    /// A Lark source with its own `start` rule.
    Lark {
        /// The reference name, without the leading `@`.
        name: String,
        /// The Lark source.
        source: String,
    },
}

impl NamedGrammar {
    /// The reference name, without the leading `@`.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Grammar {
                name,
                ..
            }
            | Self::Lark {
                name,
                ..
            } => name,
        }
    }
}
