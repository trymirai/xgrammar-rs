//! Tokenizer metadata for grammar-guided generation.
//!
//! [`TokenizerInfo`] contains the vocabulary, the type of the vocabulary, and the information
//! needed for grammar-guided generation.

mod hf_metadata;
mod token_decoder;
mod tokenizer_info;
mod vocab_type;

pub use hf_metadata::{HfMetadata, detect_metadata_from_hf, metadata_to_json};
pub use token_decoder::{decode_token, decode_token_bytes};
pub use tokenizer_info::TokenizerInfo;
pub use vocab_type::{UnknownVocabType, VocabType};
