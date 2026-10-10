# TypeScript examples

Build the NAPI addon first:

```bash
cargo tools build typescript --targets host
```

Then run an example with Bun:

```bash
cargo tools example typescript json-grammar
# or, from bindings/typescript:
bun examples/jsonGrammar.ts
```
