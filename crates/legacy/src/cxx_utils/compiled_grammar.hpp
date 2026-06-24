#ifndef XGRAMMAR_RS_CXX_UTILS_COMPILED_GRAMMAR_H_
#define XGRAMMAR_RS_CXX_UTILS_COMPILED_GRAMMAR_H_

#include <memory>
#include <string>
#include <variant>

#include "xgrammar/xgrammar.h"

#include "common.hpp"

namespace cxx_utils {

inline std::unique_ptr<xgrammar::CompiledGrammar>
compiled_grammar_deserialize_json_or_error(
    const std::string& json_string,
    const xgrammar::TokenizerInfo& tokenizer_info,
    int32_t* error_kind,
    std::string* error_out
) {
  auto result =
      xgrammar::CompiledGrammar::DeserializeJSON(json_string, tokenizer_info);
  if (std::holds_alternative<xgrammar::SerializationError>(result)) {
    const auto& err = std::get<xgrammar::SerializationError>(result);
    if (error_out) {
      std::visit([&](const auto& e) { *error_out = e.what(); }, err);
    }
    if (error_kind) {
      *error_kind = serialization_error_kind(err);
    }
    return nullptr;
  }
  return make_unique(std::get<xgrammar::CompiledGrammar>(std::move(result)));
}

inline std::unique_ptr<xgrammar::Grammar> compiled_grammar_get_grammar(
    const xgrammar::CompiledGrammar& self
) {
  return make_unique(self.GetGrammar());
}

inline std::unique_ptr<xgrammar::TokenizerInfo>
compiled_grammar_get_tokenizer_info(const xgrammar::CompiledGrammar& self) {
  return make_unique(self.GetTokenizerInfo());
}

inline std::unique_ptr<std::string> compiled_grammar_serialize_json(
    const xgrammar::CompiledGrammar& self
) {
  return make_unique(self.SerializeJSON());
}

} // namespace cxx_utils

#endif // XGRAMMAR_RS_CXX_UTILS_COMPILED_GRAMMAR_H_
