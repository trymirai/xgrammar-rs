#![allow(unsafe_op_in_unsafe_fn)]

use autocxx::prelude::*;

include_cpp! {
    #include "xgrammar/xgrammar.h"
    #include "dlpack/dlpack.h"
    #include "cxx_utils.hpp"
    safety!(unsafe_ffi)
    // xgrammar/compiler.h
    generate!("xgrammar::CompiledGrammar")
    generate!("xgrammar::GrammarCompiler")

    // xgrammar/config.h
    generate!("xgrammar::SetMaxRecursionDepth")
    generate!("xgrammar::GetMaxRecursionDepth")
    generate!("xgrammar::GetSerializationVersion")

    // xgrammar/grammar.h
    generate!("xgrammar::StructuralTagItem")
    generate!("xgrammar::Grammar")

    // xgrammar/matcher.h
    generate!("xgrammar::GetBitmaskSize")
    generate!("xgrammar::GetBitmaskDLType")
    generate!("xgrammar::ApplyTokenBitmaskInplaceCPU")
    generate!("xgrammar::GrammarMatcher")

    // xgrammar/tokenizer_info.h
    generate!("xgrammar::VocabType")
    generate!("xgrammar::TokenizerInfo")

    // cxx_utils helpers
    generate!("cxx_utils::new_string_vector")
    generate!("cxx_utils::string_vec_reserve")
    generate!("cxx_utils::string_vec_push_bytes")
    generate!("cxx_utils::make_tokenizer_info")
    generate!("cxx_utils::make_grammar_matcher")

    // DLPack core types
    generate_pod!("DLTensor")
    generate!("DLManagedTensor")  // Has function pointer, not POD
    generate_pod!("DLDevice")
    generate_pod!("DLDataType")
    generate_pod!("DLDeviceType")
    generate_pod!("DLDataTypeCode")

}

#[allow(unused_imports)]
use ffi::{
    DLDataType, DLDataTypeCode, DLDevice, DLDeviceType, DLManagedTensor,
    DLTensor, cxx_utils,
    xgrammar::{
        CompiledGrammar as FFICompiledGrammar, GetBitmaskDLType,
        GetBitmaskSize, GetMaxRecursionDepth, GetSerializationVersion,
        Grammar as FFIGrammar, GrammarCompiler as FFIGrammarCompiler,
        GrammarMatcher as FFIGrammarMatcher, SetMaxRecursionDepth,
        StructuralTagItem as FFIStructuralTagItem,
        TokenizerInfo as FFITokenizerInfo,
    },
};

mod tokenizer_info;

pub use ffi::xgrammar::VocabType;
pub use tokenizer_info::TokenizerInfo;
