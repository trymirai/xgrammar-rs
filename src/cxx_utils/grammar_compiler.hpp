#ifndef XGRAMMAR_RS_CXX_UTILS_GRAMMAR_COMPILER_H_
#define XGRAMMAR_RS_CXX_UTILS_GRAMMAR_COMPILER_H_

#include <string>
#include <memory>
#include <optional>
#include <utility>
#include <vector>
#include <cstdio>

#include "xgrammar/xgrammar.h"

namespace cxx_utils {

inline std::unique_ptr<xgrammar::GrammarCompiler> make_grammar_compiler(
    const xgrammar::TokenizerInfo& tokenizer_info,
    int max_threads,
    bool cache_enabled,
    long long cache_limit_bytes
) {
  auto obj = std::make_unique<xgrammar::GrammarCompiler>(
      tokenizer_info,
      max_threads,
      cache_enabled,
      cache_limit_bytes
  );
  return obj;
}

inline std::unique_ptr<xgrammar::CompiledGrammar> compiler_compile_json_schema(
    xgrammar::GrammarCompiler& compiler,
    const std::string& schema,
    bool any_whitespace,
    bool has_indent,
    int indent,
    bool has_separators,
    const std::string& separator_comma,
    const std::string& separator_colon,
    bool strict_mode,
    bool has_max_whitespace_cnt,
    int max_whitespace_cnt
) {
  std::optional<int> indent_opt =
      has_indent ? std::optional<int>(indent) : std::nullopt;
  std::optional<std::pair<std::string, std::string>> sep_opt =
      has_separators ? std::optional<std::pair<std::string, std::string>>(
                           std::make_pair(separator_comma, separator_colon)
                       )
                     : std::nullopt;
  std::optional<int> max_whitespace_cnt_opt =
      has_max_whitespace_cnt ? std::optional<int>(max_whitespace_cnt)
                             : std::nullopt;
  auto result = compiler.CompileJSONSchema(
      schema,
      any_whitespace,
      indent_opt,
      sep_opt,
      strict_mode,
      max_whitespace_cnt_opt
  );
  return std::make_unique<xgrammar::CompiledGrammar>(std::move(result));
}

inline std::unique_ptr<xgrammar::CompiledGrammar> compiler_compile_builtin_json(
    xgrammar::GrammarCompiler& compiler
) {
  auto result = compiler.CompileBuiltinJSONGrammar();
  return std::make_unique<xgrammar::CompiledGrammar>(std::move(result));
}

inline std::unique_ptr<xgrammar::CompiledGrammar> compiler_compile_regex(
    xgrammar::GrammarCompiler& compiler,
    const std::string& regex
) {
  auto result = compiler.CompileRegex(regex);
  return std::make_unique<xgrammar::CompiledGrammar>(std::move(result));
}

inline std::unique_ptr<xgrammar::CompiledGrammar>
compiler_compile_structural_tag(
    xgrammar::GrammarCompiler& compiler,
    const std::string& structural_tag_json
) {
  auto result = compiler.CompileStructuralTag(structural_tag_json);
  return std::make_unique<xgrammar::CompiledGrammar>(std::move(result));
}

// Safe wrapper around xgrammar::GrammarCompiler::CompileGrammar with exception
// capture.
inline std::unique_ptr<xgrammar::CompiledGrammar>
compiler_compile_grammar_or_error(
    xgrammar::GrammarCompiler& compiler,
    const xgrammar::Grammar& grammar,
    std::string* error_out
) {
  try {
    return std::make_unique<xgrammar::CompiledGrammar>(
        compiler.CompileGrammar(grammar)
    );
  } catch (const std::exception& e) {
    if (error_out)
      *error_out = e.what();
    return nullptr;
  } catch (...) {
    if (error_out)
      *error_out = "unknown C++ exception";
    return nullptr;
  }
}

} // namespace cxx_utils

#endif // XGRAMMAR_RS_CXX_UTILS_GRAMMAR_COMPILER_H_
