//! Token-bitmask allocation and CPU application — a port of the bitmask helpers in
//! `cpp/grammar_matcher.cc`.
//!
//! A bitmask is a row-major `[batch, get_bitmask_size(vocab)]` buffer of `i32` words; bit `i`
//! (word `i / 32`, bit `i % 32`) set means token `i` is allowed. These functions operate on
//! the raw buffer directly — the DLTensor/tensor wrapping lives in the bindings layer.

/// Bits per bitmask word.
const BITS_PER_WORD: i32 = 32;

/// The number of `i32` words needed to hold `vocab_size` bits: `ceil(vocab_size / 32)`.
#[must_use]
pub fn get_bitmask_size(vocab_size: i32) -> i32 {
    (vocab_size + BITS_PER_WORD - 1) / BITS_PER_WORD
}

/// Allocates a `batch_size × get_bitmask_size(vocab_size)` bitmask buffer, initialized to
/// all-ones (every token allowed).
#[must_use]
pub fn allocate_token_bitmask(
    batch_size: i32,
    vocab_size: i32,
) -> Vec<i32> {
    vec![-1; (batch_size * get_bitmask_size(vocab_size)) as usize]
}

/// Resets every word to all-ones (every token allowed).
pub fn reset_token_bitmask(bitmask: &mut [i32]) {
    bitmask.fill(-1);
}

/// Whether bit `token` is set (allowed) in `row`.
fn bit_is_set(
    row: &[i32],
    token: i32,
) -> bool {
    let word = (token / BITS_PER_WORD) as usize;
    let offset = token % BITS_PER_WORD;
    (row[word] >> offset) & 1 != 0
}

/// The row of `bitmask` for batch entry `index`.
fn row(
    bitmask: &[i32],
    vocab_size: i32,
    index: i32,
) -> &[i32] {
    let size = get_bitmask_size(vocab_size) as usize;
    let start = index as usize * size;
    &bitmask[start..start + size]
}

/// The ids of the rejected (zero-bit) tokens in batch entry `index` — the C++
/// `_DebugGetMaskedTokensFromBitmask`.
#[must_use]
pub fn get_masked_tokens_from_bitmask(
    bitmask: &[i32],
    vocab_size: i32,
    index: i32,
) -> Vec<i32> {
    let row = row(bitmask, vocab_size, index);
    (0..vocab_size).filter(|&t| !bit_is_set(row, t)).collect()
}

/// If exactly one token is allowed in batch entry `index`, returns `(true, token_id)`;
/// otherwise `(false, -1)` — the C++ `_IsSingleTokenBitmask`.
#[must_use]
pub fn is_single_token_bitmask(
    bitmask: &[i32],
    vocab_size: i32,
    index: i32,
) -> (bool, i32) {
    let row = row(bitmask, vocab_size, index);
    let mut found = -1;
    let mut count = 0;
    for t in 0..vocab_size {
        if bit_is_set(row, t) {
            count += 1;
            if count > 1 {
                return (false, -1);
            }
            found = t;
        }
    }
    if count == 1 {
        (true, found)
    } else {
        (false, -1)
    }
}

/// Applies a single-row bitmask to `logits` in place, setting every rejected token's logit to
/// negative infinity — the CPU `ApplyTokenBitmaskInplaceCPU` for one float32 row.
///
/// # Panics
/// Panics if `logits` is shorter than `vocab_size`.
pub fn apply_token_bitmask_inplace_cpu(
    logits: &mut [f32],
    bitmask: &[i32],
    vocab_size: i32,
) {
    assert!(
        logits.len() >= vocab_size as usize,
        "logits shorter than vocab size"
    );
    for token in 0..vocab_size {
        if !bit_is_set(bitmask, token) {
            logits[token as usize] = f32::NEG_INFINITY;
        }
    }
}
