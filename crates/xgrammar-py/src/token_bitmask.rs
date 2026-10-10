//! `TokenBitmask` record — an owned snapshot of one next-token mask.

/// The tokens a matcher accepts next, as one bitmask row.
///
/// `words` holds `ceil(vocab_size / 32)` words; token `k` is allowed when bit `k % 32` of word
/// `k / 32` is set. The signed words are raw bit patterns, so a set bit 31 reads as a negative
/// number. Bits at or beyond `vocab_size` carry no meaning.
#[bindings::export(Structure)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenBitmask {
    /// The packed mask words.
    pub words: Vec<i32>,
    /// Whether the mask must be applied, i.e. some token is rejected.
    pub need_apply: bool,
}
