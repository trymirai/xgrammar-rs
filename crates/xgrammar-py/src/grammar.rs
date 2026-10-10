//! `Grammar` binding — a thin opaque wrapper over [`xgrammar::grammar::Grammar`].

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
    ) -> Result<Grammar, crate::error::BindingError> {
        xgrammar::grammar::Grammar::from_ebnf(&ebnf_string, &root_rule_name).map(Grammar::wrap).map_err(map_error)
    }

    /// Builds a grammar from a JSON Schema string (the C++ `Grammar::FromJSONSchema`).
    #[bindings::export(Method(Factory))]
    #[allow(clippy::too_many_arguments)]
    pub fn from_json_schema(
        schema: String,
        any_whitespace: bool,
        indent: Option<i32>,
        separator_item: Option<String>,
        separator_kv: Option<String>,
        strict_mode: bool,
        max_whitespace_cnt: Option<i32>,
        print_converted_ebnf: bool,
        any_order: bool,
    ) -> Result<Grammar, crate::error::BindingError> {
        let seps = match (&separator_item, &separator_kv) {
            (Some(a), Some(b)) => Some((a.as_str(), b.as_str())),
            _ => None,
        };
        let g = xgrammar::grammar::Grammar::from_json_schema_with_any_order(
            &schema,
            any_whitespace,
            indent,
            seps,
            strict_mode,
            max_whitespace_cnt,
            any_order,
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
    ) -> Result<Grammar, crate::error::BindingError> {
        let g = xgrammar::grammar::Grammar::from_regex(&regex_string).map_err(map_error)?;
        if print_converted_ebnf {
            println!("{g}");
        }
        Ok(Grammar::wrap(g))
    }

    /// Builds a grammar from a structural-tag JSON document.
    #[bindings::export(Method(Factory))]
    pub fn from_structural_tag(structural_tag_json: String) -> Result<Grammar, crate::error::BindingError> {
        xgrammar::grammar::Grammar::from_structural_tag(&structural_tag_json).map(Grammar::wrap).map_err(map_error)
    }

    /// Builds a grammar from a structural-tag JSON document, resolving string token
    /// references against `tokenizer_info`.
    #[bindings::export(Method(Factory))]
    pub fn from_structural_tag_with_tokenizer(
        structural_tag_json: String,
        tokenizer_info: &crate::tokenizer_info::TokenizerInfo,
    ) -> Result<Grammar, crate::error::BindingError> {
        xgrammar::grammar::Grammar::from_structural_tag_with_tokenizer(&structural_tag_json, &tokenizer_info.inner)
            .map(Grammar::wrap)
            .map_err(map_error)
    }

    /// The built-in JSON grammar.
    #[bindings::export(Method(Factory))]
    pub fn builtin_json_grammar() -> Grammar {
        Grammar::wrap(xgrammar::grammar::Grammar::builtin_json_grammar())
    }

    /// A grammar accepting any string accepted by one of `grammars`.
    ///
    /// Takes the serialized JSON form of each grammar (`Vec<String>`) rather than opaque
    /// handles: a `Vec` of handles cannot be passed uniformly across every binding backend.
    #[bindings::export(Method(Factory))]
    pub fn union(grammars: Vec<String>) -> Result<Grammar, crate::error::BindingError> {
        let gs = grammars
            .iter()
            .map(|j| xgrammar::grammar::Grammar::deserialize_json(j))
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_deserialize_error)?;
        Ok(Grammar::wrap(xgrammar::grammar::Grammar::union(&gs)))
    }

    /// A grammar accepting the in-order concatenation of strings from `grammars`.
    ///
    /// Takes the serialized JSON form of each grammar (`Vec<String>`); see [`Grammar::union`].
    #[bindings::export(Method(Factory))]
    pub fn concat(grammars: Vec<String>) -> Result<Grammar, crate::error::BindingError> {
        let gs = grammars
            .iter()
            .map(|j| xgrammar::grammar::Grammar::deserialize_json(j))
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_deserialize_error)?;
        Ok(Grammar::wrap(xgrammar::grammar::Grammar::concat(&gs)))
    }

    /// Serializes the grammar to its `"v14"` JSON form.
    #[bindings::export(Method)]
    pub fn serialize_json(&self) -> String {
        self.inner.serialize_json()
    }

    /// Deserializes a grammar from its `"v14"` JSON form.
    #[bindings::export(Method(Factory))]
    pub fn deserialize_json(json_string: String) -> Result<Grammar, crate::error::BindingError> {
        xgrammar::grammar::Grammar::deserialize_json(&json_string).map(Grammar::wrap).map_err(map_deserialize_error)
    }

    /// The EBNF (GBNF) string form of the grammar.
    #[bindings::export(Method)]
    #[allow(clippy::inherent_to_string, reason = "the exported binding method mirrors upstream `Grammar.to_string`")]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}

#[cfg(feature = "bindings-pyo3")]
pub use grammar_pyo3_ext::named_grammars_from_py;

/// PyO3-only `Grammar.from_lark`: its named grammar values mix `Grammar` handles and Lark
/// strings, which the cross-backend export cannot express.
#[cfg(feature = "bindings-pyo3")]
mod grammar_pyo3_ext {
    use pyo3::prelude::*;
    use xgrammar::converter::NamedGrammar;

    use super::Grammar;
    use crate::{error::map_error, tokenizer_info::TokenizerInfo};

    /// Pairs parallel name and value lists, where each value is a `Grammar` or a Lark source.
    pub fn named_grammars_from_py(
        names: Vec<String>,
        values: Vec<Bound<'_, PyAny>>,
    ) -> PyResult<Vec<NamedGrammar>> {
        if names.len() != values.len() {
            return Err(map_error("Named grammar names and values must have the same length"));
        }
        names
            .into_iter()
            .zip(values)
            .map(|(name, value)| {
                if let Ok(grammar) = value.extract::<PyRef<'_, Grammar>>() {
                    return Ok(NamedGrammar::Grammar {
                        name,
                        grammar: Box::new(grammar.inner.clone()),
                    });
                }
                let source = value
                    .extract::<String>()
                    .map_err(|_| map_error("Named grammar values must be Grammar or Lark strings"))?;
                Ok(NamedGrammar::Lark {
                    name,
                    source,
                })
            })
            .collect()
    }

    #[pyo3::pymethods]
    impl Grammar {
        #[staticmethod]
        #[pyo3(name = "from_lark")]
        fn from_lark_py(
            lark_string: &str,
            tokenizer_info: Option<PyRef<'_, TokenizerInfo>>,
            names: Vec<String>,
            values: Vec<Bound<'_, PyAny>>,
        ) -> PyResult<Grammar> {
            let named_grammars = named_grammars_from_py(names, values)?;
            let tokenizer_info = tokenizer_info.as_ref().map(|info| &info.inner);
            xgrammar::grammar::Grammar::from_lark(lark_string, tokenizer_info, &named_grammars)
                .map(Grammar::wrap)
                .map_err(map_error)
        }
    }
}
