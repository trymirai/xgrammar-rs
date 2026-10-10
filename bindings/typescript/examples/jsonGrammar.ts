/**
 * Compile the built-in JSON grammar and accept a sample JSON string.
 *
 * Build the NAPI addon first (`cargo tools build typescript --targets host`),
 * then run: `pnpm exec tsx examples/jsonGrammar.ts`
 */
import {
  TokenizerInfo,
  GrammarCompiler,
  GrammarMatcher,
} from "../src/index";

const encodedVocab: string[] = [];
for (let i = 0; i < 256; i++) {
  encodedVocab.push(String.fromCharCode(i));
}
encodedVocab.push("</s>");

const tokenizerInfo = new TokenizerInfo(
  encodedVocab,
  1, // VocabType.ByteFallback
  null,
  [256],
  false
);
const compiler = new GrammarCompiler(tokenizerInfo, 8, true, -1);
const compiled = compiler.compileBuiltinJsonGrammar();
const matcher = new GrammarMatcher(compiled, null, false, -1);

const sample = '{"name":"Ada","role":"student"}';
if (!matcher.acceptString(sample, false)) {
  throw new Error(`grammar rejected: ${sample}`);
}
console.log(sample);
console.log(
  `completed=${matcher.isCompleted()} terminated=${matcher.isTerminated()}`
);
