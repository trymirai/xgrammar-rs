//! Token bitmasks for grammar-guided generation.
//!
//! A token bitmask is a row-major `[batch, get_bitmask_size(vocab)]` buffer of `i32` words.
//! Bit `i` (word `i / 32`, bit `i % 32`) set means token `i` is **allowed**; cleared bits
//! mark rejected tokens. Matchers fill bitmasks via
//! [`GrammarMatcher::fill_next_token_bitmask`](super::grammar_matcher::GrammarMatcher::fill_next_token_bitmask);
//! inference engines apply them to logits to mask out invalid tokens.
//!
//! These functions operate on the raw buffer directly — DLTensor/tensor wrapping lives in the
//! bindings layer.

/// Bits per bitmask word.
const BITS_PER_WORD: i32 = 32;

/// DLPack-compatible int32 type descriptor for token bitmasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitmaskDlType {
    /// DLDataType code: `kDLInt` = 0.
    pub code: u8,
    /// Bits per element.
    pub bits: u8,
    /// Lanes (always 1).
    pub lanes: u16,
}

/// Returns the DLPack dtype for token bitmasks: signed 32-bit integer, one lane.
#[must_use]
pub fn get_bitmask_dl_type() -> BitmaskDlType {
    BitmaskDlType {
        code: 0,
        bits: 32,
        lanes: 1,
    }
}

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

/// Returns the ids of rejected (zero-bit) tokens in batch entry `index`.
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
/// otherwise `(false, -1)`.
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
/// negative infinity.
///
/// # Panics
/// Panics if `logits` is shorter than `vocab_size`.
pub fn apply_token_bitmask_inplace_cpu(
    logits: &mut [f32],
    bitmask: &[i32],
    vocab_size: i32,
) {
    apply_token_bitmask_inplace_cpu_batch(logits, bitmask, vocab_size, 1, None);
}

/// Applies bitmask rows to a batched float32 logits buffer in place.
///
/// Both `logits` and `bitmask` are row-major. When `indices` is `None`, every batch row
/// `0.batch_size` is masked (batch sizes must match). When `indices` is `Some`, each
/// index selects the same row in both tensors.
///
/// # Panics
/// Panics if shapes are inconsistent with `vocab_size` / `batch_size` / `indices`.
pub fn apply_token_bitmask_inplace_cpu_batch(
    logits: &mut [f32],
    bitmask: &[i32],
    vocab_size: i32,
    batch_size: i32,
    indices: Option<&[i32]>,
) {
    let words = get_bitmask_size(vocab_size) as usize;
    let vocab = vocab_size as usize;
    let batch = batch_size as usize;
    assert!(logits.len() >= batch * vocab, "logits shorter than batch × vocab_size");

    let rows: Vec<usize> = if let Some(idx) = indices {
        idx.iter().map(|&i| i as usize).collect()
    } else {
        assert_eq!(bitmask.len(), batch * words, "when indices is None, bitmask batch must match logits batch");
        (0..batch).collect()
    };

    for &row_idx in &rows {
        assert!(row_idx < batch, "batch index out of range");
        assert!((row_idx + 1) * words <= bitmask.len(), "bitmask row out of range");
        let bm = &bitmask[row_idx * words..(row_idx + 1) * words];
        let logits_row = &mut logits[row_idx * vocab..(row_idx + 1) * vocab];
        for token in 0..vocab_size {
            if !bit_is_set(bm, token) {
                logits_row[token as usize] = f32::NEG_INFINITY;
            }
        }
    }
}
