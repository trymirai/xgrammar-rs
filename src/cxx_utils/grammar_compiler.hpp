#ifndef XGRAMMAR_RS_CXX_UTILS_GRAMMAR_COMPILER_H_
#define XGRAMMAR_RS_CXX_UTILS_GRAMMAR_COMPILER_H_

#include <string>
#include <optional>
#include <utility>
#include <vector>

#include "xgrammar/xgrammar.h"

namespace cxx_utils {

inline xgrammar::CompiledGrammar compiler_compile_json_schema(
    xgrammar::GrammarCompiler& compiler,
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
  return compiler.CompileJSONSchema(
      schema,
      any_whitespace,
      indent_opt,
      sep_opt,
      strict_mode
  );
}

} // namespace cxx_utils

#endif // XGRAMMAR_RS_CXX_UTILS_GRAMMAR_COMPILER_H_
