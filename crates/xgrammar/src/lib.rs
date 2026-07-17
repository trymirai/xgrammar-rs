//! Pure-Rust core of [XGrammar](https://github.com/mlc-ai/xgrammar) — an efficient,
//! flexible and portable engine for structured generation.
//!
//! This crate is a from-scratch Rust reimplementation of the xgrammar C++ core. It
//! carries no C/C++ dependency and builds with `cargo` alone on every supported target
//! (macOS, Linux, Windows, iOS, `wasm32`).
//!
//! Modules are introduced milestone by milestone:
//! `support` → `grammar` → `parser` → `converter` → `fsm` → `compiler` → `matcher`
//! → `tokenizer` → `error`.

#![forbid(unsafe_op_in_unsafe_fn)]

pub mod compiler;
pub mod config;
pub mod converter;
pub mod fsm;
pub mod functor;
pub mod grammar;
pub mod matcher;
pub mod parser;
pub mod support;
pub mod testing;
pub mod tokenizer;

// Keep the main consumer API available directly under `xgrammar::...`, as it was in
// xgrammar-rs 0.2, while retaining the more detailed module paths above.
pub use compiler::{CompiledGrammar, GrammarCompiler};
pub use config::{SERIALIZATION_VERSION, get_serialization_version};
pub use converter::{
    RegexError, SchemaError, SchemaErrorKind, StructuralTagError,
    XmlJsonFormat, deepseek_xml_tool_calling_to_ebnf,
    generate_float_range_regex, generate_float_range_regex_with_options,
    generate_range_regex, glm_xml_tool_calling_to_ebnf, json_schema_to_ebnf,
    json_schema_to_ebnf_with_any_order, json_schema_to_ebnf_xml_with_options,
    minimax_xml_tool_calling_to_ebnf, qwen_xml_tool_calling_to_ebnf,
    regex_to_ebnf, xml_tool_calling_to_ebnf,
};
pub use grammar::{DeserializeError, Grammar};
pub use matcher::{
    BatchGrammarMatcher, GrammarMatcher, MatcherTerminatedError,
    allocate_token_bitmask, apply_token_bitmask_inplace_cpu, get_bitmask_size,
    get_masked_tokens_from_bitmask, is_single_token_bitmask,
    reset_token_bitmask,
};
pub use support::{
    RecursionError, get_max_recursion_depth, reset_recursion_depth,
    set_max_recursion_depth,
};
pub use tokenizer::{
    HfMetadata, TokenizerInfo, UnknownVocabType, VocabType, decode_token,
    decode_token_bytes, detect_metadata_from_hf,
};

/// The crate version, as declared in `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
