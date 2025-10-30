#![allow(unsafe_op_in_unsafe_fn)]

use autocxx::prelude::*;

include_cpp! {
    #include "xgrammar/xgrammar.h"
    #include "cxx_utils/testing_decl.hpp"
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

    // xgrammar/testing.h
    generate!("xgrammar::_QwenXMLToolCallingToEBNF")

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
        _QwenXMLToolCallingToEBNF as FFI_QwenXMLToolCallingToEBNF,
        CompiledGrammar as FFICompiledGrammar,
        GetBitmaskDLType as FFIGetBitmaskDLType,
        GetBitmaskSize as FFIGetBitmaskSize,
        GetMaxRecursionDepth as FFIGetMaxRecursionDepth,
        GetSerializationVersion as FFIGetSerializationVersion,
        Grammar as FFIGrammar, GrammarCompiler as FFIGrammarCompiler,
        GrammarMatcher as FFIGrammarMatcher,
        SetMaxRecursionDepth as FFISetMaxRecursionDepth,
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

/// Return the serialization version string (e.g., "v5").
pub fn get_serialization_version() -> String {
    FFIGetSerializationVersion().to_string()
}

/// Set the maximum recursion depth used by the parser/matcher.
pub fn set_max_recursion_depth(depth: i32) {
    FFISetMaxRecursionDepth(autocxx::c_int(depth))
}

/// Get the maximum recursion depth used by the parser/matcher.
pub fn get_max_recursion_depth() -> i32 {
    FFIGetMaxRecursionDepth().0
}

/// Convert Qwen XML tool calling schema (JSON string) to an EBNF grammar string.
/// Mirrors Python testing `_qwen_xml_tool_calling_to_ebnf`.
pub fn qwen_xml_tool_calling_to_ebnf(schema_json: &str) -> String {
    cxx::let_cxx_string!(schema_cxx = schema_json);
    FFI_QwenXMLToolCallingToEBNF(&schema_cxx).to_string()
}
