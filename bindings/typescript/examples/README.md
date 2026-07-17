# TypeScript examples

Build the NAPI addon first:

```bash
cargo tools build typescript --targets host
```

Then:

```bash
cargo tools example typescript json-grammar
# or:
pnpm exec tsx examples/jsonGrammar.ts
```
