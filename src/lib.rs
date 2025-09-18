#![allow(unsafe_op_in_unsafe_fn)]

use autocxx::prelude::*;

include_cpp! {
    #include "xgrammar/xgrammar.h"
    #include "dlpack/dlpack.h"
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
}

pub use ffi::*;
// Re-export core types at crate root for ergonomic paths like `xgrammar_rs::Grammar`
pub use ffi::xgrammar::*;
// Re-export DLPack types at crate root
pub use ffi::{DLDataType, DLDevice, DLDeviceType, DLManagedTensor, DLTensor};
