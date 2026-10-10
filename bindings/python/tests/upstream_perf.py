"""Times the decode hot path of whichever `xgrammar` this interpreter imports and prints one JSON
line. test_upstream_perf.py runs it with this repository's package and with the official upstream
wheel; both see the same synthetic tokenizer, grammars, and token sequences.
"""

import json
import math
import random
import re
import time
from typing import Dict, List

import xgrammar as xgr

VOCAB_SIZE = 32_000
MIN_SAMPLES = 2_000
SCHEMA = {
    "type": "object",
    "properties": {
        "name": {"type": "string"},
        "age": {"type": "integer"},
        "tags": {"type": "array", "items": {"type": "string"}},
        "address": {
            "type": "object",
            "properties": {"street": {"type": "string"}, "city": {"type": "string"}, "zip": {"type": "string"}},
            "required": ["street", "city", "zip"],
        },
        "active": {"type": "boolean"},
    },
    "required": ["name", "age", "tags", "address", "active"],
}
SCHEMA_INSTANCE = json.dumps(
    {
        "name": "Ada Lovelace",
        "age": 36,
        "tags": ["mathematics", "computing", "poetry", "analysis", "engines"],
        "address": {"street": "Great Marlborough Street", "city": "London", "zip": "W1F 7HW"},
        "active": True,
    }
)
JSON_DOCUMENT = json.dumps(
    {
        "id": 1024,
        "title": "Structured generation benchmark",
        "values": [3.14, -2.5, 1e-3, 42, 0],
        "nested": {"flag": False, "empty": None, "items": [{"key": "alpha"}, {"key": "beta"}, {"key": "gamma"}]},
        "description": "a short sentence with several words to tokenize",
    }
)
WORKLOADS = {
    "builtin_json": (lambda compiler: compiler.compile_builtin_json_grammar(), JSON_DOCUMENT),
    "json_schema": (lambda compiler: compiler.compile_json_schema(json.dumps(SCHEMA)), SCHEMA_INSTANCE),
}


def vocabulary() -> List[str]:
    """Single characters, JSON punctuation, the documents' words, and random words."""
    tokens = [chr(code) for code in range(32, 127)] + ["\n", "\t", '": ', '", "', '{"', '"}', ", ", "true", "false"]
    words = re.findall(r"[A-Za-z]+", JSON_DOCUMENT + SCHEMA_INSTANCE)
    rng = random.Random(0)
    while len(words) < VOCAB_SIZE:
        words.append("".join(rng.choice("abcdefghijklmnopqrstuvwxyz") for _ in range(rng.randint(2, 8))))
    seen = set(tokens)
    for word in words:
        for piece in (word, " " + word, '"' + word, word + '"'):
            if piece not in seen and len(tokens) < VOCAB_SIZE - 1:
                seen.add(piece)
                tokens.append(piece)
    return tokens + ["</s>"]


def tokenize(text: str, vocab: List[str]) -> List[int]:
    """Greedy longest-match segmentation; every segmentation of a valid string is valid."""
    ids = {token: index for index, token in enumerate(vocab)}
    longest = max(map(len, vocab))
    result = []
    position = 0
    while position < len(text):
        for length in range(min(longest, len(text) - position), 0, -1):
            token = ids.get(text[position : position + length])
            if token is not None:
                result.append(token)
                position += length
                break
    return result


def percentile(samples: List[int], fraction: float) -> int:
    ordered = sorted(samples)
    return ordered[max(0, math.ceil(fraction * len(ordered)) - 1)]


def measure(compiled: "xgr.CompiledGrammar", tokens: List[int], vocab_size: int) -> Dict[str, Dict[str, int]]:
    repeats = max(5, math.ceil(MIN_SAMPLES / len(tokens)))
    bitmask = xgr.allocate_token_bitmask(1, vocab_size)
    samples: Dict[str, List[int]] = {"fill_next_token_bitmask": [], "accept_token": []}
    for repeat in range(repeats + 1):  # repeat 0 warms up and is discarded
        matcher = xgr.GrammarMatcher(compiled)
        for token in tokens:
            xgr.reset_token_bitmask(bitmask)
            start = time.perf_counter_ns()
            matcher.fill_next_token_bitmask(bitmask)
            filled = time.perf_counter_ns()
            accepted = matcher.accept_token(token)
            done = time.perf_counter_ns()
            if not accepted:
                raise RuntimeError(f"token {token} was rejected")
            if repeat:
                samples["fill_next_token_bitmask"].append(filled - start)
                samples["accept_token"].append(done - filled)
    return {
        op: {"p50_ns": percentile(values, 0.5), "p99_ns": percentile(values, 0.99)} for op, values in samples.items()
    }


def main() -> None:
    vocab = vocabulary()
    info = xgr.TokenizerInfo(vocab, stop_token_ids=[len(vocab) - 1])
    compiler = xgr.GrammarCompiler(info, cache_enabled=False)
    results = {}
    for name, (compile_grammar, text) in WORKLOADS.items():
        results[name] = measure(compile_grammar(compiler), tokenize(text, vocab), info.vocab_size)
    print(json.dumps({"xgrammar": xgr.__file__, "results": results}))


if __name__ == "__main__":
    main()
