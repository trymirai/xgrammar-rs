# Python examples

Upstream examples (same source as `xgrammar/examples/`):

```bash
# Hugging Face Transformers + LogitsProcessor
uv run --project bindings/python python bindings/python/examples/hf_transformers/transformers_example.py

# Benchmarks (see examples/benchmark/README.md for deps)
uv run --project bindings/python python bindings/python/examples/benchmark/bench_grammar_compile_mask_gen.py
uv run --project bindings/python python bindings/python/examples/benchmark/bench_apply_token_bitmask_inplace.py
```

Cross-language portable example:

```bash
cargo tools example python json-grammar
# or:
uv run --project bindings/python python bindings/python/examples/json_grammar.py
```
