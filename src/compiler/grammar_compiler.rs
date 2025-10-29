use std::pin::Pin;

use autocxx::WithinBox;

use crate::{
    FFIGrammarCompiler,
    compiler::CompiledGrammar,
    cxx_utils,
    grammar::{self, StructuralTagItem},
    tokenizer_info::TokenizerInfo,
};

/// The compiler for grammars. It is associated with a certain tokenizer info, and compiles
/// grammars into `CompiledGrammar` with the tokenizer info. It allows parallel compilation with
/// multiple threads, and has a cache to store the compilation result, avoiding compiling the
/// same grammar multiple times.
pub struct GrammarCompiler {
    inner: Pin<Box<FFIGrammarCompiler>>,
}

impl GrammarCompiler {
    /// Construct the compiler.
    ///
    /// Parameters
    /// - `tokenizer_info`: The tokenizer info.
    /// - `max_threads` (default: 8): The maximum number of threads used to compile the grammar.
    /// - `cache_enabled` (default: true): Whether to enable the cache.
    /// - `cache_limit_bytes` (default: -1): The maximum memory usage for the cache in bytes.
    ///   Note that the actual memory usage may slightly exceed this value.
    pub fn new(
        tokenizer_info: &TokenizerInfo,
        max_threads: i32,
        cache_enabled: bool,
        cache_limit_bytes: isize,
    ) -> Self {
        Self {
            inner: FFIGrammarCompiler::new(
                tokenizer_info.ffi_ref(),
                autocxx::c_int(max_threads),
                cache_enabled,
                autocxx::c_longlong(cache_limit_bytes as i64),
            )
            .within_box(),
        }
    }

    /// Get `CompiledGrammar` from the specified JSON schema and format. The indent
    /// and separators parameters follow the same convention as in serde_json's pretty printing
    /// (mirroring Python's json.dumps()).
    ///
    /// Parameters
    /// - `schema`: The schema string.
    /// - `any_whitespace`: Whether to allow any whitespace regardless of indent/separators.
    /// - `indent`: The number of spaces for indentation. If None, the output will be in one line.
    /// - `separators`: Two separators used in the schema: comma and colon. Examples: (",", ":"),
    ///   (", ", ": "). If None, defaults to (",", ": ") when indent is Some, otherwise
    ///   (", ", ": ").
    /// - `strict_mode`: Whether to use strict mode. In strict mode, the generated grammar will not
    ///   allow properties and items that are not specified in the schema. This is equivalent to
    ///   setting unevaluatedProperties and unevaluatedItems to false.
    pub fn compile_json_schema(
        &self,
        schema: &str,
        any_whitespace: bool,
        indent: Option<i32>,
        separators: Option<(impl AsRef<str>, impl AsRef<str>)>,
        strict_mode: bool,
    ) -> CompiledGrammar {
        cxx::let_cxx_string!(schema_cxx = schema);
        let has_indent = indent.is_some();
        let indent_i32: i32 = indent.unwrap_or(0);
        let has_separators = separators.is_some();
        let (sep_comma, sep_colon) = if let Some((comma, colon)) = separators {
            (comma.as_ref().to_string(), colon.as_ref().to_string())
        } else {
            (String::new(), String::new())
        };
        cxx::let_cxx_string!(sep_comma_cxx = sep_comma.as_str());
        cxx::let_cxx_string!(sep_colon_cxx = sep_colon.as_str());

        let ffi_pin = unsafe {
            cxx_utils::compiler_compile_json_schema(
                self.ffi_ref() as *const _ as *mut _,
                &schema_cxx,
                any_whitespace,
                has_indent,
                autocxx::c_int(indent_i32),
                has_separators,
                &sep_comma_cxx,
                &sep_colon_cxx,
                strict_mode,
            )
            .within_box()
        };

        CompiledGrammar::from_pinned_ffi(ffi_pin)
    }

    /// Get `CompiledGrammar` from the standard JSON.
    pub fn compile_builtin_json_grammar(&self) -> CompiledGrammar {
        let ffi_pin = unsafe {
            cxx_utils::compiler_compile_builtin_json_grammar(self.ffi_ref()
                as *const _
                as *mut _)
            .within_box()
        };
        CompiledGrammar::from_pinned_ffi(ffi_pin)
    }

    /// Get `CompiledGrammar` from the specified regex.
    pub fn compile_regex(
        &self,
        regex: &str,
    ) -> CompiledGrammar {
        cxx::let_cxx_string!(regex_cxx = regex);
        let ffi_pin = unsafe {
            cxx_utils::compiler_compile_regex(
                self.ffi_ref() as *const _ as *mut _,
                &regex_cxx,
            )
            .within_box()
        };
        CompiledGrammar::from_pinned_ffi(ffi_pin)
    }

    /// Compile a grammar from structural tags.
    ///
    /// Parameters
    /// - `tags`: The structural tags.
    /// - `triggers`: The triggers. Each trigger should be a prefix of a provided begin tag.
    pub fn compile_structural_tag(
        &self,
        tags: &[StructuralTagItem],
        triggers: &[impl AsRef<str>],
    ) -> CompiledGrammar {
        let mut structural_tag_vector = cxx_utils::new_structural_tag_vector();
        let mut trigger_string_vector = cxx_utils::new_string_vector();

        {
            let mut tag_vec_pin = structural_tag_vector.pin_mut();
            let mut trig_vec_pin = trigger_string_vector.pin_mut();
            cxx_utils::structural_tag_vec_reserve(
                tag_vec_pin.as_mut(),
                tags.len(),
            );
            cxx_utils::string_vec_reserve(
                trig_vec_pin.as_mut(),
                triggers.len(),
            );

            for tag in tags {
                cxx::let_cxx_string!(begin_cxx = tag.begin.as_str());
                cxx::let_cxx_string!(schema_cxx = tag.schema.as_str());
                cxx::let_cxx_string!(end_cxx = tag.end.as_str());
                cxx_utils::structural_tag_vec_push(
                    tag_vec_pin.as_mut(),
                    &begin_cxx,
                    &schema_cxx,
                    &end_cxx,
                );
            }
            for trig in triggers {
                let tb = trig.as_ref().as_bytes();
                unsafe {
                    cxx_utils::string_vec_push_bytes(
                        trig_vec_pin.as_mut(),
                        tb.as_ptr() as *const i8,
                        tb.len(),
                    );
                }
            }
        }

        let ffi_pin = unsafe {
            cxx_utils::compiler_compile_structural_tag(
                self.ffi_ref() as *const _ as *mut _,
                structural_tag_vector.as_ref().unwrap(),
                trigger_string_vector.as_ref().unwrap(),
            )
            .within_box()
        };
        CompiledGrammar::from_pinned_ffi(ffi_pin)
    }

    /// Compile a grammar object to a `CompiledGrammar`.
    pub fn compile_grammar(
        &self,
        grammar: &grammar::Grammar,
    ) -> CompiledGrammar {
        let ffi_pin = unsafe {
            cxx_utils::compiler_compile_grammar(
                self.ffi_ref() as *const _ as *mut _,
                grammar.ffi_ref(),
            )
            .within_box()
        };
        CompiledGrammar::from_pinned_ffi(ffi_pin)
    }

    /// Compile a grammar from an EBNF string. The string should follow the format described in
    /// https://github.com/ggerganov/llama.cpp/blob/master/grammars/README.md.
    ///
    /// Parameters
    /// - `ebnf_string`: The grammar string in EBNF format.
    /// - `root_rule_name`: The name of the root rule in the grammar.
    pub fn compile_grammar_from_ebnf(
        &self,
        ebnf_string: &str,
        root_rule_name: &str,
    ) -> CompiledGrammar {
        let grammar = grammar::Grammar::from_ebnf(ebnf_string, root_rule_name);
        self.compile_grammar(&grammar)
    }

    /// Clear all cached compiled grammars.
    pub fn clear_cache(&mut self) {
        unsafe {
            cxx_utils::compiler_clear_cache(self.ffi_ref() as *const _ as *mut _)
        };
    }

    /// The approximate memory usage of the cache in bytes.
    pub fn get_cache_size_bytes(&self) -> i64 {
        unsafe { cxx_utils::compiler_get_cache_size_bytes(self.ffi_ref()) }
            .into()
    }

    /// The maximum memory usage for the cache in bytes. Returns -1 if unlimited.
    pub fn cache_limit_bytes(&self) -> i64 {
        unsafe { cxx_utils::compiler_cache_limit_bytes(self.ffi_ref()) }.into()
    }

    pub(crate) fn ffi_ref(&self) -> &FFIGrammarCompiler {
        self.inner.as_ref().get_ref()
    }
}
