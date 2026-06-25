//! Tokenizer metadata: vocabulary decoding, stop/special-token detection, and the
//! sorted-vocabulary pseudo-trie used for token masking. Ported from `cpp/tokenizer_info.cc`.
//!
//! One dedicated type per file; re-exported here.

mod hf_metadata;
mod token_decoder;
mod tokenizer_info;
mod vocab_type;

pub use hf_metadata::{HfMetadata, detect_metadata_from_hf, metadata_to_json};
pub use token_decoder::decode_token;
pub use tokenizer_info::TokenizerInfo;
pub use vocab_type::{UnknownVocabType, VocabType};
