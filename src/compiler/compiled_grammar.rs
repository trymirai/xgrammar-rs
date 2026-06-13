use std::pin::Pin;

use crate::{CxxUniquePtr, DeserializeError, Grammar, TokenizerInfo, ffi};

/// This is the primary object to store compiled grammar.
///
/// A `CompiledGrammar` can be used to construct `GrammarMatcher` to generate token masks
/// efficiently.
///
/// # Notes
///
/// Do not construct this class directly, instead use `GrammarCompiler` to construct the object.
pub struct CompiledGrammar {
    inner: CxxUniquePtr<ffi::CompiledGrammar>,
}

impl CompiledGrammar {
    /// The original grammar.
    pub fn grammar(&self) -> Grammar {
        let inner_ref =
            self.inner.as_ref().expect("CompiledGrammar inner is null");
        Grammar::from_unique_ptr(ffi::compiled_grammar_get_grammar(inner_ref))
    }

    /// The tokenizer info associated with the compiled grammar.
    pub fn tokenizer_info(&self) -> TokenizerInfo {
        let inner_ref =
            self.inner.as_ref().expect("CompiledGrammar inner is null");
        TokenizerInfo::from_unique_ptr(
            ffi::compiled_grammar_get_tokenizer_info(inner_ref),
        )
    }

    /// The approximate memory usage of the compiled grammar in bytes.
    pub fn memory_size_bytes(&self) -> usize {
        trait ToUsize {
            fn to_usize(self) -> usize;
        }

        impl ToUsize for usize {
            fn to_usize(self) -> usize {
                self
            }
        }

        let inner_ref =
            self.inner.as_ref().expect("CompiledGrammar inner is null");
        let sz = inner_ref.MemorySizeBytes().to_usize();
        sz
    }

    /// Serialize the compiled grammar to a JSON string. It will serialize the compiled grammar
    /// without the tokenizer info, since the tokenizer info is shared by multiple compiled
    /// grammars.
    ///
    /// # Notes
    ///
    /// The metadata of the tokenizer info is serialized and will be checked when deserializing.
    ///
    /// # Returns
    ///
    /// The JSON string.
    pub fn serialize_json(&self) -> String {
        let inner_ref =
            self.inner.as_ref().expect("CompiledGrammar inner is null");
        ffi::compiled_grammar_serialize_json(inner_ref).to_string()
    }

    /// Deserialize the compiled grammar from a JSON string and associate it with the specified
    /// tokenizer info.
    ///
    /// # Notes
    ///
    /// This will check the metadata of the tokenizer info matching the serialized metadata in
    /// `json`. If the metadata does not match, an error will be returned.
    ///
    /// # Parameters
    ///
    /// - `json`: The JSON string.
    /// - `tokenizer_info`: The tokenizer info.
    ///
    /// # Returns
    ///
    /// The compiled grammar.
    ///
    /// # Errors
    ///
    /// - When the JSON string is invalid.
    /// - When the JSON string does not follow the serialization format of the grammar, or the
    ///   tokenizer info metadata does not match.
    /// - When the `__VERSION__` field in the JSON string is not the same as the current version.
    pub fn deserialize_json(
        json: &str,
        tokenizer_info: &TokenizerInfo,
    ) -> Result<Self, DeserializeError> {
        cxx::let_cxx_string!(json_cxx = json);
        cxx::let_cxx_string!(error_out_cxx = "");
        let mut error_kind: i32 = 0;
        let unique_ptr = unsafe {
            ffi::compiled_grammar_deserialize_json_or_error(
                &json_cxx,
                tokenizer_info.ffi_ref(),
                &mut error_kind,
                error_out_cxx.as_mut().get_unchecked_mut(),
            )
        };
        if unique_ptr.is_null() {
            return Err(DeserializeError::from_parts(error_kind, error_out_cxx.to_string()));
        }
        Ok(Self {
            inner: unique_ptr,
        })
    }

    pub(crate) fn from_unique_ptr(
        inner: cxx::UniquePtr<ffi::CompiledGrammar>
    ) -> Self {
        Self {
            inner,
        }
    }

    pub(crate) fn ffi_ref(&self) -> &ffi::CompiledGrammar {
        self.inner.as_ref().expect("CompiledGrammar inner is null")
    }
}

impl Drop for CompiledGrammar {
    fn drop(&mut self) {}
}
