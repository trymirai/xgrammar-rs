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

pub mod grammar;
pub mod support;

/// The crate version, as declared in `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
