//! `Grammar` binding — a thin opaque wrapper over [`xgrammar::grammar::Grammar`].

use crate::{
    error::{map_deserialize_error, map_error},
    named_grammars::NamedGrammars,
    tokenizer_info::TokenizerInfo,
};

/// A context-free grammar compiled from EBNF, JSON Schema, regex, or a structural tag.
#[bindings::export(Class)]
#[derive(Debug, Clone)]
pub struct Grammar {
    inner: xgrammar::grammar::Grammar,
}

impl From<xgrammar::grammar::Grammar> for Grammar {
    fn from(inner: xgrammar::grammar::Grammar) -> Self {
        Self {
            inner,
        }
    }
}

impl AsRef<xgrammar::grammar::Grammar> for Grammar {
    fn as_ref(&self) -> &xgrammar::grammar::Grammar {
        &self.inner
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
        xgrammar::grammar::Grammar::from_ebnf(&ebnf_string, &root_rule_name).map(Grammar::from).map_err(map_error)
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
        Ok(Grammar::from(g))
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
        Ok(Grammar::from(g))
    }

    /// Builds a grammar from a structural-tag JSON document.
    #[bindings::export(Method(Factory))]
    pub fn from_structural_tag(structural_tag_json: String) -> Result<Grammar, crate::error::BindingError> {
        xgrammar::grammar::Grammar::from_structural_tag(&structural_tag_json).map(Grammar::from).map_err(map_error)
    }

    /// Builds a grammar from a structural-tag JSON document, resolving string token
    /// references against `tokenizer_info`.
    #[bindings::export(Method(Factory))]
    pub fn from_structural_tag_with_tokenizer(
        structural_tag_json: String,
        tokenizer_info: &TokenizerInfo,
    ) -> Result<Grammar, crate::error::BindingError> {
        xgrammar::grammar::Grammar::from_structural_tag_with_tokenizer(&structural_tag_json, tokenizer_info.as_ref())
            .map(Grammar::from)
            .map_err(map_error)
    }

    /// Builds a grammar from Lark syntax; `named_grammars` resolves its `@name` references.
    #[bindings::export(Method(Factory))]
    pub fn from_lark(
        lark_string: String,
        named_grammars: &NamedGrammars,
    ) -> Result<Grammar, crate::error::BindingError> {
        xgrammar::grammar::Grammar::from_lark(&lark_string, None, &named_grammars.entries())
            .map(Grammar::from)
            .map_err(map_error)
    }

    /// Builds a grammar from Lark syntax, resolving named special tokens against
    /// `tokenizer_info`; `named_grammars` resolves its `@name` references.
    #[bindings::export(Method(Factory))]
    pub fn from_lark_with_tokenizer(
        lark_string: String,
        tokenizer_info: &TokenizerInfo,
        named_grammars: &NamedGrammars,
    ) -> Result<Grammar, crate::error::BindingError> {
        xgrammar::grammar::Grammar::from_lark(&lark_string, Some(tokenizer_info.as_ref()), &named_grammars.entries())
            .map(Grammar::from)
            .map_err(map_error)
    }

    /// The built-in JSON grammar.
    #[bindings::export(Method(Factory))]
    pub fn builtin_json_grammar() -> Grammar {
        Grammar::from(xgrammar::grammar::Grammar::builtin_json_grammar())
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
        Ok(Grammar::from(xgrammar::grammar::Grammar::union(&gs)))
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
        Ok(Grammar::from(xgrammar::grammar::Grammar::concat(&gs)))
    }

    /// Serializes the grammar to its `"v14"` JSON form.
    #[bindings::export(Method)]
    pub fn serialize_json(&self) -> String {
        self.inner.serialize_json()
    }

    /// Deserializes a grammar from its `"v14"` JSON form.
    #[bindings::export(Method(Factory))]
    pub fn deserialize_json(json_string: String) -> Result<Grammar, crate::error::BindingError> {
        xgrammar::grammar::Grammar::deserialize_json(&json_string).map(Grammar::from).map_err(map_deserialize_error)
    }

    /// The EBNF (GBNF) string form of the grammar.
    #[bindings::export(Method)]
    #[allow(clippy::inherent_to_string, reason = "the exported binding method mirrors upstream `Grammar.to_string`")]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}
