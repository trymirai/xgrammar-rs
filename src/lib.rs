#![allow(unsafe_op_in_unsafe_fn)]
// Suppress warnings from auto-generated FFI code
#![allow(unused_imports)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::too_many_arguments)]

#[cxx::bridge()]
mod ffi {
    // `#[safe_shared_extern]` is an extension to `cxx`

    /// DLPack data type descriptor
    #[safe_shared_extern]
    #[derive(Clone, Debug, Hash)]
    pub struct DLDataType {
        code: u8,
        bits: u8,
        lanes: u16,
    }

    #[safe_shared_extern]
    #[derive(Clone, Debug, Hash)]
    pub struct DLDevice {
        pub device_type: DLDeviceType,
        pub device_id: i32,
    }

    /// Not a Rust enum, but a C++ enum, i.e., can hold any value that
    /// fits into `i32`. The Rust enum is defined in dlpack.rs.
    #[derive(PartialEq, Eq, Clone, Debug, Hash)]
    #[repr(u32)]
    pub enum DLDataTypeCode {
        kDLInt = 0,
        kDLUInt = 1,
        kDLFloat = 2,
        kDLOpaqueHandle = 3,
        kDLBfloat = 4,
        kDLComplex = 5,
        kDLBool = 6,
    }

    /// Not a Rust enum, but a C++ enum, i.e., can hold any value that
    /// fits into `i32`. The Rust enum is defined in dlpack.rs.
    #[derive(Clone, Debug, Hash)]
    #[repr(i32)]
    pub enum DLDeviceType {
        kDLCPU = 1,
        kDLCUDA = 2,
        kDLCUDAHost = 3,
        kDLOpenCL = 4,
        kDLVulkan = 7,
        kDLMetal = 8,
        kDLVPI = 9,
        kDLROCM = 10,
        kDLROCMHost = 11,
        kDLExtDev = 12,
        kDLCUDAManaged = 13,
        kDLOneAPI = 14,
        kDLWebGPU = 15,
        kDLHexagon = 16,
        kDLMAIA = 17,
    }

    extern "C++" {
        include!("dlpack/dlpack.h");

        // Shared enums
        pub type DLDataTypeCode;
        pub type DLDeviceType;

        // Opaque types
        pub type DLManagedTensor;
    }

    // The original `DLTensor` is incompatible with `#[safe_shared_extern]`
    // on wasm32 because `void *data` is too small and implicit padding
    // is added.
    // Instead, using this wrapper declared in `cxx_utils`.
    #[namespace = "cxx_utils"]
    #[safe_shared_extern]
    #[derive(Clone, Debug, Hash)]
    pub struct DLTensor_Rust {
        pub data: *mut c_void,
        pub _unused: *mut c_void,
        pub device: DLDevice,
        pub ndim: i32,
        pub dtype: DLDataType,
        pub shape: *mut i64,
        pub strides: *mut i64,
        pub byte_offset: u64,
    }

    /// Not a Rust enum, but a C++ enum, i.e., can hold any value that
    /// fits into `i32`. The Rust enum is defined in tokenizer_info.rs.
    #[namespace = "xgrammar"]
    #[derive(Clone, Debug, Hash)]
    #[repr(i32)]
    pub enum VocabType {
        RAW = 0,
        BYTE_FALLBACK = 1,
        BYTE_LEVEL = 2,
    }

    // This block is unsafe because some functions in it are declared safe.
    #[namespace = "xgrammar"]
    unsafe extern "C++" {
        include!("xgrammar/xgrammar.h");

        // Shared enums
        pub type VocabType;

        // Opaque types

        pub type CompiledGrammar;
        pub fn MemorySizeBytes(self: &CompiledGrammar) -> usize;

        pub type GrammarCompiler;
        pub fn ClearCache(self: Pin<&mut GrammarCompiler>);
        pub fn GetCacheSizeBytes(self: &GrammarCompiler) -> i64;
        pub fn CacheLimitBytes(self: &GrammarCompiler) -> i64;

        pub type StructuralTagItem;

        pub type Grammar;

        pub type GrammarMatcher;
        pub fn AcceptToken(
            self: Pin<&mut GrammarMatcher>,
            token_id: i32,
            debug_print: bool,
        ) -> bool;
        pub fn AcceptString(
            self: Pin<&mut GrammarMatcher>,
            input_str: &CxxString,
            debug_print: bool,
        ) -> bool;
        pub fn Rollback(
            self: Pin<&mut GrammarMatcher>,
            num_tokens: i32,
        );
        pub fn IsTerminated(self: &GrammarMatcher) -> bool;
        pub fn Reset(self: Pin<&mut GrammarMatcher>);

        pub type BatchGrammarMatcher;

        pub type TokenizerInfo;
        pub fn GetVocabType(self: &TokenizerInfo) -> VocabType;
        pub fn GetAddPrefixSpace(self: &TokenizerInfo) -> bool;
        pub fn GetVocabSize(self: &TokenizerInfo) -> i32;
        pub fn GetDecodedVocab(self: &TokenizerInfo) -> &CxxVector<CxxString>;
        pub fn GetStopTokenIds(self: &TokenizerInfo) -> &CxxVector<i32>;
        pub fn GetSpecialTokenIds(self: &TokenizerInfo) -> &CxxVector<i32>;

        /// Set the maximum allowed recursion depth. The depth is shared per process.
        /// This method is thread-safe.
        ///
        /// Parameters
        /// ----------
        /// max_recursion_depth : int
        ///     The maximum allowed recursion depth.
        pub fn SetMaxRecursionDepth(max_recursion_depth: i32);

        /// Get the maximum allowed recursion depth. The depth is shared per process.
        ///
        /// The maximum recursion depth is determined in the following order:
        ///
        /// 1. Manually set via :py:func:`set_max_recursion_depth`
        /// 2. `XGRAMMAR_MAX_RECURSION_DEPTH` environment variable (if set and is a valid integer <= 1,000,000)
        /// 3. Default value of 10,000
        ///
        /// Returns
        /// -------
        /// max_recursion_depth : int
        ///     The maximum allowed recursion depth.
        pub fn GetMaxRecursionDepth() -> i32;

        pub fn GetBitmaskSize(vocab_size: i32) -> i32;

        pub fn GetBitmaskDLType() -> DLDataType;
    }

    // This block is unsafe because some functions in it are declared safe.
    #[namespace = "cxx_utils"]
    unsafe extern "C++" {
        include!("cxx_utils.hpp");

        pub type c_void;

        // cxx_utils/string_vec.hpp

        pub fn new_string_vector() -> UniquePtr<CxxVector<CxxString>>;

        pub fn string_vec_reserve(
            vec: Pin<&mut CxxVector<CxxString>>,
            n: usize,
        );

        pub unsafe fn string_vec_push_bytes(
            vec: Pin<&mut CxxVector<CxxString>>,
            ptr: *const c_char,
            len: usize,
        );

        // cxx_utils/tokenizer_info.hpp

        pub unsafe fn make_tokenizer_info(
            encoded_vocab: &CxxVector<CxxString>,
            vocab_type: VocabType,
            has_vocab_size: bool,
            vocab_size: i32,
            has_stop_ids: bool,
            stop_token_ids_ptr: *const i32,
            stop_token_ids_len: usize,
            add_prefix_space: bool,
            error_out: *mut CxxString,
        ) -> UniquePtr<TokenizerInfo>;

        pub unsafe fn tokenizer_info_deserialize_json_or_error(
            json_string: &CxxString,
            error_out: *mut CxxString,
        ) -> UniquePtr<TokenizerInfo>;

        pub unsafe fn detect_metadata_from_hf(
            backend_str: &CxxString,
            metadata_out: *mut CxxString,
            error_out: *mut CxxString,
        ) -> bool;

        // cxx_utils/grammar.hpp

        pub unsafe fn grammar_from_json_schema(
            schema: &CxxString,
            any_whitespace: bool,
            has_indent: bool,
            indent: i32,
            has_separators: bool,
            separator_comma: &CxxString,
            separator_colon: &CxxString,
            strict_mode: bool,
            has_max_whitespace_cnt: bool,
            max_whitespace_cnt: i32,
            print_converted_ebnf: bool,
            error_out: *mut CxxString,
        ) -> UniquePtr<Grammar>;

        pub unsafe fn grammar_from_ebnf(
            ebnf_string: &CxxString,
            root_rule_name: &CxxString,
            error_out: *mut CxxString,
        ) -> UniquePtr<Grammar>;

        pub unsafe fn grammar_from_regex(
            regex_string: &CxxString,
            print_converted_ebnf: bool,
            error_out: *mut CxxString,
        ) -> UniquePtr<Grammar>;

        pub unsafe fn grammar_from_structural_tag(
            structural_tag_json: &CxxString,
            error_out: *mut CxxString,
        ) -> UniquePtr<Grammar>;

        pub fn new_grammar_vector() -> UniquePtr<CxxVector<Grammar>>;

        pub fn grammar_vec_reserve(
            vec: Pin<&mut CxxVector<Grammar>>,
            n: usize,
        );

        pub fn grammar_vec_push(
            vec: Pin<&mut CxxVector<Grammar>>,
            g: &Grammar,
        );

        pub unsafe fn grammar_deserialize_json_or_error(
            json_string: &CxxString,
            error_out: *mut CxxString,
        ) -> UniquePtr<Grammar>;

        pub fn grammar_to_string(self_: &Grammar) -> UniquePtr<CxxString>;

        pub fn grammar_builtin_json_grammar() -> UniquePtr<Grammar>;

        pub fn grammar_union(
            grammars: &CxxVector<Grammar>
        ) -> UniquePtr<Grammar>;

        pub fn grammar_concat(
            grammars: &CxxVector<Grammar>
        ) -> UniquePtr<Grammar>;

        pub fn grammar_serialize_json(self_: &Grammar) -> UniquePtr<CxxString>;

        // cxx_utils/compiled_grammar.hpp

        pub unsafe fn compiled_grammar_deserialize_json_or_error(
            json_string: &CxxString,
            tokenizer_info: &TokenizerInfo,
            error_out: *mut CxxString,
        ) -> UniquePtr<CompiledGrammar>;

        pub fn compiled_grammar_get_grammar(
            self_: &CompiledGrammar
        ) -> UniquePtr<Grammar>;

        pub fn compiled_grammar_get_tokenizer_info(
            self_: &CompiledGrammar
        ) -> UniquePtr<TokenizerInfo>;

        pub fn compiled_grammar_serialize_json(
            self_: &CompiledGrammar
        ) -> UniquePtr<CxxString>;

        // cxx_utils/grammar_compiler.hpp

        pub unsafe fn make_grammar_compiler(
            tokenizer_info: &TokenizerInfo,
            max_threads: i32,
            cache_enabled: bool,
            cache_limit_bytes: i64,
            error_out: *mut CxxString,
        ) -> UniquePtr<GrammarCompiler>;

        pub fn tokenizer_info_from_vocab_and_metadata(
            encodec_vocab: &CxxVector<CxxString>,
            metadata: &CxxString,
        ) -> UniquePtr<TokenizerInfo>;

        pub fn tokenizer_info_serialize_json(
            self_: &TokenizerInfo
        ) -> UniquePtr<CxxString>;

        pub fn tokenizer_info_dump_metadata(
            self_: &TokenizerInfo
        ) -> UniquePtr<CxxString>;

        pub unsafe fn compiler_compile_json_schema(
            compiler: Pin<&mut GrammarCompiler>,
            schema: &CxxString,
            any_whitespace: bool,
            has_indent: bool,
            indent: i32,
            has_separators: bool,
            separator_comma: &CxxString,
            separator_colon: &CxxString,
            strict_mode: bool,
            has_max_whitespace_cnt: bool,
            max_whitespace_cnt: i32,
            error_out: *mut CxxString,
        ) -> UniquePtr<CompiledGrammar>;

        pub unsafe fn compiler_compile_builtin_json(
            compiler: Pin<&mut GrammarCompiler>,
            error_out: *mut CxxString,
        ) -> UniquePtr<CompiledGrammar>;

        pub unsafe fn compiler_compile_regex(
            compiler: Pin<&mut GrammarCompiler>,
            regex: &CxxString,
            error_out: *mut CxxString,
        ) -> UniquePtr<CompiledGrammar>;

        pub unsafe fn compiler_compile_structural_tag(
            compiler: Pin<&mut GrammarCompiler>,
            structural_tag_json: &CxxString,
            error_out: *mut CxxString,
        ) -> UniquePtr<CompiledGrammar>;

        pub unsafe fn compiler_compile_grammar_or_error(
            compiler: Pin<&mut GrammarCompiler>,
            grammar: &Grammar,
            error_out: *mut CxxString,
        ) -> UniquePtr<CompiledGrammar>;

        // cxx_utils/matcher.hpp

        pub unsafe fn make_grammar_matcher(
            compiled_grammar: &CompiledGrammar,
            has_override_stop_tokens: bool,
            override_stop_tokens_ptr: *const i32,
            override_stop_tokens_len: usize,
            terminate_without_stop_token: bool,
            max_rollback_tokens: i32,
            error_out: *mut CxxString,
        ) -> UniquePtr<GrammarMatcher>;

        pub unsafe fn make_batch_grammar_matcher(
            max_threads: i32,
            error_out: *mut CxxString,
        ) -> UniquePtr<BatchGrammarMatcher>;

        pub fn grammar_matcher_find_jump_forward_string(
            self_: Pin<&mut GrammarMatcher>
        ) -> UniquePtr<CxxString>;

        pub fn grammar_matcher_debug_print_internal_state(
            self_: &GrammarMatcher
        ) -> UniquePtr<CxxString>;

        pub unsafe fn grammar_matcher_fill_next_token_bitmask(
            self_: Pin<&mut GrammarMatcher>,
            next_token_bitmask_r: *mut DLTensor_Rust,
            next: i32,
            debug_print: bool,
        ) -> bool;

        pub fn new_grammar_matcher_vector()
        -> UniquePtr<CxxVector<GrammarMatcher>>;

        pub fn grammar_matcher_vec_reserve(
            vec: Pin<&mut CxxVector<GrammarMatcher>>,
            n: usize,
        );

        pub fn grammar_matcher_vec_push(
            vec: Pin<&mut CxxVector<GrammarMatcher>>,
            matcher: &GrammarMatcher,
        );

        pub unsafe fn batch_matcher_batch_fill_next_token_bitmask(
            batch_matcher: Pin<&mut BatchGrammarMatcher>,
            matchers: *mut CxxVector<GrammarMatcher>,
            bitmask_r: *mut DLTensor_Rust,
            has_indices: bool,
            indices_ptr: *const i32,
            indices_len: usize,
            debug_print: bool,
        );

        pub unsafe fn batch_accept_token(
            matchers: *mut CxxVector<GrammarMatcher>,
            token_ids_ptr: *const i32,
            token_ids_len: usize,
            debug_print: bool,
        ) -> UniquePtr<CxxVector<u8>>;

        pub unsafe fn batch_accept_string(
            matchers: *mut CxxVector<GrammarMatcher>,
            strings: &CxxVector<CxxString>,
            debug_print: bool,
        ) -> UniquePtr<CxxVector<u8>>;

        pub unsafe fn apply_token_bitmask_inplace_cpu(
            logits_r: *mut DLTensor_Rust,
            bitmask_r: *const DLTensor_Rust,
            vocab_size: i32,
            has_indices: bool,
            indices_ptr: *const i32,
            indices_len: usize,
            error_out: *mut CxxString,
        ) -> bool;

        // cxx_utils/config.hpp

        pub fn GetSerializationVersion() -> UniquePtr<CxxString>;

    }

    // This block is unsafe because some functions in it are declared safe.
    #[namespace = "cxx_utils"]
    unsafe extern "C++" {
        include!("cxx_utils/testing.hpp");

        pub fn json_schema_to_ebnf(
            schema: &CxxString,
            any_whitespace: bool,
            has_indent: bool,
            indent: i32,
            has_separators: bool,
            separator_comma: &CxxString,
            separator_colon: &CxxString,
            strict_mode: bool,
            has_max_whitepsace_cnt: bool,
            max_whitespace_cnt: i32,
        ) -> UniquePtr<CxxString>;

        pub fn ebnf_to_grammar_no_normalization(
            ebnf_string: &CxxString,
            root_rule_name: &CxxString,
        ) -> UniquePtr<Grammar>;

        pub fn qwen_xml_tool_calling_to_ebnf(
            schema: &CxxString
        ) -> UniquePtr<CxxString>;

        pub unsafe fn get_masked_tokens_from_bitmask(
            bitmask_r: *const DLTensor_Rust,
            vocab_size: i32,
            index: i32,
        ) -> UniquePtr<CxxVector<i32>>;

        pub type SingleTokenResult;
        pub unsafe fn is_single_token_bitmask(
            bitmask_r: *const DLTensor_Rust,
            vocab_size: i32,
            index: i32,
        ) -> UniquePtr<SingleTokenResult>;
        pub fn get_is_single(self: &SingleTokenResult) -> bool;
        pub fn get_token_id(self: &SingleTokenResult) -> i32;

        pub unsafe fn regex_to_ebnf(
            regex: &CxxString,
            with_rule_name: bool,
            error_out: *mut CxxString,
        ) -> UniquePtr<CxxString>;

        pub unsafe fn generate_range_regex(
            has_start: bool,
            start: i64,
            has_end: bool,
            end: i64,
            error_out: *mut CxxString,
        ) -> UniquePtr<CxxString>;

        pub unsafe fn generate_float_range_regex(
            has_start: bool,
            start: f64,
            has_end: bool,
            end: f64,
            error_out: *mut CxxString,
        ) -> UniquePtr<CxxString>;

        pub unsafe fn print_grammar_fsms(
            grammar: &Grammar,
            error_out: *mut CxxString,
        ) -> UniquePtr<CxxString>;

        pub unsafe fn traverse_draft_tree(
            retrieve_next_token_r: *const DLTensor_Rust,
            retrieve_next_sibling_r: *const DLTensor_Rust,
            draft_tokens_r: *const DLTensor_Rust,
            matcher: Pin<&mut GrammarMatcher>,
            bitmask_r: *mut DLTensor_Rust,
            error_out: *mut CxxString,
        ) -> bool;
    }
}

// Re-export DLPack types for public use.
//
// Note: these are generated from `dlpack/dlpack.h` via autocxx/bindgen. We
// provide Rust-side docs here to avoid leaking Doxygen markup (e.g. `\brief`)
// into docs.rs.
/// DLPack data type descriptor (`DLDataType`).
pub use ffi::DLDataType;
/// DLPack managed tensor (`DLManagedTensor`) (owns tensor + deleter).
pub use ffi::DLManagedTensor;
/// Opaque type representing C/C++'s `void`
pub use ffi::c_void;
// TODO: doc?
pub use ffi::GetBitmaskDLType as get_bitmask_dltype;
// TODO: doc?
pub use ffi::GetBitmaskSize as get_bitmask_size;
mod compiler;
mod config;
mod dlpack;
mod grammar;
mod matcher;
mod tokenizer_info;
mod utils;

pub mod testing;

pub use compiler::{CompiledGrammar, GrammarCompiler};
pub use config::{
    get_max_recursion_depth, get_serialization_version, set_max_recursion_depth,
};
pub use cxx::UniquePtr as CxxUniquePtr;
pub use dlpack::{DLDataTypeCode, DLDevice, DLDeviceType, DLTensor};
pub use grammar::{Grammar, StructuralTagItem};
pub use matcher::{
    BatchGrammarMatcher, GrammarMatcher, allocate_token_bitmask,
    apply_token_bitmask_inplace_cpu, get_bitmask_shape, reset_token_bitmask,
};
pub use tokenizer_info::{
    HfMetadata, TokenizerInfo, VocabType, detect_metadata_from_hf,
};
