"""Per-rule FSM storage can differ while preserving the serialized grammar's language."""

import json

import pytest
import torch

import xgrammar as xgr


@pytest.mark.parametrize("kind", ["ebnf", "regex", "schema", "lark"])
@pytest.mark.parametrize("change", ["state", "aux"])
def test_distinct_rule_graph_keeps_masks_and_acceptance(kind, change):
    info = xgr.TokenizerInfo([chr(i) for i in range(128)] + ["</s>"], stop_token_ids=[128])
    compiler = xgr.GrammarCompiler(info, cache_enabled=False)
    if kind == "ebnf":
        compiled = compiler.compile_grammar('root ::= "a" ("a" | "b")*')
        text = "aaba"
    elif kind == "regex":
        compiled = compiler.compile_regex("[ab]+")
        text = "aaba"
    elif kind == "lark":
        compiled = compiler.compile_lark('start: "a" ("a" | "b")*')
        text = "aaba"
    else:
        compiled = compiler.compile_json_schema({"type": "string"})
        text = '"aaba"'
    value = json.loads(compiled.serialize_json())
    graph = value["grammar"]["per_rule_fsms"][0][0][0]
    if change == "state":
        graph["edges"]["indptr_"].append(graph["edges"]["indptr_"][-1])
    else:
        graph["edge_aux_data"].append(0)
    restored = xgr.CompiledGrammar.deserialize_json(json.dumps(value), info)
    expected, actual = xgr.GrammarMatcher(compiled), xgr.GrammarMatcher(restored)
    for token in [*(ord(char) for char in text), 128]:
        expected_mask = xgr.allocate_token_bitmask(1, info.vocab_size)
        actual_mask = xgr.allocate_token_bitmask(1, info.vocab_size)
        assert expected.fill_next_token_bitmask(expected_mask) == actual.fill_next_token_bitmask(actual_mask)
        assert torch.equal(expected_mask, actual_mask)
        assert expected.accept_token(token) == actual.accept_token(token)
        assert expected.is_completed() == actual.is_completed()
        assert expected.is_terminated() == actual.is_terminated()
