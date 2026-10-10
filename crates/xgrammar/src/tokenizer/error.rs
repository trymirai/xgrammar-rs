//! Errors from building [`TokenizerInfo`](super::TokenizerInfo).

/// An error from constructing tokenizer info.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TokenizerError {
    /// The padded vocabulary size is smaller than the number of real tokens.
    #[error("vocab_size ({vocab_size}) must be at least the number of tokens in the vocab ({num_tokens}).")]
    VocabSizeTooSmall {
        /// The requested vocabulary size.
        vocab_size: i32,
        /// The number of tokens in the encoded vocabulary.
        num_tokens: usize,
    },
    /// The metadata JSON is malformed.
    #[error("{0}")]
    InvalidMetadata(String),
}
