# xgrammar-rs

[![License](https://img.shields.io/badge/license-apache_2-blue)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/xgrammar)](https://crates.io/crates/xgrammar)
[![Documentation](https://docs.rs/xgrammar/badge.svg)](https://docs.rs/xgrammar)

**Efficient, Flexible and Portable Structured Generation for Rust**

Rust bindings for [XGrammar](https://github.com/mlc-ai/xgrammar) - a library for efficient, flexible, and portable structured generation for Large Language Models (LLMs).

## Overview

XGrammar is an open-source library for efficient, flexible, and portable structured generation.

It leverages constrained decoding to ensure **100% structural correctness** of the output. It supports general context-free grammar to enable a broad range of structures, including **JSON**, **regex**, **custom context-free grammar**, etc.

XGrammar uses careful optimizations to achieve extremely low overhead in structured generation. It has achieved **near-zero overhead** in JSON generation, making it one of the fastest structured generation engines available.

## Features

- ✅ **Grammar-guided generation** with context-free grammars (EBNF)
- ✅ **JSON Schema** support for structured output
- ✅ **Regular expressions** for pattern matching
- ✅ **Function calling** with structural tags
- ✅ **Efficient token masking** for LLM generation
- ✅ **Rollback support** for speculative decoding
- ✅ **Serialization/deserialization** of grammars
- ✅ **HuggingFace tokenizer** integration (optional `hf` feature)

## Installation

> **Note:** The crate name `xgrammar` is already taken on crates.io (v0.2.0). 
> Available alternatives: `xgrammar-bindings`, `xgrammar-sys`, `libxgrammar`

Add this to your `Cargo.toml`:

```toml
[dependencies]
xgrammar = "0.1"  # Note: Change to your chosen crate name
```

For HuggingFace tokenizer support:

```toml
[dependencies]
xgrammar = { version = "0.1", features = ["hf"] }  # Note: Change to your chosen crate name
```

## Quick Start

### JSON Schema Generation

```rust
use xgrammar::{Grammar, GrammarCompiler, GrammarMatcher, TokenizerInfo, VocabType};

// Define your JSON schema
let schema = r#"{
    "type": "object",
    "properties": {
        "name": {"type": "string"},
        "age": {"type": "integer"}
    },
    "required": ["name", "age"]
}"#;

// Create grammar from JSON schema
let grammar = Grammar::from_json_schema(
    schema,
    true,  // any_whitespace
    None,  // indent
    Some((",", ":")),  // separators
    true,  // strict_mode
    false, // print_converted_ebnf
);

// Create tokenizer info (example with empty vocab)
let vocab: Vec<&str> = vec![];
let tokenizer_info = TokenizerInfo::new(&vocab, VocabType::RAW, &None, false);

// Compile grammar
let mut compiler = GrammarCompiler::new(&tokenizer_info, 8, false, -1);
let compiled_grammar = compiler.compile_grammar(&grammar);

// Create matcher
let mut matcher = GrammarMatcher::new(&compiled_grammar, None, true, -1);

// Use the matcher to validate strings
assert!(matcher.accept_string(r#"{"name":"John","age":30}"#, false));
assert!(matcher.is_terminated());
```

### EBNF Grammar

```rust
use xgrammar::Grammar;

let ebnf = r#"
root ::= expression
expression ::= term ("+" term | "-" term)*
term ::= factor ("*" factor | "/" factor)*
factor ::= number | "(" expression ")"
number ::= [0-9]+
"#;

let grammar = Grammar::from_ebnf(ebnf, "root");
```

### Regular Expression

```rust
use xgrammar::Grammar;

let regex = r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}";
let grammar = Grammar::from_regex(regex, false);
```

### With HuggingFace Tokenizers (requires `hf` feature)

```rust
use xgrammar::{Grammar, GrammarCompiler, GrammarMatcher, TokenizerInfo};

// Load tokenizer
let tokenizer = tokenizers::Tokenizer::from_file("tokenizer.json")?;
let tokenizer_info = TokenizerInfo::from_huggingface(&tokenizer, None, None);

// Create and compile grammar
let grammar = Grammar::builtin_json_grammar();
let mut compiler = GrammarCompiler::new(&tokenizer_info, 8, false, -1);
let compiled_grammar = compiler.compile_grammar(&grammar);

// Create matcher and use for token-level generation
let mut matcher = GrammarMatcher::new(&compiled_grammar, None, true, -1);

// Fill next token bitmask
use xgrammar::{allocate_token_bitmask, create_bitmask_dltensor};
let mut bitmask_data = allocate_token_bitmask(1, tokenizer_info.vocab_size());
// ... use with LLM generation
```

## API Documentation

For detailed API documentation, visit [docs.rs/xgrammar](https://docs.rs/xgrammar).

### Main Types

- **`Grammar`** - Represents a context-free grammar
- **`GrammarCompiler`** - Compiles grammars with tokenizer info
- **`CompiledGrammar`** - Compiled grammar ready for matching
- **`GrammarMatcher`** - Stateful matcher for grammar-guided generation
- **`TokenizerInfo`** - Tokenizer vocabulary and metadata
- **`StructuralTagItem`** - For function calling and structured tags

## Development

### Building

```bash
cargo build
```

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with HuggingFace feature
HF_TOKEN=your_token cargo test --features hf
```

### Requirements

- Rust 1.70 or later
- CMake 3.18 or later (for building C++ components)
- C++20 compatible compiler

## Related Projects

- [XGrammar](https://github.com/mlc-ai/xgrammar) - Original Python/C++ implementation
- [vLLM](https://github.com/vllm-project/vllm) - Uses XGrammar for structured generation
- [SGLang](https://github.com/sgl-project/sglang) - Uses XGrammar for structured generation
- [TensorRT-LLM](https://github.com/NVIDIA/TensorRT-LLM) - Uses XGrammar for structured generation
- [MLC-LLM](https://github.com/mlc-ai/mlc-llm) - Uses XGrammar for structured generation

## Citation

If you find XGrammar useful in your research, please consider citing:

```bibtex
@article{dong2024xgrammar,
  title={Xgrammar: Flexible and efficient structured generation engine for large language models},
  author={Dong, Yixin and Ruan, Charlie F and Cai, Yaxing and Lai, Ruihang and Xu, Ziyi and Zhao, Yilong and Chen, Tianqi},
  journal={Proceedings of Machine Learning and Systems 7},
  year={2024}
}
```

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

This Rust binding is built on top of the excellent [XGrammar](https://github.com/mlc-ai/xgrammar) library developed by the MLC team.
