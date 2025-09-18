#![allow(unsafe_op_in_unsafe_fn)]

use autocxx::prelude::*;

include_cpp! {
    #include "xgrammar/xgrammar.h"
    safety!(unsafe_ffi)
    generate!("xgrammar::TokenizerInfo")
    generate!("xgrammar::GrammarCompiler")
    generate!("xgrammar::CompiledGrammar")
    generate!("xgrammar::Grammar")
}

pub use ffi::*;
