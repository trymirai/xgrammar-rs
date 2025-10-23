# xgrammar-rs Copilot Instructions

## Project Overview

This is a Rust FFI wrapper for XGrammar, a C++ library for efficient structured generation (JSON, regex, context-free grammars) used in LLM inference engines. The project uses `autocxx` to generate Rust bindings for the upstream C++ library located in `external/xgrammar/`.

## Architecture

### Build System (Hybrid CMake + Cargo)

- **`build.rs`**: Critical build orchestration script that:
  1. Configures and builds the C++ XGrammar library using CMake (`cmake::Config`)
  2. Generates Rust bindings via `autocxx-build::Builder`
  3. Handles cross-platform linking (macOS uses `libc++`, Linux uses `libstdc++`)
  4. Supports cross-compilation for iOS and iOS simulator targets
  5. Copies headers to expected locations for generated code (`autocxxgen_ffi.h`, `xgrammar/xgrammar.h`, `dlpack/dlpack.h`)

- **CMake Configuration**: Disables Python bindings and C++ tests via:
  ```rust
  cmake_config.define("XGRAMMAR_BUILD_PYTHON_BINDINGS", "OFF");
  cmake_config.define("XGRAMMAR_BUILD_CXX_TESTS", "OFF");
  cmake_config.define("XGRAMMAR_ENABLE_CPPTRACE", "OFF");
  ```

### FFI Layer

- **`src/lib.rs`**: Main entry point using `autocxx::include_cpp!` macro to generate bindings for:
  - Core XGrammar types: `TokenizerInfo`, `GrammarCompiler`, `CompiledGrammar`, `Grammar`, `GrammarMatcher`
  - DLPack tensor types: `DLTensor`, `DLManagedTensor`, `DLDevice`, `DLDataType`
  - Helper utilities from `cxx_utils.h`

- **`src/cxx_utils.h`**: C++ helper functions that wrap XGrammar APIs to be autocxx-compatible:
  - `new_string_vector()`, `string_vec_push_bytes()`: Bridge Rust `Vec<String>` to C++ `std::vector<std::string>`
  - `make_grammar_matcher()`: Wraps constructor with `std::nullopt` (not directly bindable)
  - `matcher_fill_next_token_bitmask()`, `apply_token_bitmask_inplace_cpu()`: Wrap raw pointer DLTensor operations

### Code Style

- **Edition 2024**: Uses Rust 2024 edition features
- **rustfmt.toml**: Strict formatting with `fn_params_layout = "Vertical"`, `max_width = 80`, `use_small_heuristics = "Off"`
- Import grouping: `StdExternalCrate` with crate-level granularity

## Developer Workflows

### Building

```bash
cargo build --release
```

The build process is slow (compiles C++ library) - `build/` directory contains CMake artifacts.

### Running Examples

```bash
# Basic grammar usage
cargo run --example basic

# DLTensor/bitmask operations
cargo run --example dlpack_matcher
```

### Key Dependencies

- `autocxx` 0.30.0: C++ FFI code generation
- `cxx` 1.0.184: Low-level C++ interop
- `cmake` 0.1.54 (build): CMake integration

## Critical Patterns

### Working with String Vectors

Rust strings must be converted to `std::vector<std::string>` via `cxx_utils` helpers:

```rust
let mut encoded_vocab = cxx_utils::new_string_vector();
let mut vpin = encoded_vocab.pin_mut();
cxx_utils::string_vec_reserve(vpin.as_mut(), vocab.len());
for item in &vocab {
    unsafe {
        cxx_utils::string_vec_push_bytes(
            vpin.as_mut(),
            item.as_bytes().as_ptr() as *const i8,
            item.as_bytes().len(),
        );
    }
}
```

### DLTensor Setup

Manual construction required for CPU tensors:

```rust
let mut bitmask = xgrammar::DLTensor {
    data: storage.as_mut_ptr() as *mut core::ffi::c_void,
    device: xgrammar::DLDevice {
        device_type: xgrammar::DLDeviceType::kDLCPU,
        device_id: 0,
    },
    ndim: 1,
    dtype: GetBitmaskDLType(),
    shape: &mut bm_shape as *mut i64,
    strides: &mut bm_stride as *mut i64,
    byte_offset: 0,
};
```

### Using XGrammar Types

Objects from C++ must be moved to the heap with `.within_box()`:

```rust
let grammar = xgrammar::Grammar::BuiltinJSONGrammar().within_box();
let mut compiler = GrammarCompiler::new(&tok, ...).within_box();
```

## File Organization

- `src/lib.rs`: FFI bindings definition (minimal, mostly macro invocation)
- `src/cxx_utils.h`: C++ helper layer for autocxx compatibility
- `build.rs`: Build orchestration (CMake + autocxx)
- `examples/`: Usage demonstrations (`basic.rs`, `dlpack_matcher.rs`)
- `external/xgrammar/`: Git submodule of upstream C++ library

## Common Pitfalls

1. **Don't edit generated code**: Autocxx generates code in `target/*/autocxx-build-dir/` - modify `src/lib.rs` instead
2. **Unsafe required for raw pointers**: DLTensor operations need `unsafe` blocks
3. **Pin semantics**: Use `.pin_mut()` when passing references to C++ mutating functions
4. **Build script changes**: Modifying `build.rs` or C++ headers requires `cargo clean`

## Upstream Context

The C++ library is from https://github.com/mlc-ai/xgrammar - a structured generation engine adopted by vLLM, SGLang, TensorRT-LLM, and MLC-LLM. This Rust wrapper enables zero-overhead constrained decoding for Rust-based LLM inference systems.
