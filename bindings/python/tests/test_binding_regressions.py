"""Regression tests for the Python binding boundary that the upstream suite does not cover:
vocabulary transport, duplicate matcher references in batch calls, and bitmask buffer borrowing."""

import subprocess
import sys

import numpy as np
import pytest
import torch

import xgrammar as xgr

STOP = "</s>"


def _tokenizer_info(vocab):
    return xgr.TokenizerInfo(vocab, stop_token_ids=[len(vocab) - 1])


def _matcher(info, grammar='root ::= "a" "b"'):
    return xgr.GrammarMatcher(xgr.GrammarCompiler(info, cache_enabled=False).compile_grammar(grammar))


# A literal string that looks like a private byte-transport marker must stay a literal string.
COLLIDING_TOKEN = "xgrammar-bytes:41"


def test_string_token_is_kept_literally():
    info = _tokenizer_info([COLLIDING_TOKEN, "b", STOP])
    assert info.decoded_vocab[0] == COLLIDING_TOKEN.encode("utf-8")
    assert not _matcher(info, 'root ::= "A"').accept_token(0)


def test_string_token_is_kept_literally_with_metadata():
    metadata = _tokenizer_info(["a", STOP]).dump_metadata()
    info = xgr.TokenizerInfo.from_vocab_and_metadata([COLLIDING_TOKEN, "b"], metadata)
    assert info.decoded_vocab[0] == COLLIDING_TOKEN.encode("utf-8")


@pytest.mark.parametrize("token", [b"\x00", b"\xff\xfe", b"a\x00b"])
def test_raw_byte_tokens_are_exact(token):
    info = _tokenizer_info(["x", token, STOP])
    assert info.decoded_vocab[1] == token
    metadata = info.dump_metadata()
    assert xgr.TokenizerInfo.from_vocab_and_metadata(["x", token, STOP], metadata).decoded_vocab[1] == token


def test_vocab_rejects_unsupported_token_types():
    with pytest.raises(TypeError):
        _tokenizer_info(["a", 7, STOP])


def _fresh_pair(info):
    return _matcher(info), _matcher(info)


def test_batch_fill_with_duplicate_matcher():
    info = _tokenizer_info(["a", "b", STOP])
    matcher, reference = _fresh_pair(info)
    expected = xgr.allocate_token_bitmask(1, info.vocab_size)
    reference.fill_next_token_bitmask(expected)
    for indices in (None, [1, 0]):
        bitmask = xgr.allocate_token_bitmask(2, info.vocab_size)
        xgr.BatchGrammarMatcher(max_threads=2).batch_fill_next_token_bitmask([matcher, matcher], bitmask, indices)
        assert torch.equal(bitmask[0], expected[0]) and torch.equal(bitmask[1], expected[0])


def test_batch_accept_and_rollback_with_duplicate_matcher():
    info = _tokenizer_info(["a", "b", STOP])
    matcher = _matcher(info)
    assert xgr.BatchGrammarMatcher.batch_accept_token([matcher, matcher], [0, 1]) == [True, True]
    assert matcher.is_completed()
    xgr.BatchGrammarMatcher.batch_rollback([matcher, matcher], [1, 1])
    assert xgr.BatchGrammarMatcher.batch_accept_string([matcher, matcher], ["a", "b"]) == [True, True]
    assert matcher.is_completed()


def _single_token_mask(info):
    return xgr.allocate_token_bitmask(1, info.vocab_size)


@pytest.fixture
def info_and_matcher():
    info = _tokenizer_info(["a", "b", "c", STOP])
    return info, _matcher(info, 'root ::= "a"')


def test_native_numpy_buffer_is_written(info_and_matcher):
    info, matcher = info_and_matcher
    expected = _single_token_mask(info)
    matcher.fill_next_token_bitmask(expected)
    destination = np.zeros(expected.shape, dtype=np.int32)
    matcher.fill_next_token_bitmask(destination)
    np.testing.assert_array_equal(destination, expected.numpy())


def test_torch_buffer_is_written(info_and_matcher):
    info, matcher = info_and_matcher
    destination = torch.zeros(_single_token_mask(info).shape, dtype=torch.int32)
    matcher.fill_next_token_bitmask(destination)
    assert int(destination[0, 0]) & 0b1111 == 0b0001


def test_non_native_byte_order_is_rejected(info_and_matcher):
    info, matcher = info_and_matcher
    swapped = np.zeros(_single_token_mask(info).shape, dtype=np.dtype(np.int32).newbyteorder())
    with pytest.raises(BufferError):
        matcher.fill_next_token_bitmask(swapped)
    assert not swapped.any()


def test_copy_producing_input_is_rejected(info_and_matcher):
    _, matcher = info_and_matcher
    nested = [[np.int32(0)]]
    with pytest.raises(TypeError):
        matcher.fill_next_token_bitmask(nested)
    assert nested == [[np.int32(0)]]


def test_read_only_array_is_rejected(info_and_matcher):
    info, matcher = info_and_matcher
    read_only = np.zeros(_single_token_mask(info).shape, dtype=np.int32)
    read_only.flags.writeable = False
    with pytest.raises(BufferError):
        matcher.fill_next_token_bitmask(read_only)
    assert not read_only.any()


def test_non_contiguous_array_is_rejected():
    info = _tokenizer_info([f"t{i}" for i in range(39)] + [STOP])  # two bitmask words
    matcher = _matcher(info, 'root ::= "t1"')
    strided = np.zeros((1, 2 * _single_token_mask(info).shape[1]), dtype=np.int32)[:, ::2]
    with pytest.raises(RuntimeError):
        matcher.fill_next_token_bitmask(strided)
    assert not strided.any()


def test_object_imitating_a_tensor_is_rejected_safely():
    # Duck-typed tensor attributes with a null pointer must raise, not reach native memory. Runs in
    # a child process so that a crash fails this test instead of the whole session.
    script = """
import torch, xgrammar as xgr
info = xgr.TokenizerInfo(["a", "</s>"], stop_token_ids=[1])
matcher = xgr.GrammarMatcher(xgr.GrammarCompiler(info).compile_grammar('root ::= "a"'))
class FakeTensor:
    is_cpu, dtype, requires_grad, shape = True, torch.int32, False, (1, 1)
    def is_contiguous(self): return True
    def data_ptr(self): return 0
try:
    matcher.fill_next_token_bitmask(FakeTensor())
except TypeError:
    print("TypeError")
"""
    result = subprocess.run([sys.executable, "-c", script], capture_output=True, text=True, timeout=120)
    assert result.returncode == 0, result.stderr[-500:]
    assert result.stdout.strip() == "TypeError"


def _unaligned(shape, dtype, fill):
    """A writable native-endian array whose data starts one byte past an element boundary."""
    count = int(np.prod(shape))
    backing = bytearray(count * np.dtype(dtype).itemsize + 1)
    array = np.ndarray(shape, dtype=dtype, buffer=backing, offset=1)
    array.fill(fill)
    assert array.ctypes.data % np.dtype(dtype).alignment != 0
    return array, backing


@pytest.mark.parametrize("make", ["numpy", "torch"])
def test_unaligned_mask_buffer_is_written(info_and_matcher, make):
    info, matcher = info_and_matcher
    expected = _single_token_mask(info)
    matcher.fill_next_token_bitmask(expected)
    array, backing = _unaligned(expected.shape, np.int32, 0)
    destination = (
        array if make == "numpy" else torch.frombuffer(backing, dtype=torch.int32, offset=1).view(expected.shape)
    )
    matcher.fill_next_token_bitmask(destination)
    np.testing.assert_array_equal(
        np.ndarray(expected.shape, dtype=np.int32, buffer=backing, offset=1), expected.numpy()
    )


@pytest.mark.parametrize("aligned", [True, False])
def test_batch_fill_partial_indices_keep_other_rows(info_and_matcher, aligned):
    info, matcher = info_and_matcher
    expected = _single_token_mask(info)
    matcher.fill_next_token_bitmask(expected)
    sentinel = -7
    shape = (4, expected.shape[1])
    if aligned:
        destination = np.full(shape, sentinel, dtype=np.int32)
    else:
        destination, _ = _unaligned(shape, np.int32, sentinel)
    xgr.BatchGrammarMatcher(max_threads=2).batch_fill_next_token_bitmask(
        [matcher, _matcher(info, 'root ::= "a"')], destination, [3, 1]
    )
    for row in (1, 3):
        np.testing.assert_array_equal(destination[row], expected[0].numpy())
    assert (destination[[0, 2]] == sentinel).all()


@pytest.mark.parametrize("make", ["numpy_aligned", "numpy_unaligned", "torch"])
def test_batch_temperature_partial_indices_keep_other_entries(make):
    info = _tokenizer_info(["a", "b", STOP])
    hot = _matcher(info, 'root ::= r\nr[temperature=1.5] ::= "a"')
    plain = _matcher(info, 'root ::= "a"')
    sentinel = 9.0
    if make == "numpy_aligned":
        temperatures = np.full(4, sentinel, dtype=np.float32)
    elif make == "numpy_unaligned":
        temperatures, _ = _unaligned((4,), np.float32, sentinel)
    else:
        temperatures = torch.full((4,), sentinel, dtype=torch.float32)
    xgr.BatchGrammarMatcher.batch_fill_temperature([hot, plain], temperatures, [2, 0])
    values = np.asarray(temperatures).tolist()
    assert values == [-1.0, sentinel, 1.5, sentinel]
