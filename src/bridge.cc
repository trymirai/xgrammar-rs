#include "bridge.hxx"

#include <xgrammar/tokenizer_info.h>
#include <xgrammar/compiler.h>
#include <xgrammar/grammar.h>

namespace xgrammar {

std::unique_ptr<TokenizerInfo> xg_make_tokenizer_info(
    const std::vector<std::string>& encoded_vocab,
    int vocab_type,
    bool add_prefix_space) {
  return std::make_unique<TokenizerInfo>(
      encoded_vocab,
      static_cast<VocabType>(vocab_type),
      std::nullopt,
      std::nullopt,
      add_prefix_space);
}

std::unique_ptr<GrammarCompiler> xg_make_compiler(
    const TokenizerInfo& tokenizer,
    int max_threads,
    bool cache_enabled,
    long long max_memory_bytes) {
  return std::make_unique<GrammarCompiler>(
      tokenizer,
      max_threads,
      cache_enabled,
      max_memory_bytes);
}

std::unique_ptr<CompiledGrammar> xg_compile_builtin_json(
    GrammarCompiler& compiler) {
  return std::make_unique<CompiledGrammar>(compiler.CompileBuiltinJSONGrammar());
}

} // namespace xgrammar
