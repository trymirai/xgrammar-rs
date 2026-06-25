//! `Grammar` binding — a thin opaque wrapper over [`xgrammar::grammar::Grammar`].

use pyo3::PyErr;

use crate::error::{map_deserialize_error, map_error};

/// A context-free grammar compiled from EBNF, JSON Schema, regex, or a structural tag.
#[bindings::export(Class)]
#[derive(Debug, Clone)]
pub struct Grammar {
    pub(crate) inner: xgrammar::grammar::Grammar,
}

impl Grammar {
    pub(crate) fn wrap(inner: xgrammar::grammar::Grammar) -> Self {
        Self {
            inner,
        }
    }
}

#[bindings::export(Implementation)]
impl Grammar {
    /// Parses a grammar from an EBNF string (the C++ `Grammar::FromEBNF`).
    #[bindings::export(Method(Factory))]
    pub fn from_ebnf(
        ebnf_string: String,
        root_rule_name: String,
    ) -> Result<Grammar, PyErr> {
        xgrammar::grammar::Grammar::from_ebnf(&ebnf_string, &root_rule_name)
            .map(Grammar::wrap)
            .map_err(map_error)
    }

    /// Builds a grammar from a JSON Schema string (the C++ `Grammar::FromJSONSchema`).
    #[bindings::export(Method(Factory))]
    #[allow(clippy::too_many_arguments)]
    pub fn from_json_schema(
        schema: String,
        any_whitespace: bool,
        indent: Option<i32>,
        separators: Option<(String, String)>,
        strict_mode: bool,
        max_whitespace_cnt: Option<i32>,
        print_converted_ebnf: bool,
    ) -> Result<Grammar, PyErr> {
        let seps = separators.as_ref().map(|(a, b)| (a.as_str(), b.as_str()));
        let g = xgrammar::grammar::Grammar::from_json_schema(
            &schema,
            any_whitespace,
            indent,
            seps,
            strict_mode,
            max_whitespace_cnt,
        )
        .map_err(map_error)?;
        if print_converted_ebnf {
            println!("{g}");
        }
        Ok(Grammar::wrap(g))
    }

    /// Builds a grammar from a regular expression (the C++ `Grammar::FromRegex`).
    #[bindings::export(Method(Factory))]
    pub fn from_regex(
        regex_string: String,
        print_converted_ebnf: bool,
    ) -> Result<Grammar, PyErr> {
        let g = xgrammar::grammar::Grammar::from_regex(&regex_string)
            .map_err(map_error)?;
        if print_converted_ebnf {
            println!("{g}");
        }
        Ok(Grammar::wrap(g))
    }

    /// Builds a grammar from a structural-tag JSON document.
    #[bindings::export(Method(Factory))]
    pub fn from_structural_tag(
        structural_tag_json: String
    ) -> Result<Grammar, PyErr> {
        xgrammar::grammar::Grammar::from_structural_tag(&structural_tag_json)
            .map(Grammar::wrap)
            .map_err(map_error)
    }

    /// The built-in JSON grammar.
    #[bindings::export(Method(Factory))]
    pub fn builtin_json_grammar() -> Grammar {
        Grammar::wrap(xgrammar::grammar::Grammar::builtin_json_grammar())
    }

    /// A grammar accepting any string accepted by one of `grammars`.
    #[bindings::export(Method(Factory))]
    pub fn union(grammars: Vec<Grammar>) -> Grammar {
        let gs: Vec<_> = grammars.into_iter().map(|g| g.inner).collect();
        Grammar::wrap(xgrammar::grammar::Grammar::union(&gs))
    }

    /// A grammar accepting the in-order concatenation of strings from `grammars`.
    #[bindings::export(Method(Factory))]
    pub fn concat(grammars: Vec<Grammar>) -> Grammar {
        let gs: Vec<_> = grammars.into_iter().map(|g| g.inner).collect();
        Grammar::wrap(xgrammar::grammar::Grammar::concat(&gs))
    }

    /// Serializes the grammar to its `"v11"` JSON form.
    #[bindings::export(Method)]
    pub fn serialize_json(&self) -> String {
        self.inner.serialize_json()
    }

    /// Deserializes a grammar from its `"v11"` JSON form.
    #[bindings::export(Method(Factory))]
    pub fn deserialize_json(json_string: String) -> Result<Grammar, PyErr> {
        xgrammar::grammar::Grammar::deserialize_json(&json_string)
            .map(Grammar::wrap)
            .map_err(map_deserialize_error)
    }

    /// The EBNF (GBNF) string form of the grammar.
    #[bindings::export(Method)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}
