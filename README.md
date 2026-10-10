<div align="center" id="top">

<img src="https://raw.githubusercontent.com/mlc-ai/xgrammar/main/assets/logo.svg" alt="XGrammar" width="400">

[![License](https://img.shields.io/badge/license-apache_2-blue)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/xgrammar-rs)](https://crates.io/crates/xgrammar-rs)
[![Documentation](https://docs.rs/xgrammar-rs/badge.svg)](https://docs.rs/xgrammar-rs)

**Pure-Rust XGrammar with generated Python, Swift, Node, and WebAssembly bindings**

</div>

XGrammar is an open-source library for efficient, flexible, and portable structured generation.
It uses constrained decoding to ensure structural correctness of model output. It supports
general context-free grammars, including JSON, regular expressions, and custom EBNF. XGrammar
achieves near-zero overhead in JSON generation, making it one of the fastest structured
generation engines available.

This repository ports the upstream [`mlc-ai/xgrammar`](https://github.com/mlc-ai/xgrammar) core
to safe Rust. The main crate has no C or C++ build dependency. EBNF and Lark grammar parsing, JSON
Schema, regex, and structural-tag conversion, compilation, matching, token masks, serialization,
and tokenizer metadata all run in Rust.

## Versioning

Versions follow upstream releases: `0.2.8` is the port of upstream
[`v0.2.8`](https://github.com/mlc-ai/xgrammar/releases/tag/v0.2.8). Only published upstream
releases are tracked, not upstream `main`.

The pinned release (C++ sources and Python tests) is downloaded by Cargo into the gitignored
`external/xgrammar` when tests or development tools are built. The unpublished `xgrammar-upstream`
crate owns the pin; published crates never include or download it.

## Packages

- `xgrammar-rs`: the pure-Rust core; imported as `xgrammar` in Rust code.
- `xgrammar`: the Python package, an ABI3 extension for Python 3.8 and newer plus a free-threaded
  build. Its API is upstream's Python API.
- `XGrammar` (Swift), `xgrammar` (TypeScript/Node), and a wasm-bindgen module: the same decoding API
  for other languages.
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
xgrammar-rs = "0.2"
```

```rust
use xgrammar::{
    Grammar, GrammarCompiler, GrammarMatcher, TokenizerInfo, VocabType,
    allocate_token_bitmask, get_bitmask_size,
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
let tokenizer = TokenizerInfo::new(&vocab, VocabType::RAW, None, None, false)?;
let compiler = GrammarCompiler::with_defaults(tokenizer.clone());
let compiled = compiler.compile_grammar(&grammar)?;
let mut matcher = GrammarMatcher::from_compiled_grammar(&compiled, false);
let mut bitmask = allocate_token_bitmask(1, tokenizer.vocab_size());
let shape = [1, get_bitmask_size(tokenizer.vocab_size()).into()];
matcher.fill_next_token_bitmask(&mut bitmask, &shape, 0)?;

# Ok::<(), Box<dyn std::error::Error>>(())
```

Enable direct Hugging Face `tokenizers::Tokenizer` support with:

```toml
xgrammar-rs = { version = "0.2", features = ["tokenizers"] }
```

Then use `TokenizerInfo::from_huggingface(&tokenizer, vocab_size, stop_token_ids)`.

## Swift, TypeScript, and WebAssembly

The generated bindings expose `TokenizerInfo`, `Grammar` (with its `from_*` factories),
`GrammarCompiler`, `CompiledGrammar`, and `GrammarMatcher`, enough to run a full decode loop
(method names follow each language's convention, e.g. `nextTokenBitmask` in Swift and Node):

- `accept_token`, `accept_string`, `accept_bytes`, `accept_stop_token`, `rollback`, `reset`, `fork`;
- `next_token_bitmask()` returns an owned `TokenBitmask { words, need_apply }`: `ceil(vocab_size / 32)`
  `i32` words, where token `k` is allowed when bit `k % 32` of word `k / 32` is set;
- `find_jump_forward_string` / `find_jump_forward_bytes`, and `capture_values` returning
  `Capture { name, value }` with the raw captured bytes;
- `NamedGrammars` supplies the grammars a Lark source references as `@name`.

Batch matching and draft-tree traversal are currently Python-only. The API tests are runnable
examples: [`XGrammarTests.swift`](bindings/swift/Tests/XGrammarTests/XGrammarTests.swift),
[`api.test.ts`](bindings/typescript/tests/api.test.ts) (Node), and
[`api.test.ts`](bindings/wasm/tests/api.test.ts) (wasm).

## Performance

Runtime only (grammar compilation is not a target). A quick comparison against the official
upstream wheel of the same version, run in separate interpreters with identical Python
dependencies:

```bash
uv sync --locked --project bindings/python --extra test
uv run --no-sync --project bindings/python python -m pytest bindings/python/tests/test_upstream_perf.py
```

It prints the median time per call for both and fails if this package is more than 1.25x slower
on any row. Results from [CI](https://github.com/trymirai/xgrammar-rs/actions/runs/38063085870)
(Python 3.13, 32k-token vocabulary; this package / upstream, lower is better):

| Workload | Operation | Linux x86-64 | Windows x86-64 | macOS Arm64 |
| --- | --- | --- | --- | --- |
| Built-in JSON | `fill_next_token_bitmask` | 851 / 1041 ns (0.82x) | 1600 / 2600 ns (0.62x) | 1084 / 1292 ns (0.84x) |
| Built-in JSON | `accept_token` | 440 / 611 ns (0.72x) | 800 / 1100 ns (0.73x) | 500 / 584 ns (0.86x) |
| JSON Schema | `fill_next_token_bitmask` | 772 / 781 ns (0.99x) | 1500 / 2200 ns (0.68x) | 958 / 1000 ns (0.96x) |
| JSON Schema | `accept_token` | 370 / 531 ns (0.70x) | 700 / 1000 ns (0.70x) | 417 / 500 ns (0.83x) |

This is a small regression guard, not a full benchmark: two workloads and the two hottest calls,
measured on shared CI runners.

## Testing

Correctness is checked against upstream itself:

- **Upstream's Python suite, unmodified.** `external/xgrammar/tests/python` imports `xgrammar`,
  which resolves to this repository's package, so every upstream expectation (masks, accepted
  strings, errors, serialization) runs against the Rust core. Warnings are errors, except three
  exact warnings raised by upstream's own tests.
- **Upstream's C++ tests, ported** to Rust under `crates/xgrammar/tests/cpp`.
- **This repository's tests** in `bindings/python/tests` for behavior upstream does not test
  (byte vocabularies, tensor buffer safety, the shared API, serialization edge cases), plus the
  Swift, TypeScript, and wasm API tests above.
- **The performance comparison** above,
  [`test_upstream_perf.py`](bindings/python/tests/test_upstream_perf.py), runs with the other tests.

CI runs six jobs: lint (formatting, Clippy and rustdoc with warnings as errors on every crate and
binding backend, every release target, Rust 1.85, Ruff), tests on Linux, Windows, and macOS (Rust,
the full upstream Python suite, this repository's Python tests, the perf comparison, and the
Node, Swift, and wasm API tests), and the Python suite on Python 3.8 and free-threaded 3.14t.

## Development

The pinned upstream reference (C++ sources + Python tests) is downloaded into the gitignored
`external/xgrammar` by `cargo test` or `cargo tools`, or explicitly with:

```bash
cargo build -p xgrammar-upstream
```

It is only needed for the Python suite and for reading the C++ sources.

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

Run the Rust suites directly (C++ gtest port + rust_api):

```bash
cargo test -p xgrammar-rs
cargo test -p xgrammar-rs --features tokenizers
```

Python tests: this repository's tests (including the perf comparison), then the untouched
upstream suite. Tests marked `hf_token_required` need a Hugging Face token with access to the
gated tokenizers they load:

```bash
uv sync --locked --project bindings/python --extra test
uv run --no-sync --project bindings/python python -m pytest bindings/python/tests
uv run --no-sync --project bindings/python python -m pytest external/xgrammar/tests/python -m "not hf_token_required"
```

Node and Swift API tests (the wasm test steps are in [`ci.yml`](.github/workflows/ci.yml)):

```bash
cargo tools test typescript
cargo tools build swift --targets host && swift test
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

To move to a new upstream release, update `TAG` and `COMMIT` in
[`crates/xgrammar-upstream/build.rs`](crates/xgrammar-upstream/build.rs), set
`[workspace.package] version` in [`Cargo.toml`](Cargo.toml) to the same version, and run
`cargo tools sync` to propagate it to the Python and TypeScript packages.

## License

Apache-2.0. See [LICENSE](LICENSE).
