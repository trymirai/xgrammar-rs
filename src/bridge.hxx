#pragma once

#include <memory>
#include <string>
#include <vector>

#include "xgrammar/xgrammar.h"

namespace xgrammar {

// Forward declare PImpl types exist in headers
class TokenizerInfo;
class GrammarCompiler;
class CompiledGrammar;

// Factory functions exposed through cxx::bridge
std::unique_ptr<TokenizerInfo> xg_make_tokenizer_info(
    const std::vector<std::string>& encoded_vocab,
    int vocab_type,
    bool add_prefix_space);

std::unique_ptr<GrammarCompiler> xg_make_compiler(
    const TokenizerInfo& tokenizer,
    int max_threads,
    bool cache_enabled,
    long long max_memory_bytes);

std::unique_ptr<CompiledGrammar> xg_compile_builtin_json(
    GrammarCompiler& compiler);

} // namespace xgrammar
