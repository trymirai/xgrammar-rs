use std::pin::Pin;

use autocxx::prelude::*;

use crate::{FFICompiledGrammar, Grammar, TokenizerInfo, cxx_utils};

/// This is the primary object to store compiled grammar.
///
/// A CompiledGrammar can be used to construct GrammarMatcher to generate token masks efficiently.
///
/// Notes
/// -----
/// Do not construct this class directly, instead use GrammarCompiler to construct the object.
pub struct CompiledGrammar {
    inner: Pin<Box<FFICompiledGrammar>>,
}

impl CompiledGrammar {
    /// The original grammar.
    pub fn grammar(&self) -> Grammar {
        Grammar::from_pinned_ffi(self.inner.GetGrammar().within_box())
    }

    /// The tokenizer info associated with the compiled grammar.
    pub fn tokenizer_info(&self) -> TokenizerInfo {
        TokenizerInfo::from_pinned_ffi(
            self.inner.GetTokenizerInfo().within_box(),
        )
    }

    /// The approximate memory usage of the compiled grammar in bytes.
    pub fn memory_size_bytes(&self) -> usize {
        // MemorySizeBytes() returns different types on different platforms:
        // - On some platforms: primitive usize (can cast directly)
        // - On other platforms: autocxx::c_ulong (needs conversion via From trait)
        // We handle both cases by converting to u64 first, then to usize.
        let size = self.inner.MemorySizeBytes();

        #[allow(clippy::useless_conversion)]
        {
            let size_as_u64 =
                u64::try_from(size as u64).unwrap_or_else(|_| size as u64);
            size_as_u64 as usize
        }
    }

    /// Serialize the compiled grammar to a JSON string.
    /// It will serialize the compiled grammar without the tokenizer info,
    /// since the tokenizer info is shared by multiple compiled grammars.
    ///
    /// Notes
    /// -----
    /// The metadata of the tokenizer info is serialized and will be checked when deserializing.
    pub fn serialize_json(&self) -> String {
        self.inner.SerializeJSON().to_string()
    }

    /// Deserialize the compiled grammar from a JSON string and associate it with the specified
    /// tokenizer info.
    ///
    /// Returns
    /// - Ok(CompiledGrammar) on success
    /// - Err(String) if the JSON is invalid, format mismatch, version mismatch, or tokenizer
    ///   metadata does not match. The error string mirrors the C++ exception message.
    pub fn deserialize_json(
        json: &str,
        tokenizer_info: &TokenizerInfo,
    ) -> Result<Self, String> {
        cxx::let_cxx_string!(json_cxx = json);
        cxx::let_cxx_string!(error_out_cxx = "");
        let unique_ptr = unsafe {
            cxx_utils::compiled_grammar_deserialize_json_or_error(
                &json_cxx,
                tokenizer_info.ffi_ref(),
                error_out_cxx.as_mut().get_unchecked_mut(),
            )
        };
        if unique_ptr.is_null() {
            return Err(error_out_cxx.to_string());
        }
        let raw_ptr = unique_ptr.into_raw();
        let ffi_box = unsafe { Box::from_raw(raw_ptr) };
        let ffi_pin = unsafe { Pin::new_unchecked(ffi_box) };
        Ok(Self {
            inner: ffi_pin,
        })
    }

    pub(crate) fn from_pinned_ffi(inner: Pin<Box<FFICompiledGrammar>>) -> Self {
        Self {
            inner,
        }
    }

    pub(crate) fn ffi_ref(&self) -> &FFICompiledGrammar {
        self.inner.as_ref().get_ref()
    }
}
