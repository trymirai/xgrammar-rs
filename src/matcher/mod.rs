mod batch_grammar_matcher;
mod grammar_matcher;

pub use batch_grammar_matcher::BatchGrammarMatcher;
pub use grammar_matcher::GrammarMatcher;

/// Get the shape of the bitmask for next token prediction.
///
/// # Parameters
/// - `batch_size`: The batch size of the bitmask.
/// - `vocab_size`: The size of the vocabulary.
///
/// # Returns
/// A tuple of (batch_size, ceil(vocab_size / 32)).
pub fn get_bitmask_shape(
    batch_size: usize,
    vocab_size: usize,
) -> (usize, usize) {
    (batch_size, (vocab_size + 31) / 32)
}

/// Allocate the bitmask for the next token prediction. The bitmask is an int32 tensor on
/// CPU with shape (batch_size, ceil(vocab_size / 32)).
///
/// The reason why we use int32 instead of uint32 is compatibility with various tensor libraries.
///
/// # Parameters
/// - `batch_size`: The batch size of the bitmask.
/// - `vocab_size`: The size of the vocabulary.
///
/// # Returns
/// A boxed slice containing the bitmask data, initialized to all bits set (no masking).
pub fn allocate_token_bitmask(
    batch_size: usize,
    vocab_size: usize,
) -> Box<[i32]> {
    let (_, bitmask_size) = get_bitmask_shape(batch_size, vocab_size);
    let total_size = batch_size * bitmask_size;
    vec![-1i32; total_size].into_boxed_slice()
}

/// Reset the bitmask to the full mask (all bits set to 1, meaning no tokens are masked).
///
/// # Parameters
/// - `bitmask`: The bitmask to reset. Must be a mutable slice of i32.
pub fn reset_token_bitmask(bitmask: &mut [i32]) {
    bitmask.fill(-1i32);
}
