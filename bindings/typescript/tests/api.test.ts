import { expect, test } from "bun:test";

import {
  CompiledGrammar,
  Grammar,
  GrammarCompiler,
  GrammarMatcher,
  NamedGrammars,
  type TokenBitmask,
  TokenizerInfo,
} from "../src/napi/index.js";

// 40 tokens: token 31 is the sign bit of the first mask word, tokens 32.. fill a second word.
const vocab = [...Array.from({ length: 39 }, (_, i) => `t${i}`), "</s>"];
const stopToken = 39;

const tokenizerInfo = () => new TokenizerInfo(vocab, 0, null, [stopToken], false);
const compiler = () => new GrammarCompiler(tokenizerInfo(), 1, false, -1);
const matcher = (compiled: CompiledGrammar, terminateWithoutStopToken = false) =>
  new GrammarMatcher(compiled, null, terminateWithoutStopToken, -1, null);
const ebnfMatcher = (ebnf: string) => matcher(compiler().compileGrammarFromStrings(ebnf, "root"));
const allowed = (mask: TokenBitmask) =>
  vocab.map((_, token) => token).filter((token) => ((mask.words[token >> 5] >>> (token & 31)) & 1) === 1);

test("owned mask packs bit 31 as a signed word", () => {
  const mask = ebnfMatcher('root ::= "t31"').nextTokenBitmask();
  expect(mask.words).toHaveLength(2);
  expect(mask.needApply).toBe(true);
  expect(mask.words[0]).toBeLessThan(0);
  expect(allowed(mask)).toEqual([3, 31]);
});

test("decode loop accepts, rejects and stops", () => {
  const m = ebnfMatcher('root ::= "t1" "t2"');
  expect(m.acceptToken(3, false)).toBe(false);
  expect(m.acceptToken(1, false)).toBe(true);
  expect(allowed(m.nextTokenBitmask())).toEqual([2]);
  expect(m.acceptToken(2, false)).toBe(true);
  expect(allowed(m.nextTokenBitmask())).toEqual([stopToken]);
  expect(m.acceptStopToken()).toBe(true);
  expect(m.isTerminated()).toBe(true);
  expect(() => m.nextTokenBitmask()).toThrow();
});

test("rollback, fork, reset and jump-forward", () => {
  const m = ebnfMatcher('root ::= "t1" "t2t3"');
  expect(m.findJumpForwardString()).toBe("t1t2t3");
  expect(m.acceptToken(1, false)).toBe(true);
  expect(m.findJumpForwardBytes()).toEqual([...new TextEncoder().encode("t2t3")]);
  const fork = m.fork();
  m.rollback(1);
  expect(allowed(m.nextTokenBitmask())).toEqual([1]);
  expect(allowed(fork.nextTokenBitmask())).toEqual([2]);
  fork.reset();
  expect(fork.nextTokenBitmask()).toEqual(m.nextTokenBitmask());
  expect(m.tokenizerInfo().vocabSize()).toBe(vocab.length);
});

test("Lark named grammars and captures", () => {
  const named = new NamedGrammars();
  named.addGrammar("first", Grammar.from_ebnf('root ::= "t1"', "root"));
  named.addLark("second", 'start: v\nv[capture]: "t" /[0-9]/');
  const m = matcher(compiler().compileLark("start: @first @second", named), true);
  expect(m.acceptString("t1t7", false)).toBe(true);
  expect(m.isCompleted()).toBe(true);
  expect(m.captureValues(false)).toEqual([{ name: "v", value: [...new TextEncoder().encode("t7")] }]);
  expect(Grammar.from_lark_with_tokenizer("start: @first", tokenizerInfo(), named).toString()).not.toBe("");
  expect(() => Grammar.from_lark("start: @missing", new NamedGrammars())).toThrow(/missing/);
});

test("class arguments must be instances of the declared class", () => {
  const grammar = Grammar.from_ebnf('root ::= "a"', "root");
  expect(() => new GrammarCompiler(grammar as unknown as TokenizerInfo, 1, false, -1)).toThrow(/TokenizerInfo/);
  expect(() => matcher(tokenizerInfo() as unknown as CompiledGrammar)).toThrow(/CompiledGrammar/);
});
