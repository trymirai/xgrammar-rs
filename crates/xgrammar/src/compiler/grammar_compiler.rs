//! The grammar compiler with a result cache — a port of `GrammarCompiler` in
//! `cpp/grammar_compiler.cc`.
//!
//! Optimizes grammars (running the full functor pipeline so per-rule FSMs and
//! `allow_empty_rule_ids` are built) against a fixed tokenizer, caching the results keyed by
//! their source so repeated requests reuse the work.

use std::{collections::HashMap, sync::Mutex};

use super::compiled_grammar::CompiledGrammar;
use crate::{
    converter::StructuralTagError, functor::grammar_optimizer,
    grammar::Grammar, tokenizer::TokenizerInfo,
};

/// Unlimited cache size sentinel.
const UNLIMITED: i64 = -1;

/// Compiles grammars/schemas/regexes/structural-tags against a fixed tokenizer, with a cache.
#[derive(Debug)]
pub struct GrammarCompiler {
    tokenizer_info: TokenizerInfo,
    #[allow(dead_code)]
    max_threads: i32,
    cache_enabled: bool,
    max_memory_bytes: i64,
    cache: Mutex<HashMap<String, CompiledGrammar>>,
}

impl GrammarCompiler {
    /// Creates a compiler bound to `tokenizer_info`.
    #[must_use]
    pub fn new(
        tokenizer_info: TokenizerInfo,
        max_threads: i32,
        cache_enabled: bool,
        max_memory_bytes: i64,
    ) -> Self {
        Self {
            tokenizer_info,
            max_threads,
            cache_enabled,
            max_memory_bytes,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Creates a compiler with the C++ defaults (`max_threads=8`, cache on, unlimited memory).
    #[must_use]
    pub fn with_defaults(tokenizer_info: TokenizerInfo) -> Self {
        Self::new(tokenizer_info, 8, true, UNLIMITED)
    }

    /// Compiles an already-built grammar.
    #[must_use]
    pub fn compile_grammar(
        &self,
        grammar: &Grammar,
    ) -> CompiledGrammar {
        self.cached(format!("grammar:{grammar}"), || self.optimize(grammar))
    }

    /// Compiles a grammar from an EBNF string.
    ///
    /// # Panics
    /// Panics if `ebnf_str` fails to parse.
    #[must_use]
    pub fn compile_grammar_ebnf(
        &self,
        ebnf_str: &str,
        root_rule_name: &str,
    ) -> CompiledGrammar {
        self.cached(format!("ebnf:{root_rule_name}:{ebnf_str}"), || {
            let grammar = Grammar::from_ebnf(ebnf_str, root_rule_name)
                .expect("valid EBNF");
            self.optimize(&grammar)
        })
    }

    /// Compiles the built-in JSON grammar.
    #[must_use]
    pub fn compile_builtin_json_grammar(&self) -> CompiledGrammar {
        self.cached("builtin_json".to_owned(), || {
            self.optimize(&Grammar::builtin_json_grammar())
        })
    }

    /// Compiles a JSON Schema (see [`Grammar::from_json_schema`]).
    ///
    /// # Panics
    /// Panics if the schema is invalid.
    #[must_use]
    pub fn compile_json_schema(
        &self,
        schema: &str,
        any_whitespace: bool,
        indent: Option<i32>,
        separators: Option<(&str, &str)>,
        strict_mode: bool,
        max_whitespace_cnt: Option<i32>,
    ) -> CompiledGrammar {
        let key = format!(
            "schema:{any_whitespace}:{indent:?}:{separators:?}:{strict_mode}:{max_whitespace_cnt:?}:{schema}"
        );
        self.cached(key, || {
            let grammar = Grammar::from_json_schema(
                schema,
                any_whitespace,
                indent,
                separators,
                strict_mode,
                max_whitespace_cnt,
            )
            .expect("valid JSON schema");
            self.optimize(&grammar)
        })
    }

    /// Compiles a regex (see [`Grammar::from_regex`]).
    ///
    /// # Panics
    /// Panics if the regex is invalid.
    #[must_use]
    pub fn compile_regex(
        &self,
        regex: &str,
    ) -> CompiledGrammar {
        self.cached(format!("regex:{regex}"), || {
            let grammar = Grammar::from_regex(regex).expect("valid regex");
            self.optimize(&grammar)
        })
    }

    /// Compiles a structural tag (see [`Grammar::from_structural_tag`]).
    ///
    /// # Errors
    /// Returns a [`StructuralTagError`] if the structural tag is invalid.
    pub fn compile_structural_tag(
        &self,
        structural_tag_json: &str,
    ) -> Result<CompiledGrammar, StructuralTagError> {
        if self.cache_enabled
            && let Some(hit) = self
                .cache
                .lock()
                .expect("cache mutex")
                .get(&format!("stag:{structural_tag_json}"))
        {
            return Ok(hit.clone());
        }
        let grammar = Grammar::from_structural_tag_with_tokenizer(
            structural_tag_json,
            &self.tokenizer_info,
        )?;
        let compiled = self.optimize(&grammar);
        if self.cache_enabled {
            self.cache.lock().expect("cache mutex").insert(
                format!("stag:{structural_tag_json}"),
                compiled.clone(),
            );
        }
        Ok(compiled)
    }

    /// Clears the result cache.
    pub fn clear_cache(&self) {
        self.cache.lock().expect("cache mutex").clear();
    }

    /// The approximate cache memory usage, in bytes.
    #[must_use]
    pub fn get_cache_size_bytes(&self) -> i64 {
        self.cache
            .lock()
            .expect("cache mutex")
            .values()
            .map(|c| c.memory_size_bytes() as i64)
            .sum()
    }

    /// The configured cache memory limit (`-1` = unlimited).
    #[must_use]
    pub fn cache_limit_bytes(&self) -> i64 {
        self.max_memory_bytes
    }

    /// Optimizes `grammar` (if needed) and bundles it with the tokenizer.
    fn optimize(
        &self,
        grammar: &Grammar,
    ) -> CompiledGrammar {
        let optimized = if grammar.is_optimized() {
            grammar.clone()
        } else {
            grammar_optimizer(grammar)
        };
        CompiledGrammar::new(optimized, self.tokenizer_info.clone())
    }

    /// Returns the cached result for `key`, computing and storing it on a miss.
    fn cached(
        &self,
        key: String,
        compute: impl FnOnce() -> CompiledGrammar,
    ) -> CompiledGrammar {
        if self.cache_enabled
            && let Some(hit) = self.cache.lock().expect("cache mutex").get(&key)
        {
            return hit.clone();
        }
        let compiled = compute();
        if self.cache_enabled {
            self.cache
                .lock()
                .expect("cache mutex")
                .insert(key, compiled.clone());
        }
        compiled
    }
}
