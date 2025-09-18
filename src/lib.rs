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
    generate!("DLManagedTensor")
    generate_pod!("DLDevice")
    generate_pod!("DLDataType")
    generate!("DLDeviceType")
    // cxx_utils helpers
    generate!("cxx_utils::new_string_vector")
    generate!("cxx_utils::string_vec_reserve")
    generate!("cxx_utils::string_vec_push")
    generate!("cxx_utils::string_vec_push_bytes")
}

pub use ffi::*;
pub use ffi::xgrammar::*;
pub use ffi::{DLDataType, DLDevice, DLDeviceType, DLManagedTensor, DLTensor};

