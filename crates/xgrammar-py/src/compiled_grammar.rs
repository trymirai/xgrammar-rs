//! `CompiledGrammar` binding.

use crate::{error::map_deserialize_error, grammar::Grammar, tokenizer_info::TokenizerInfo};

/// A grammar compiled against a tokenizer, ready to drive a matcher.
#[bindings::export(Class)]
#[derive(Debug, Clone)]
pub struct CompiledGrammar {
    inner: xgrammar::compiler::CompiledGrammar,
}

impl From<xgrammar::compiler::CompiledGrammar> for CompiledGrammar {
    fn from(inner: xgrammar::compiler::CompiledGrammar) -> Self {
        Self {
            inner,
        }
    }
}

impl AsRef<xgrammar::compiler::CompiledGrammar> for CompiledGrammar {
    fn as_ref(&self) -> &xgrammar::compiler::CompiledGrammar {
        &self.inner
    }
}

#[bindings::export(Implementation)]
impl CompiledGrammar {
    /// The underlying grammar.
    #[bindings::export(Method)]
    pub fn grammar(&self) -> Grammar {
        Grammar::from(self.inner.grammar().as_ref().clone())
    }

    /// The tokenizer info the grammar was compiled against.
    #[bindings::export(Method)]
    pub fn tokenizer_info(&self) -> TokenizerInfo {
        TokenizerInfo::from(self.inner.tokenizer_info().clone())
    }

    /// Approximate in-memory size of the compiled grammar, in bytes.
    #[bindings::export(Method)]
    pub fn memory_size_bytes(&self) -> u64 {
        self.inner.memory_size_bytes() as u64
    }

    /// Serializes the compiled grammar without embedding the full tokenizer info.
    #[bindings::export(Method)]
    pub fn serialize_json(&self) -> String {
        self.inner.serialize_json()
    }

    /// Deserializes a compiled grammar bound to `tokenizer_info`.
    #[bindings::export(Method(Factory))]
    pub fn deserialize_json(
        json_string: String,
        tokenizer_info: &TokenizerInfo,
    ) -> Result<CompiledGrammar, crate::error::BindingError> {
        xgrammar::compiler::CompiledGrammar::deserialize_json(&json_string, tokenizer_info.as_ref())
            .map(CompiledGrammar::from)
            .map_err(map_deserialize_error)
    }
}
