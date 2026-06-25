//! `GrammarCompiler` and `CompiledGrammar` bindings.

use std::sync::Arc;

use crate::{
    error::{map_deserialize_error, map_error}, grammar::Grammar, tokenizer_info::TokenizerInfo,
};

/// A grammar compiled against a tokenizer, ready to drive a matcher.
#[bindings::export(Class)]
#[derive(Debug, Clone)]
pub struct CompiledGrammar {
    pub(crate) inner: xgrammar::compiler::CompiledGrammar,
}

impl CompiledGrammar {
    pub(crate) fn wrap(inner: xgrammar::compiler::CompiledGrammar) -> Self {
        Self {
            inner,
        }
    }
}

#[bindings::export(Implementation)]
impl CompiledGrammar {
    /// The underlying grammar.
    #[bindings::export(Method)]
    pub fn grammar(&self) -> Grammar {
        Grammar::wrap(self.inner.grammar().clone())
    }

    /// The tokenizer info the grammar was compiled against.
    #[bindings::export(Method)]
    pub fn tokenizer_info(&self) -> TokenizerInfo {
        TokenizerInfo::wrap(self.inner.tokenizer_info().clone())
    }

    /// Approximate in-memory size of the compiled grammar, in bytes.
    #[bindings::export(Method)]
    pub fn memory_size_bytes(&self) -> usize {
        self.inner.memory_size_bytes()
    }

    /// Serializes the compiled grammar without embedding the full tokenizer info.
    #[bindings::export(Method)]
    pub fn serialize_json(&self) -> String {
        self.inner.serialize_json()
    }

    /// Deserializes a compiled grammar bound to `tokenizer_info`.
    #[bindings::export(Method(Factory))]
    pub fn deserialize_json(
        json_string: String,
        tokenizer_info: TokenizerInfo,
    ) -> Result<CompiledGrammar, pyo3::PyErr> {
        xgrammar::compiler::CompiledGrammar::deserialize_json(
            &json_string,
            &tokenizer_info.inner,
        )
        .map(CompiledGrammar::wrap)
        .map_err(map_deserialize_error)
    }
}

/// Compiles grammars against a tokenizer, with a result cache.
#[bindings::export(Class)]
#[derive(Clone)]
pub struct GrammarCompiler {
    inner: Arc<xgrammar::compiler::GrammarCompiler>,
}

#[bindings::export(Implementation)]
impl GrammarCompiler {
    /// Creates a compiler bound to `tokenizer_info`.
    #[bindings::export(Method(Constructor))]
    pub fn new(
        tokenizer_info: TokenizerInfo,
        max_threads: i32,
        cache_enabled: bool,
        cache_limit_bytes: i64,
    ) -> GrammarCompiler {
        GrammarCompiler {
            inner: Arc::new(xgrammar::compiler::GrammarCompiler::new(
                tokenizer_info.inner,
                max_threads,
                cache_enabled,
                cache_limit_bytes,
            )),
        }
    }

    /// Compiles an existing [`Grammar`].
    #[bindings::export(Method)]
    pub fn compile_grammar_ebnf(
        &self,
        grammar: Grammar,
    ) -> CompiledGrammar {
        CompiledGrammar::wrap(self.inner.compile_grammar(&grammar.inner))
    }

    /// Compiles a grammar from an EBNF string.
    #[bindings::export(Method)]
    pub fn compile_grammar_from_strings(
        &self,
        ebnf_string: String,
        root_rule_name: String,
    ) -> CompiledGrammar {
        CompiledGrammar::wrap(
            self.inner.compile_grammar_ebnf(&ebnf_string, &root_rule_name),
        )
    }

    /// Compiles a JSON Schema string.
    #[bindings::export(Method)]
    #[allow(clippy::too_many_arguments)]
    pub fn compile_json_schema(
        &self,
        schema: String,
        any_whitespace: bool,
        indent: Option<i32>,
        separators: Option<(String, String)>,
        strict_mode: bool,
        max_whitespace_cnt: Option<i32>,
    ) -> CompiledGrammar {
        let seps = separators.as_ref().map(|(a, b)| (a.as_str(), b.as_str()));
        CompiledGrammar::wrap(self.inner.compile_json_schema(
            &schema,
            any_whitespace,
            indent,
            seps,
            strict_mode,
            max_whitespace_cnt,
        ))
    }

    /// Compiles a regular expression.
    #[bindings::export(Method)]
    pub fn compile_regex(
        &self,
        regex: String,
    ) -> CompiledGrammar {
        CompiledGrammar::wrap(self.inner.compile_regex(&regex))
    }

    /// Compiles a structural-tag JSON document.
    #[bindings::export(Method)]
    pub fn compile_structural_tag(
        &self,
        structural_tag_json: String,
    ) -> Result<CompiledGrammar, pyo3::PyErr> {
        self.inner
            .compile_structural_tag(&structural_tag_json)
            .map(CompiledGrammar::wrap)
            .map_err(map_error)
    }

    /// Compiles the built-in JSON grammar.
    #[bindings::export(Method)]
    pub fn compile_builtin_json_grammar(&self) -> CompiledGrammar {
        CompiledGrammar::wrap(self.inner.compile_builtin_json_grammar())
    }

    /// Clears the result cache.
    #[bindings::export(Method)]
    pub fn clear_cache(&self) {
        self.inner.clear_cache();
    }

    /// The current cache memory usage, in bytes.
    #[bindings::export(Method)]
    pub fn get_cache_size_bytes(&self) -> i64 {
        self.inner.get_cache_size_bytes()
    }

    /// The cache memory limit, in bytes.
    #[bindings::export(Method)]
    pub fn cache_limit_bytes(&self) -> i64 {
        self.inner.cache_limit_bytes()
    }
}
