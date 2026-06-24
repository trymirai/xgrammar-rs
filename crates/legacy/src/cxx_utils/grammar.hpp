#ifndef XGRAMMAR_RS_CXX_UTILS_GRAMMAR_H_
#define XGRAMMAR_RS_CXX_UTILS_GRAMMAR_H_

#include <memory>
#include <cstdint>
#include <exception>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "xgrammar/grammar.h"
#include "xgrammar/tokenizer_info.h"

#include "common.hpp"

namespace cxx_utils {

inline std::unique_ptr<xgrammar::Grammar> grammar_from_json_schema(
    const std::string& schema,
    bool any_whitespace,
    bool has_indent,
    int32_t indent,
    bool has_separators,
    const std::string& separator_comma,
    const std::string& separator_colon,
    bool strict_mode,
    bool has_max_whitespace_cnt,
    int32_t max_whitespace_cnt,
    bool print_converted_ebnf,
    std::string* error_out
) {
  try {
    if (error_out) {
      error_out->clear();
    }

    std::optional<int> indent_opt = std::nullopt;
    if (has_indent) {
      indent_opt = indent;
    }

    std::optional<std::pair<std::string, std::string>> separators_opt =
        std::nullopt;
    if (has_separators) {
      separators_opt = std::make_pair(separator_comma, separator_colon);
    }

    std::optional<int> max_whitespace_cnt_opt = std::nullopt;
    if (has_max_whitespace_cnt) {
      max_whitespace_cnt_opt = static_cast<int>(max_whitespace_cnt);
    }

    xgrammar::Grammar g = xgrammar::Grammar::FromJSONSchema(
        schema,
        any_whitespace,
        indent_opt,
        separators_opt,
        strict_mode,
        max_whitespace_cnt_opt,
        print_converted_ebnf
    );
    if (g.IsNull()) {
      if (error_out) {
        *error_out = g.ToString();
      }
      return nullptr;
    }
    return make_unique(std::move(g));
  } catch (const std::exception& e) {
    if (error_out) {
      *error_out = e.what();
    }
    return nullptr;
  } catch (...) {
    if (error_out) {
      *error_out = "unknown C++ exception";
    }
    return nullptr;
  }
}

inline std::unique_ptr<xgrammar::Grammar> grammar_from_ebnf(
    const std::string& ebnf_string,
    const std::string& root_rule_name,
    std::string* error_out
) {
  try {
    if (error_out) {
      error_out->clear();
    }
    xgrammar::Grammar g =
        xgrammar::Grammar::FromEBNF(ebnf_string, root_rule_name);
    if (g.IsNull()) {
      if (error_out) {
        *error_out = g.ToString();
      }
      return nullptr;
    }
    return make_unique(std::move(g));
  } catch (const std::exception& e) {
    if (error_out) {
      *error_out = e.what();
    }
    return nullptr;
  } catch (...) {
    if (error_out) {
      *error_out = "unknown C++ exception";
    }
    return nullptr;
  }
}

inline std::unique_ptr<xgrammar::Grammar> grammar_from_regex(
    const std::string& regex_string,
    bool print_converted_ebnf,
    std::string* error_out
) {
  try {
    if (error_out) {
      error_out->clear();
    }
    xgrammar::Grammar g =
        xgrammar::Grammar::FromRegex(regex_string, print_converted_ebnf);
    if (g.IsNull()) {
      if (error_out) {
        *error_out = g.ToString();
      }
      return nullptr;
    }
    return make_unique(std::move(g));
  } catch (const std::exception& e) {
    if (error_out) {
      *error_out = e.what();
    }
    return nullptr;
  } catch (...) {
    if (error_out) {
      *error_out = "unknown C++ exception";
    }
    return nullptr;
  }
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
    int32_t* error_kind,
    std::string* error_out
) {
  auto result = xgrammar::Grammar::DeserializeJSON(json_string);
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
  return make_unique(std::get<xgrammar::Grammar>(std::move(result)));
}

inline std::unique_ptr<xgrammar::Grammar> grammar_from_structural_tag(
    const std::string& structural_tag_json,
    const xgrammar::TokenizerInfo* tokenizer_info,
    int32_t* error_kind,
    std::string* error_out
) {
  std::optional<xgrammar::TokenizerInfo> tokenizer_info_opt = std::nullopt;
  if (tokenizer_info) {
    tokenizer_info_opt = *tokenizer_info;
  }
  auto result =
      xgrammar::Grammar::FromStructuralTag(structural_tag_json, tokenizer_info_opt);
  if (std::holds_alternative<xgrammar::StructuralTagError>(result)) {
    const auto& err = std::get<xgrammar::StructuralTagError>(result);
    if (error_out) {
      std::visit([&](const auto& e) { *error_out = e.what(); }, err);
    }
    if (error_kind) {
      *error_kind = structural_tag_error_kind(err);
    }
    return nullptr;
  }
  return make_unique(std::get<xgrammar::Grammar>(std::move(result)));
}

inline std::unique_ptr<std::string> grammar_to_string(
    const xgrammar::Grammar& self
) {
  return make_unique(self.ToString());
}

inline std::unique_ptr<xgrammar::Grammar> grammar_builtin_json_grammar() {
  return make_unique(xgrammar::Grammar::BuiltinJSONGrammar());
}

inline std::unique_ptr<xgrammar::Grammar> grammar_union(
    const std::vector<xgrammar::Grammar>& grammars
) {
  return make_unique(xgrammar::Grammar::Union(grammars));
}

inline std::unique_ptr<xgrammar::Grammar> grammar_concat(
    const std::vector<xgrammar::Grammar>& grammars
) {
  return make_unique(xgrammar::Grammar::Concat(grammars));
}

inline std::unique_ptr<std::string> grammar_serialize_json(
    const xgrammar::Grammar& self
) {
  return make_unique(self.SerializeJSON());
}

} // namespace cxx_utils

#endif // XGRAMMAR_RS_CXX_UTILS_GRAMMAR_H_
