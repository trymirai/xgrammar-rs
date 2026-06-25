//! `Grammar` binding — a thin opaque wrapper over [`xgrammar::grammar::Grammar`].

/// Errors raised by the grammar bindings.
#[bindings::export(Error)]
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum XgrammarError {
    /// A grammar/schema/structural-tag input was invalid.
    #[error("{message}")]
    Invalid {
        /// The underlying error message.
        message: String,
    },
}

/// A context-free grammar compiled from EBNF, JSON Schema, regex, or a structural tag.
#[bindings::export(Class)]
#[derive(Debug, Clone)]
pub struct Grammar {
    inner: xgrammar::grammar::Grammar,
}

#[bindings::export(Implementation)]
impl Grammar {
    /// Parses a grammar from an EBNF string (the C++ `Grammar::FromEBNF`).
    #[bindings::export(Method(Factory))]
    pub fn from_ebnf(
        ebnf_string: String,
        root_rule_name: String,
    ) -> Result<Grammar, XgrammarError> {
        xgrammar::grammar::Grammar::from_ebnf(&ebnf_string, &root_rule_name)
            .map(|inner| Grammar { inner })
            .map_err(|e| XgrammarError::Invalid {
                message: e.to_string(),
            })
    }

    /// Serializes the grammar to its `"v11"` JSON form.
    #[bindings::export(Method)]
    pub fn serialize_json(&self) -> String {
        self.inner.serialize_json()
    }
}
