//! A cache to get the compiled grammar for a grammar or schema.
//!
//! This class avoids redundant preprocessing of the grammar or schema when constructing a
//! [`CompiledGrammar`].
//!
//! This class is associated with a vocabulary when constructed. The vocabulary is used to
//! create every compiled grammar. If multiple token tables are used to create init contexts,
//! an instance of this class for each vocabulary should be created.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

use super::{
    compiled_grammar::CompiledGrammar,
    rule_level_cache::{RuleLevelCache, UNLIMITED_SIZE},
};
use crate::{
    converter::StructuralTagError,
    functor::{grammar_fsm_hasher, grammar_optimizer},
    grammar::Grammar,
    tokenizer::TokenizerInfo,
};

/// Unlimited cache size sentinel.
const UNLIMITED: i64 = -1;

#[derive(Debug)]
struct CacheState {
    map: HashMap<String, CompiledGrammar>,
    /// Front = least recently used; back = most recently used.
    order: VecDeque<String>,
    size_bytes: i64,
}

impl CacheState {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            size_bytes: 0,
        }
    }

    fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
        self.size_bytes = 0;
    }

    fn get(
        &mut self,
        key: &str,
    ) -> Option<CompiledGrammar> {
        if !self.map.contains_key(key) {
            return None;
        }
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            let k = self.order.remove(pos).expect("position valid");
            self.order.push_back(k);
        }
        self.map.get(key).cloned()
    }

    fn insert(
        &mut self,
        key: String,
        value: CompiledGrammar,
        max_memory_bytes: i64,
    ) {
        let entry_size = value.memory_size_bytes() as i64;
        if let Some(old) = self.map.remove(&key) {
            self.size_bytes -= old.memory_size_bytes() as i64;
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                self.order.remove(pos);
            }
        }
        self.map.insert(key.clone(), value);
        self.order.push_back(key);
        self.size_bytes += entry_size;
        if self.size_bytes < 0 {
            self.size_bytes = 0;
        }
        self.evict_to_limit(max_memory_bytes);
    }

    fn evict_to_limit(
        &mut self,
        max_memory_bytes: i64,
    ) {
        if max_memory_bytes < 0 {
            return;
        }
        while self.size_bytes > max_memory_bytes {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(removed) = self.map.remove(&oldest) {
                self.size_bytes -= removed.memory_size_bytes() as i64;
            }
        }
        if self.size_bytes < 0 {
            self.size_bytes = 0;
        }
    }
}

/// A cache to get the compiled grammar for a grammar or schema.
///
/// Avoids redundant preprocessing when constructing a [`CompiledGrammar`]. Always creates
/// compiled grammars with the vocabulary supplied at construction.
#[derive(Debug)]
pub struct GrammarCompiler {
    tokenizer_info: Arc<TokenizerInfo>,
    max_threads: i32,
    cache_enabled: bool,
    max_memory_bytes: i64,
    grammar_cache_limit_bytes: i64,
    rule_level_cache: Option<Arc<RuleLevelCache>>,
    cache: Mutex<CacheState>,
}

impl GrammarCompiler {
    /// Constructs a [`GrammarCompiler`] with a vocabulary.
    ///
    /// This class will always create compiled grammars with this vocabulary.
    ///
    /// `max_threads` is the maximum number of threads to use for compiling grammars.
    /// `cache_enabled` controls whether the cache is enabled.
    /// `max_memory_bytes` is the maximum memory usage in bytes (`-1` for unlimited).
    #[must_use]
    pub fn new(
        tokenizer_info: TokenizerInfo,
        max_threads: i32,
        cache_enabled: bool,
        max_memory_bytes: i64,
    ) -> Self {
        assert!(max_memory_bytes >= -1, "Invalid max_memory_bytes: {max_memory_bytes}. Must be -1 (unlimited) or >= 0");
        let grammar_cache_limit_bytes = if max_memory_bytes < 0 {
            UNLIMITED
        } else {
            max_memory_bytes / 3 * 2
        };
        let rule_level_cache = if cache_enabled {
            Some(Arc::new(RuleLevelCache::new(if max_memory_bytes < 0 {
                UNLIMITED_SIZE
            } else {
                (max_memory_bytes - max_memory_bytes / 3 * 2) as usize
            })))
        } else {
            None
        };
        Self {
            tokenizer_info: Arc::new(tokenizer_info),
            max_threads,
            cache_enabled,
            max_memory_bytes,
            grammar_cache_limit_bytes,
            rule_level_cache,
            cache: Mutex::new(CacheState::new()),
        }
    }

    /// Creates a compiler with default settings (`max_threads=8`, cache enabled, unlimited memory).
    #[must_use]
    pub fn with_defaults(tokenizer_info: TokenizerInfo) -> Self {
        Self::new(tokenizer_info, 8, true, UNLIMITED)
    }

    /// Gets the compiled grammar for a grammar.
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
            let grammar = Grammar::from_ebnf(ebnf_str, root_rule_name).expect("valid EBNF");
            self.optimize(&grammar)
        })
    }

    /// Gets the compiled grammar for pure JSON.
    #[must_use]
    pub fn compile_builtin_json_grammar(&self) -> CompiledGrammar {
        self.cached("builtin_json".to_owned(), || self.optimize(&Grammar::builtin_json_grammar()))
    }

    /// Gets the compiled grammar for a JSON schema string.
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
        self.compile_json_schema_with_any_order(
            schema,
            any_whitespace,
            indent,
            separators,
            strict_mode,
            max_whitespace_cnt,
            false,
        )
    }

    /// Gets the compiled grammar for a JSON schema string, with optional property-order
    /// independence.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn compile_json_schema_with_any_order(
        &self,
        schema: &str,
        any_whitespace: bool,
        indent: Option<i32>,
        separators: Option<(&str, &str)>,
        strict_mode: bool,
        max_whitespace_cnt: Option<i32>,
        any_order: bool,
    ) -> CompiledGrammar {
        let key = format!(
            "schema:{any_whitespace}:{indent:?}:{separators:?}:{strict_mode}:{max_whitespace_cnt:?}:{any_order}:{schema}"
        );
        self.cached(key, || {
            let grammar = Grammar::from_json_schema_with_any_order(
                schema,
                any_whitespace,
                indent,
                separators,
                strict_mode,
                max_whitespace_cnt,
                any_order,
            )
            .expect("valid JSON schema");
            self.optimize(&grammar)
        })
    }

    /// Gets the compiled grammar for a regex.
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

    /// Gets the compiled grammar for a structural tag.
    ///
    /// # Errors
    /// Returns a [`StructuralTagError`] if the structural tag is invalid.
    pub fn compile_structural_tag(
        &self,
        structural_tag_json: &str,
    ) -> Result<CompiledGrammar, StructuralTagError> {
        let key = format!("stag:{structural_tag_json}");
        if self.cache_enabled {
            if let Some(hit) = self.cache.lock().expect("cache mutex").get(&key) {
                return Ok(hit);
            }
        }
        let grammar = Grammar::from_structural_tag_with_tokenizer(structural_tag_json, &self.tokenizer_info)?;
        let compiled = self.optimize(&grammar);
        if self.cache_enabled {
            self.cache.lock().expect("cache mutex").insert(key, compiled.clone(), self.grammar_cache_limit_bytes);
        }
        Ok(compiled)
    }

    /// Clears the internal cache of compiled grammars.
    pub fn clear_cache(&self) {
        self.cache.lock().expect("cache mutex").clear();
    }

    /// Returns the approximate memory usage of the compiler cache in bytes.
    #[must_use]
    pub fn get_cache_size_bytes(&self) -> i64 {
        self.cache.lock().expect("cache mutex").size_bytes
    }

    /// Returns the approximate memory usage limit of the compiler cache in bytes.
    ///
    /// `-1` means unlimited.
    #[must_use]
    pub fn cache_limit_bytes(&self) -> i64 {
        self.max_memory_bytes
    }

    /// The configured compile parallelism for the adaptive token-mask cache.
    #[must_use]
    pub fn max_threads(&self) -> i32 {
        self.max_threads
    }

    /// Optimizes `grammar` (if needed), builds the adaptive token-mask cache, and bundles
    /// the result with the tokenizer.
    fn optimize(
        &self,
        grammar: &Grammar,
    ) -> CompiledGrammar {
        let mut optimized = if grammar.is_optimized() {
            grammar.clone()
        } else {
            grammar_optimizer(grammar)
        };
        if self.rule_level_cache.is_some() {
            grammar_fsm_hasher(&mut optimized);
        }
        CompiledGrammar::build(
            optimized,
            Arc::clone(&self.tokenizer_info),
            self.max_threads,
            self.rule_level_cache.as_ref().map(Arc::clone),
        )
    }

    /// Returns the cached result for `key`, computing and storing it on a miss.
    fn cached(
        &self,
        key: String,
        compute: impl FnOnce() -> CompiledGrammar,
    ) -> CompiledGrammar {
        if !self.cache_enabled {
            return compute();
        }
        let mut cache = self.cache.lock().expect("cache mutex");
        if let Some(hit) = cache.get(&key) {
            return hit;
        }
        // Drop the lock while compiling; a racing insert is rare and harmless.
        drop(cache);
        let compiled = compute();
        self.cache.lock().expect("cache mutex").insert(key, compiled.clone(), self.grammar_cache_limit_bytes);
        compiled
    }
}
