//! Pure-Rust core of [XGrammar](https://github.com/mlc-ai/xgrammar) — an open-source library
//! for efficient, flexible, and portable structured generation.
//!
//! XGrammar uses constrained decoding (grammar-guided generation) to enforce structural
//! correctness of model output. It supports general context-free grammars, including JSON,
//! regular expressions, and custom EBNF. A [`GrammarMatcher`](matcher::GrammarMatcher) drives
//! a non-deterministic pushdown automaton (NPDA) over a compiled grammar and tokenizer,
//! producing token bitmasks that downstream inference engines apply to logits.
//!
//! This crate is a from-scratch Rust reimplementation of the upstream xgrammar core. It
//! carries no C/C++ dependency and builds with `cargo` alone on every supported target
//! (macOS, Linux, Windows, iOS, `wasm32`).

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
    RegexError, SchemaError, SchemaErrorKind, StructuralTagError, StructuralTagItem, XmlJsonFormat,
    deepseek_xml_tool_calling_to_ebnf, generate_float_range_regex, generate_float_range_regex_with_options,
    generate_range_regex, glm_xml_tool_calling_to_ebnf, json_schema_to_ebnf, json_schema_to_ebnf_with_any_order,
    json_schema_to_ebnf_xml_with_options, minimax_xml_tool_calling_to_ebnf, qwen_xml_tool_calling_to_ebnf,
    regex_to_ebnf, xml_tool_calling_to_ebnf,
};
pub use grammar::{DeserializeError, Grammar};
pub use matcher::{
    BatchGrammarMatcher, BitmaskDlType, GrammarMatcher, MatcherTerminatedError, allocate_token_bitmask,
    apply_token_bitmask_inplace_cpu, apply_token_bitmask_inplace_cpu_batch, get_bitmask_dl_type, get_bitmask_size,
    get_masked_tokens_from_bitmask, is_single_token_bitmask, reset_token_bitmask,
};
pub use support::{RecursionError, get_max_recursion_depth, reset_recursion_depth, set_max_recursion_depth};
pub use testing::print_token_by_ids;
pub use tokenizer::{
    HfMetadata, TokenizerInfo, UnknownVocabType, VocabType, decode_token, decode_token_bytes, detect_metadata_from_hf,
};

/// The crate version, as declared in `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
