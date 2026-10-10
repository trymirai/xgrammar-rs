"""Compile the built-in JSON grammar and accept a sample JSON string."""

import xgrammar as xgr

# Minimal byte-level vocab: one token per byte 0..=255, plus a stop token.
encoded_vocab = [bytes([i]).decode("latin1") for i in range(256)] + ["</s>"]
tokenizer_info = xgr.TokenizerInfo(
    encoded_vocab,
    vocab_type=xgr.VocabType.BYTE_FALLBACK,
    stop_token_ids=[256],
)
compiler = xgr.GrammarCompiler(tokenizer_info)
compiled = compiler.compile_builtin_json_grammar()
matcher = xgr.GrammarMatcher(compiled)

sample = '{"name":"Ada","role":"student"}'
assert matcher.accept_string(sample), f"grammar rejected: {sample}"
print(sample)
print(f"completed={matcher.is_completed()} terminated={matcher.is_terminated()}")
