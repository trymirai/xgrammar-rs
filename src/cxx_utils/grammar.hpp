#ifndef XGRAMMAR_RS_CXX_UTILS_GRAMMAR_H_
#define XGRAMMAR_RS_CXX_UTILS_GRAMMAR_H_

#include <memory>
#include <cstdint>
#include <string>
#include <utility>
#include <vector>

#include "xgrammar/xgrammar.h"

namespace cxx_utils {

// JSON schema constructor with explicit option flags to avoid std::optional
// across FFI
inline xgrammar::Grammar grammar_from_json_schema(
    const std::string& schema,
    bool any_whitespace,
    bool has_indent,
    int32_t indent,
    bool has_separators,
    const std::string& separator_comma,
    const std::string& separator_colon,
    bool strict_mode,
    bool print_converted_ebnf
) {
  std::optional<int> indent_opt = std::nullopt;
  if (has_indent)
    indent_opt = indent;

  std::optional<std::pair<std::string, std::string>> separators_opt =
      std::nullopt;
  if (has_separators)
    separators_opt = std::make_pair(separator_comma, separator_colon);

  return xgrammar::Grammar::FromJSONSchema(
      schema,
      any_whitespace,
      indent_opt,
      separators_opt,
      strict_mode,
      print_converted_ebnf
  );
}

inline std::unique_ptr<std::vector<xgrammar::Grammar>> new_grammar_vector() {
  return std::make_unique<std::vector<xgrammar::Grammar>>();
}

inline void grammar_vec_reserve(std::vector<xgrammar::Grammar>& vec, size_t n) {
  vec.reserve(n);
}

inline void grammar_vec_push(
    std::vector<xgrammar::Grammar>& vec,
    const xgrammar::Grammar& g
) {
  vec.push_back(g);
}

inline std::unique_ptr<xgrammar::Grammar> grammar_deserialize_json_or_error(
    const std::string& json_string,
    std::string* error_out
) {
  auto result = xgrammar::Grammar::DeserializeJSON(json_string);
  if (std::holds_alternative<xgrammar::SerializationError>(result)) {
    if (error_out) {
      const auto& err = std::get<xgrammar::SerializationError>(result);
      std::visit([&](const auto& e) { *error_out = e.what(); }, err);
    }
    return nullptr;
  }
  return std::make_unique<xgrammar::Grammar>(
      std::get<xgrammar::Grammar>(std::move(result))
  );
}

} // namespace cxx_utils

#endif // XGRAMMAR_RS_CXX_UTILS_GRAMMAR_H_
