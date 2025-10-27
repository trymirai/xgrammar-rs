use std::pin::Pin;

use autocxx::prelude::*;

use crate::{FFITokenizerInfo, VocabType, cxx_utils};

pub struct TokenizerInfo {
    inner: Pin<Box<FFITokenizerInfo>>,
}

impl TokenizerInfo {
    pub fn new_from_bytes<I, B>(
        encoded_vocab: I,
        vocab_type: VocabType,
        vocab_size: Option<usize>,
        stop_token_ids: Option<Vec<i32>>,
        add_prefix_space: bool,
    ) -> Self
    where
        I: IntoIterator<Item = B>,
        B: AsRef<[u8]>,
    {
        // Build std::vector<std::string> with helpers
        let mut vec = cxx_utils::new_string_vector();
        {
            let mut pin = vec.pin_mut();
            // If we can, reserve using an upper bound by collecting into a Vec first
            let items: Vec<Vec<u8>> = encoded_vocab
                .into_iter()
                .map(|b| b.as_ref().to_vec())
                .collect();
            cxx_utils::string_vec_reserve(pin.as_mut(), items.len());
            for item in items.iter() {
                let ptr = item.as_ptr() as *const i8;
                let len = item.len();
                unsafe {
                    cxx_utils::string_vec_push_bytes(pin.as_mut(), ptr, len);
                }
            }
        }

        let (has_vocab_size, vocab_size_i32) = match vocab_size {
            Some(v) => (true, v as i32),
            None => (false, 0),
        };
        let (has_stop_ids, stop_ptr, stop_len) = match stop_token_ids {
            Some(ref v) if !v.is_empty() => (true, v.as_ptr(), v.len()),
            _ => (false, std::ptr::null(), 0usize),
        };

        let ffi_obj = unsafe {
            cxx_utils::make_tokenizer_info(
                vec.as_ref().unwrap(),
                vocab_type,
                has_vocab_size,
                vocab_size_i32,
                has_stop_ids,
                stop_ptr,
                stop_len,
                add_prefix_space,
            )
        };

        Self {
            inner: ffi_obj.within_box(),
        }
    }

    pub fn new_from_strings<I, S>(
        encoded_vocab: I,
        vocab_type: VocabType,
        vocab_size: Option<usize>,
        stop_token_ids: Option<Vec<i32>>,
        add_prefix_space: bool,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let bytes = encoded_vocab
            .into_iter()
            .map(|s| s.as_ref().as_bytes().to_vec())
            .collect::<Vec<_>>();
        Self::new_from_bytes(
            bytes,
            vocab_type,
            vocab_size,
            stop_token_ids,
            add_prefix_space,
        )
    }

    pub fn vocab_type(&self) -> VocabType {
        self.inner.GetVocabType()
    }

    pub fn vocab_size(&self) -> autocxx::c_int {
        self.inner.GetVocabSize()
    }

    pub fn add_prefix_space(&self) -> bool {
        self.inner.GetAddPrefixSpace()
    }

    pub fn decoded_vocab_as_bytes(&self) -> Vec<Vec<u8>> {
        let inner = self.inner.as_ref();
        let v = inner.GetDecodedVocab();
        let mut out = Vec::with_capacity(v.len());
        for s in v.iter() {
            // CxxString -> bytes; fall back to lossy UTF-8 if needed
            out.push(s.to_string_lossy().into_owned().into_bytes());
        }
        out
    }

    pub fn stop_token_ids(&self) -> Vec<i32> {
        let inner = self.inner.as_ref();
        let v = inner.GetStopTokenIds();
        v.iter().copied().collect()
    }

    pub fn special_token_ids(&self) -> Vec<i32> {
        let inner = self.inner.as_ref();
        let v = inner.GetSpecialTokenIds();
        v.iter().copied().collect()
    }

    pub fn dump_metadata(&self) -> String {
        self.inner.as_ref().DumpMetadata().to_string()
    }

    pub fn serialize_json(&self) -> String {
        self.inner.as_ref().SerializeJSON().to_string()
    }

    pub fn from_vocab_and_metadata_bytes<I, B>(
        _encoded_vocab: I,
        _metadata: &str,
    ) -> Self
    where
        I: IntoIterator<Item = B>,
        B: AsRef<[u8]>,
    {
        unimplemented!(
            "from_vocab_and_metadata not yet implemented in Rust wrapper"
        );
    }

    pub fn inner(&self) -> &Pin<Box<FFITokenizerInfo>> {
        &self.inner
    }
}
