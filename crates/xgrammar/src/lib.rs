//! Efficient, flexible and portable **structured generation** for Rust.
//!
//! `xgrammar` is a pure-Rust reimplementation of [XGrammar](https://github.com/mlc-ai/xgrammar).
//! It builds the constrained-decoding grammar, compiles it against a tokenizer, and produces
//! token bitmasks for masking LLM logits — with no C/C++ dependency, on every platform Rust
//! targets (including `wasm32`).
//!
//! The public API is re-exported from [`xgrammar_core`]; this crate adds the optional
//! HuggingFace tokenizer integration (`hf`/`tokenizers` feature) and the cross-language
//! bindings layer.

pub use xgrammar_core::*;
