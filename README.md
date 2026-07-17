<div align="center" id="top">

<img src="https://raw.githubusercontent.com/mlc-ai/xgrammar/main/assets/logo.svg" alt="XGrammar" width="400">

[![License](https://img.shields.io/badge/license-apache_2-blue)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/xgrammar-rs)](https://crates.io/crates/xgrammar-rs)
[![Documentation](https://docs.rs/xgrammar-rs/badge.svg)](https://docs.rs/xgrammar-rs)

**Pure-Rust XGrammar with generated Python, Swift, Node, and WebAssembly bindings**

</div>

This repository ports the XGrammar C++ core to safe Rust. The main crate has no C or C++
build dependency. Grammar parsing, JSON Schema and regex conversion, compilation, matching,
token masks, serialization, and tokenizer metadata all run in Rust.

The upstream [`mlc-ai/xgrammar`](https://github.com/mlc-ai/xgrammar) checkout is pinned as the
`xgrammar/` submodule. Its original Python tests are kept unchanged and run against this Rust
extension in CI.

## Packages

- `xgrammar-rs`: the pure-Rust core; imported as `xgrammar` in Rust code.
- `xgrammar`: the Python package, built as an ABI3 extension for Python 3.8 and newer.
- `xgrammar-py`: the shared binding layer. Uzu-style `#[bindings::export]` annotations generate
  PyO3, UniFFI, NAPI, and wasm-bindgen glue from the same Rust definitions.

## Supported targets

| Surface | Targets |
| --- | --- |
| Rust core | Linux x86-64/Arm64, macOS x86-64/Arm64, Windows x86-64/Arm64, iOS device/simulator, `wasm32-unknown-unknown` |
| Python | Linux x86-64/Arm64, macOS x86-64/Arm64, Windows x86-64/Arm64 |
| Swift/UniFFI | macOS Arm64, iOS Arm64, iOS Arm64 simulator |
| Node/NAPI | Linux, macOS, and Windows on x86-64/Arm64 |
| Browser wasm | `wasm32-unknown-unknown` |

This includes every desktop OS supported by the upstream C++ package and adds iOS and browser
WebAssembly.

## Rust quick start

```toml
[dependencies]
xgrammar-rs = "0.3"
```

```rust
use xgrammar::{
    Grammar, GrammarCompiler, GrammarMatcher, TokenizerInfo, VocabType,
    allocate_token_bitmask,
};

let grammar = Grammar::from_json_schema(
    r#"{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}"#,
    true,
    None,
    None,
    true,
    None,
)?;

let vocab = vec!["{".to_owned(), "}".to_owned(), "</s>".to_owned()];
let tokenizer = TokenizerInfo::new(&vocab, VocabType::RAW, None, None, false);
let compiler = GrammarCompiler::with_defaults(tokenizer.clone());
let compiled = compiler.compile_grammar(&grammar);
let mut matcher = GrammarMatcher::from_compiled_grammar(&compiled, false);
let mut bitmask = allocate_token_bitmask(1, tokenizer.vocab_size());
matcher.fill_next_token_bitmask(&mut bitmask, 0)?;

# Ok::<(), Box<dyn std::error::Error>>(())
```

Enable direct Hugging Face `tokenizers::Tokenizer` support with:

```toml
xgrammar-rs = { version = "0.3", features = ["tokenizers"] }
```

Then use `TokenizerInfo::from_huggingface(&tokenizer, vocab_size, stop_token_ids)`.

## Development

Initial setup (installs rustup / uv / pnpm when missing):

```bash
cargo tools setup
```

Language tooling and builds (driven by [`platforms.toml`](platforms.toml)):

```bash
cargo tools install python   # maturin, …
cargo tools install swift    # cargo-swift, …
cargo tools build rust --targets host
cargo tools build python --targets host
cargo tools build swift --targets host
cargo tools test rust
cargo tools test python
cargo tools test swift
```

Run the Rust suites directly:

```bash
cargo test -p xgrammar-rs
cargo test -p xgrammar-rs --features tokenizers
```

Build the Python package and run the untouched upstream suite with `uv`:

```bash
uv sync --project bindings/python --extra test
uv run --project bindings/python python -m pytest xgrammar/tests/python -m "not hf_token_required"
```

Check each generated binding backend independently:

```bash
cargo check -p xgrammar-py --no-default-features --features bindings-pyo3
cargo check -p xgrammar-py --no-default-features --features bindings-uniffi
cargo check -p xgrammar-py --no-default-features --features bindings-napi
cargo check -p xgrammar-py --target wasm32-unknown-unknown --no-default-features --features bindings-wasm
```

The authoritative platform list used by `cargo tools` lives in
[`platforms.toml`](platforms.toml).

## License

Apache-2.0. See [LICENSE](LICENSE).
