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

    // cxx_utils/string_vec.hpp
    generate!("cxx_utils::new_string_vector")
    generate!("cxx_utils::string_vec_reserve")
    generate!("cxx_utils::string_vec_push_bytes")

    // cxx_utils/tokenizer_info.hpp
    generate!("cxx_utils::make_tokenizer_info")
    generate!("cxx_utils::tokenizer_info_deserialize_json_or_error")

    // cxx_utils/structural_tag.hpp
    generate!("cxx_utils::new_structural_tag_vector")
    generate!("cxx_utils::structural_tag_vec_reserve")
    generate!("cxx_utils::structural_tag_vec_push")

    // cxx_utils/grammar.hpp
    generate!("cxx_utils::grammar_from_json_schema")
    generate!("cxx_utils::new_grammar_vector")
    generate!("cxx_utils::grammar_vec_reserve")
    generate!("cxx_utils::grammar_vec_push")
    generate!("cxx_utils::grammar_deserialize_json_or_error")

    // cxx_utils/compiled_grammar.hpp
    generate!("cxx_utils::compiled_grammar_deserialize_json_or_error")

    // cxx_utils/grammar_compiler.hpp
    generate!("cxx_utils::compiler_compile_json_schema")
    generate!("cxx_utils::compiler_compile_builtin_json_grammar")
    generate!("cxx_utils::compiler_compile_regex")
    generate!("cxx_utils::compiler_compile_structural_tag")
    generate!("cxx_utils::compiler_compile_grammar")
    generate!("cxx_utils::compiler_clear_cache")
    generate!("cxx_utils::compiler_get_cache_size_bytes")
    generate!("cxx_utils::compiler_cache_limit_bytes")

    // cxx_utils/matcher.hpp
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

mod compiler;
mod grammar;
mod matcher;
mod tokenizer_info;

pub use compiler::{CompiledGrammar, GrammarCompiler};
pub use ffi::xgrammar::VocabType;
pub use grammar::{Grammar, StructuralTagItem};
pub use matcher::GrammarMatcher;
pub use tokenizer_info::TokenizerInfo;
