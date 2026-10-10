//! Errors from [`GrammarCompiler`](super::GrammarCompiler).

use crate::{
    converter::{LarkError, RegexError, SchemaError, StructuralTagError},
    functor::FunctorError,
    parser::EbnfError,
};

/// An error from converting or compiling a grammar.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompileError {
    /// The EBNF grammar is invalid.
    #[error(transparent)]
    Ebnf(#[from] EbnfError),
    /// The JSON schema is invalid.
    #[error(transparent)]
    Schema(#[from] SchemaError),
    /// The regex is invalid.
    #[error(transparent)]
    Regex(#[from] RegexError),
    /// The structural tag is invalid.
    #[error(transparent)]
    StructuralTag(#[from] StructuralTagError),
    /// A grammar pass rejected the grammar.
    #[error(transparent)]
    Functor(#[from] FunctorError),
    /// The Lark grammar is invalid.
    #[error(transparent)]
    Lark(#[from] LarkError),
    /// A `Token(...)`/`ExcludeToken(...)`/`TokenTagDispatch` id is outside the vocabulary.
    #[error("Token id {token_id} in the grammar is out of the vocabulary range [0, {vocab_size})")]
    TokenIdOutOfRange {
        /// The offending token id.
        token_id: i32,
        /// The vocabulary size.
        vocab_size: i32,
    },
}
