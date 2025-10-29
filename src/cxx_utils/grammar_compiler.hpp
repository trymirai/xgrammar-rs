#ifndef XGRAMMAR_RS_CXX_UTILS_GRAMMAR_COMPILER_H_
#define XGRAMMAR_RS_CXX_UTILS_GRAMMAR_COMPILER_H_

#include <string>
#include <optional>
#include <utility>
#include <vector>

#include "xgrammar/xgrammar.h"

namespace cxx_utils {

inline xgrammar::CompiledGrammar compiler_compile_json_schema(
    xgrammar::GrammarCompiler* compiler,
    const std::string& schema,
    bool any_whitespace,
    bool has_indent,
    int indent,
    bool has_separators,
    const std::string& separator_comma,
    const std::string& separator_colon,
    bool strict_mode
) {
  std::optional<int> indent_opt =
      has_indent ? std::optional<int>(indent) : std::nullopt;
  std::optional<std::pair<std::string, std::string>> sep_opt =
      has_separators ? std::optional<std::pair<std::string, std::string>>(
                           std::make_pair(separator_comma, separator_colon)
                       )
                     : std::nullopt;
  return compiler->CompileJSONSchema(
      schema,
      any_whitespace,
      indent_opt,
      sep_opt,
      strict_mode
  );
}

inline xgrammar::CompiledGrammar compiler_compile_builtin_json_grammar(
    xgrammar::GrammarCompiler* compiler
) {
  return compiler->CompileBuiltinJSONGrammar();
}

inline xgrammar::CompiledGrammar compiler_compile_regex(
    xgrammar::GrammarCompiler* compiler,
    const std::string& regex
) {
  return compiler->CompileRegex(regex);
}

inline xgrammar::CompiledGrammar compiler_compile_structural_tag(
    xgrammar::GrammarCompiler* compiler,
    const std::vector<xgrammar::StructuralTagItem>& tags,
    const std::vector<std::string>& triggers
) {
  return compiler->CompileStructuralTag(tags, triggers);
}

inline xgrammar::CompiledGrammar compiler_compile_grammar(
    xgrammar::GrammarCompiler* compiler,
    const xgrammar::Grammar& grammar
) {
  return compiler->CompileGrammar(grammar);
}

inline void compiler_clear_cache(xgrammar::GrammarCompiler* compiler) {
  compiler->ClearCache();
}

inline long long compiler_get_cache_size_bytes(
    const xgrammar::GrammarCompiler* compiler
) {
  return compiler->GetCacheSizeBytes();
}

inline long long compiler_cache_limit_bytes(
    const xgrammar::GrammarCompiler* compiler
) {
  return compiler->CacheLimitBytes();
}

} // namespace cxx_utils

#endif // XGRAMMAR_RS_CXX_UTILS_GRAMMAR_COMPILER_H_
