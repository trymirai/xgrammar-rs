"""Regression tests for the methods shared by every binding backend (owned masks, captures,
jump-forward, named Lark grammars), checked against the Python facade they must agree with."""

import pytest

import xgrammar as xgr
from xgrammar.base import _core

STOP = "</s>"
# 40 tokens: token 31 is the sign bit of the first mask word, tokens 32.. fill a second word.
VOCAB = [f"t{i}" for i in range(39)] + [STOP]


@pytest.fixture
def info():
    return xgr.TokenizerInfo(VOCAB, stop_token_ids=[len(VOCAB) - 1])


def _matcher(info, grammar):
    return xgr.GrammarMatcher(xgr.GrammarCompiler(info, cache_enabled=False).compile_grammar(grammar))


def _allowed(words, vocab_size):
    return [token for token in range(vocab_size) if words[token // 32] >> (token % 32) & 1]


def test_owned_mask_matches_in_place_fill(info):
    matcher = _matcher(info, 'root ::= ("t31" | "t33") "t1"')
    mask = matcher._handle.next_token_bitmask()
    expected = xgr.allocate_token_bitmask(1, info.vocab_size)
    need_apply = matcher.fill_next_token_bitmask(expected)
    assert mask.words == expected[0].tolist()
    assert mask.need_apply == need_apply is True
    assert len(mask.words) == 2 and mask.words[0] < 0
    assert _allowed(mask.words, info.vocab_size) == [3, 31, 33]  # "t3" prefixes both


def test_decode_loop_accept_reject_and_stop(info):
    matcher = _matcher(info, 'root ::= "t1" "t2"')
    assert not matcher._handle.accept_token(3, False)
    assert matcher._handle.accept_token(1, False)
    assert _allowed(matcher._handle.next_token_bitmask().words, info.vocab_size) == [2]
    assert matcher._handle.accept_token(2, False)
    assert _allowed(matcher._handle.next_token_bitmask().words, info.vocab_size) == [len(VOCAB) - 1]
    assert matcher._handle.accept_stop_token()
    assert matcher.is_terminated()
    with pytest.raises(RuntimeError):
        matcher._handle.next_token_bitmask()


def test_rollback_fork_and_reset(info):
    matcher = _matcher(info, 'root ::= "t1" "t2"')
    assert matcher._handle.accept_token(1, False)
    fork = matcher._handle.fork()
    matcher._handle.rollback(1)
    assert _allowed(matcher._handle.next_token_bitmask().words, info.vocab_size) == [1]
    assert _allowed(fork.next_token_bitmask().words, info.vocab_size) == [2]
    fork.reset()
    assert fork.next_token_bitmask().words == matcher._handle.next_token_bitmask().words


def test_jump_forward_bytes_and_string(info):
    matcher = _matcher(info, 'root ::= "t1t2" "t3"')
    assert matcher._handle.find_jump_forward_bytes() == b"t1t2t3"
    assert matcher._handle.find_jump_forward_string() == matcher.find_jump_forward_string() == "t1t2t3"


def test_tokenizer_info_round_trips(info):
    matcher = _matcher(info, 'root ::= "t1"')
    assert matcher._handle.tokenizer_info().serialize_json() == info._handle.serialize_json()


def test_capture_values_match_facade_captures(info):
    grammar = 'start: outer\nouter[capture="o"]: "t1" inner\ninner[capture="i"]: "t" /[0-9]/'
    matcher = xgr.GrammarMatcher(xgr.GrammarCompiler(info).compile_lark(grammar), terminate_without_stop_token=True)
    assert matcher.accept_string("t1t7")
    captures = matcher._handle.capture_values(False)
    assert [(capture.name, capture.value) for capture in captures] == matcher.get_captures()
    assert [(capture.name, capture.value) for capture in captures] == [("i", b"t7"), ("o", b"t1t7")]


def test_named_grammars_builder_reaches_lark(info):
    named = _core.NamedGrammars()
    named.add_grammar("first", xgr.Grammar.from_ebnf('root ::= "t1"')._handle)
    named.add_lark("second", 'start: "t2"')
    compiled = xgr.GrammarCompiler(info)._handle.compile_lark("start: @first @second", named)
    matcher = _core.GrammarMatcher(compiled, None, False, -1, None)
    assert matcher.accept_string("t1t2", False) and matcher.is_completed()
    grammar = _core.Grammar.from_lark_with_tokenizer("start: @first @second", info._handle, named)
    assert grammar.to_string() == _core.Grammar.from_lark("start: @first @second", named).to_string()


def test_facade_named_grammars_dict_with_tokenizer(info):
    grammar = xgr.Grammar.from_lark(
        "start: @item", tokenizer_info=info, named_grammars={"item": xgr.Grammar.from_lark('start: "t5"')}
    )
    matcher = _matcher(info, grammar)
    assert matcher.accept_string("t5") and matcher.is_completed()
