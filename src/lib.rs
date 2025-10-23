#![allow(unsafe_op_in_unsafe_fn)]

use autocxx::prelude::*;

include_cpp! {
    #include "xgrammar/xgrammar.h"
    #include "dlpack/dlpack.h"
    #include "cxx_utils.h"
    safety!(unsafe_ffi)
    generate!("xgrammar::TokenizerInfo")
    generate!("xgrammar::GrammarCompiler")
    generate!("xgrammar::CompiledGrammar")
    generate!("xgrammar::Grammar")
    generate!("xgrammar::VocabType")
    generate!("xgrammar::GrammarMatcher")
    generate!("xgrammar::GetBitmaskSize")
    generate!("xgrammar::GetBitmaskDLType")
    generate!("xgrammar::ApplyTokenBitmaskInplaceCPU")
    // DLPack core types
    generate_pod!("DLTensor")
    generate!("DLManagedTensor")  // Has function pointer, not POD
    generate_pod!("DLDevice")
    generate_pod!("DLDataType")
    generate_pod!("DLDeviceType")
    generate_pod!("DLDataTypeCode")
    // cxx_utils helpers
    generate!("cxx_utils::new_string_vector")
    generate!("cxx_utils::string_vec_reserve")
    generate!("cxx_utils::string_vec_push")
    generate!("cxx_utils::string_vec_push_bytes")
    generate!("cxx_utils::make_grammar_matcher")
    generate!("cxx_utils::matcher_fill_next_token_bitmask")
    generate!("cxx_utils::apply_token_bitmask_inplace_cpu")
}

pub use ffi::{
    DLDataType, DLDataTypeCode, DLDevice, DLDeviceType, DLManagedTensor,
    DLTensor, xgrammar::*, *,
};
