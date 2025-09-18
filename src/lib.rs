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
    // DLPack core types
    generate!("DLTensor")
    generate!("DLManagedTensor")
    generate!("DLDevice")
    generate!("DLDataType")
    generate!("DLDeviceType")
}

pub use ffi::*;
// Re-export core types at crate root for ergonomic paths like `xgrammar_rs::Grammar`
pub use ffi::xgrammar::{CompiledGrammar, Grammar, GrammarCompiler, TokenizerInfo};
// Re-export DLPack types at crate root
pub use ffi::{DLDataType, DLDevice, DLDeviceType, DLManagedTensor, DLTensor};
