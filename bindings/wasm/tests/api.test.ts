// Runs against the wasm-bindgen `--target nodejs` output of the `bindings-wasm` build:
//   cargo build -p xgrammar-py --release --target wasm32-unknown-unknown --no-default-features --features bindings-wasm
//   wasm-bindgen --target nodejs --out-dir target/wasm-bindgen target/wasm32-unknown-unknown/release/xgrammar_rs.wasm
import { expect, test } from "bun:test";

import {
  type CompiledGrammar,
  Grammar,
  GrammarCompiler,
  GrammarMatcher,
  NamedGrammars,
  type TokenBitmask,
  TokenizerInfo,
} from "../../../target/wasm-bindgen/xgrammar_rs.js";

// 40 tokens: token 31 is the sign bit of the first mask word, tokens 32.. fill a second word.
const vocab = [...Array.from({ length: 39 }, (_, i) => `t${i}`), "</s>"];
const stopToken = 39;

const tokenizerInfo = () => new TokenizerInfo(vocab, 0, null, Int32Array.of(stopToken), false);
const compiler = () => new GrammarCompiler(tokenizerInfo(), 1, false, -1n);
const matcher = (compiled: CompiledGrammar, terminateWithoutStopToken = false) =>
  new GrammarMatcher(compiled, null, terminateWithoutStopToken, -1, null);
const ebnfMatcher = (ebnf: string) => matcher(compiler().compile_grammar_from_strings(ebnf, "root"));
const allowed = (mask: TokenBitmask) =>
  vocab.map((_, token) => token).filter((token) => ((mask.words[token >> 5] >>> (token & 31)) & 1) === 1);

test("owned mask packs bit 31 as a signed word", () => {
  const mask = ebnfMatcher('root ::= "t31"').next_token_bitmask();
  expect(mask.words).toHaveLength(2);
  expect(mask.need_apply).toBe(true);
  expect(mask.words[0]).toBeLessThan(0);
  expect(allowed(mask)).toEqual([3, 31]);
});

test("decode loop accepts, rejects and stops", () => {
  const m = ebnfMatcher('root ::= "t1" "t2"');
  expect(m.accept_token(3, false)).toBe(false);
  expect(m.accept_token(1, false)).toBe(true);
  expect(allowed(m.next_token_bitmask())).toEqual([2]);
  expect(m.accept_token(2, false)).toBe(true);
  expect(allowed(m.next_token_bitmask())).toEqual([stopToken]);
  expect(m.accept_stop_token()).toBe(true);
  expect(m.is_terminated()).toBe(true);
  expect(() => m.next_token_bitmask()).toThrow();
});

test("rollback, fork, reset and jump-forward", () => {
  const m = ebnfMatcher('root ::= "t1" "t2t3"');
  expect(m.find_jump_forward_string()).toBe("t1t2t3");
  expect(m.accept_token(1, false)).toBe(true);
  expect(m.find_jump_forward_bytes()).toEqual(new TextEncoder().encode("t2t3"));
  const fork = m.fork();
  m.rollback(1);
  expect(allowed(m.next_token_bitmask())).toEqual([1]);
  expect(allowed(fork.next_token_bitmask())).toEqual([2]);
  fork.reset();
  expect(fork.next_token_bitmask()).toEqual(m.next_token_bitmask());
  expect(m.tokenizer_info().vocab_size()).toBe(vocab.length);
});

test("Lark named grammars and captures", () => {
  const named = new NamedGrammars();
  named.add_grammar("first", Grammar.from_ebnf('root ::= "t1"', "root"));
  named.add_lark("second", 'start: v\nv[capture]: "t" /[0-9]/');
  const m = matcher(compiler().compile_lark("start: @first @second", named), true);
  expect(m.accept_string("t1t7", false)).toBe(true);
  expect(m.is_completed()).toBe(true);
  expect(m.capture_values(false)).toEqual([{ name: "v", value: [...new TextEncoder().encode("t7")] }]);
  expect(Grammar.from_lark_with_tokenizer("start: @first", tokenizerInfo(), named).to_string()).not.toBe("");
  expect(() => Grammar.from_lark("start: @missing", new NamedGrammars())).toThrow(/missing/);
});

test("class arguments must be instances of the declared class", () => {
  const grammar = Grammar.from_ebnf('root ::= "a"', "root");
  expect(() => new GrammarCompiler(grammar as unknown as TokenizerInfo, 1, false, -1n)).toThrow(/TokenizerInfo/);
  expect(() => matcher(tokenizerInfo() as unknown as CompiledGrammar)).toThrow(/CompiledGrammar/);
});
