#ifndef XGRAMMAR_RS_CXX_UTILS_H_
#define XGRAMMAR_RS_CXX_UTILS_H_

#include <memory>
#include <optional>
#include <string>
#include <utility>
#include <variant>
#include <vector>

#include "dlpack/dlpack.h"
#include "xgrammar/xgrammar.h"

namespace cxx_utils {

inline std::unique_ptr<std::vector<std::string>> new_string_vector() {
  return std::make_unique<std::vector<std::string>>();
}

inline void string_vec_reserve(std::vector<std::string>& v, size_t n) {
  v.reserve(n);
}

inline void string_vec_push(std::vector<std::string>& v, const std::string& s) {
  v.push_back(s);
}

inline void string_vec_push_bytes(
    std::vector<std::string>& v,
    const char* data,
    size_t len
) {
  v.emplace_back(data, len);
}

} // namespace cxx_utils

namespace cxx_utils {

inline xgrammar::GrammarMatcher make_grammar_matcher(
    const xgrammar::CompiledGrammar& compiled,
    bool terminate_without_stop_token,
    int max_rollback_tokens
) {
  return xgrammar::GrammarMatcher(
      compiled,
      std::nullopt,
      terminate_without_stop_token,
      max_rollback_tokens
  );
}

} // namespace cxx_utils

namespace cxx_utils {

inline bool matcher_fill_next_token_bitmask(
    xgrammar::GrammarMatcher& matcher,
    DLTensor* next_token_bitmask,
    int index,
    bool debug_print
) {
  return matcher.FillNextTokenBitmask(next_token_bitmask, index, debug_print);
}

inline void apply_token_bitmask_inplace_cpu(
    DLTensor* logits,
    const DLTensor& bitmask,
    int vocab_size
) {
  xgrammar::ApplyTokenBitmaskInplaceCPU(
      logits,
      bitmask,
      vocab_size,
      std::nullopt
  );
}

} // namespace cxx_utils

// ==================== Safe helper APIs (exception to status)
// ====================
namespace cxx_utils {

inline std::unique_ptr<xgrammar::GrammarCompiler> make_compiler(
    const xgrammar::TokenizerInfo& tokenizer_info,
    int max_threads,
    bool cache_enabled,
    long long max_memory_bytes
) noexcept {
  try {
    return std::make_unique<xgrammar::GrammarCompiler>(
        tokenizer_info,
        max_threads,
        cache_enabled,
        max_memory_bytes
    );
  } catch (...) {
    return nullptr;
  }
}

inline std::unique_ptr<xgrammar::CompiledGrammar>
compiler_compile_json_schema_safe(
    xgrammar::GrammarCompiler& compiler,
    const std::string& schema,
    bool any_whitespace,
    bool has_indent,
    int indent,
    bool has_separators,
    const std::string& sep1,
    const std::string& sep2,
    bool strict_mode,
    std::string* err
) noexcept {
  try {
    std::optional<int> opt_indent =
        has_indent ? std::optional<int>(indent) : std::nullopt;
    std::optional<std::pair<std::string, std::string>> seps =
        has_separators ? std::optional<std::pair<std::string, std::string>>(
                             std::make_pair(sep1, sep2)
                         )
                       : std::nullopt;
    auto result = compiler.CompileJSONSchema(
        schema,
        any_whitespace,
        opt_indent,
        seps,
        strict_mode
    );
    return std::make_unique<xgrammar::CompiledGrammar>(std::move(result));
  } catch (const std::exception& e) {
    if (err)
      *err = e.what();
    return nullptr;
  } catch (...) {
    if (err)
      *err = "unknown error";
    return nullptr;
  }
}

inline std::unique_ptr<xgrammar::CompiledGrammar>
compiler_compile_builtin_json_safe(
    xgrammar::GrammarCompiler& compiler,
    std::string* err
) noexcept {
  try {
    auto result = compiler.CompileBuiltinJSONGrammar();
    return std::make_unique<xgrammar::CompiledGrammar>(std::move(result));
  } catch (const std::exception& e) {
    if (err)
      *err = e.what();
    return nullptr;
  } catch (...) {
    if (err)
      *err = "unknown error";
    return nullptr;
  }
}

inline std::unique_ptr<xgrammar::CompiledGrammar> compiler_compile_grammar_safe(
    xgrammar::GrammarCompiler& compiler,
    const xgrammar::Grammar& grammar,
    std::string* err
) noexcept {
  try {
    auto result = compiler.CompileGrammar(grammar);
    return std::make_unique<xgrammar::CompiledGrammar>(std::move(result));
  } catch (const std::exception& e) {
    if (err)
      *err = e.what();
    return nullptr;
  } catch (...) {
    if (err)
      *err = "unknown error";
    return nullptr;
  }
}

inline std::unique_ptr<xgrammar::CompiledGrammar> compiler_compile_regex_safe(
    xgrammar::GrammarCompiler& compiler,
    const std::string& regex,
    std::string* err
) noexcept {
  try {
    auto result = compiler.CompileRegex(regex);
    return std::make_unique<xgrammar::CompiledGrammar>(std::move(result));
  } catch (const std::exception& e) {
    if (err)
      *err = e.what();
    return nullptr;
  } catch (...) {
    if (err)
      *err = "unknown error";
    return nullptr;
  }
}

inline void compiler_clear_cache_safe(
    xgrammar::GrammarCompiler& compiler
) noexcept {
  try {
    compiler.ClearCache();
  } catch (...) {
  }
}

inline long long compiler_cache_size_bytes(
    const xgrammar::GrammarCompiler& compiler
) noexcept {
  try {
    return compiler.GetCacheSizeBytes();
  } catch (...) {
    return -1;
  }
}

inline long long compiler_cache_limit_bytes(
    const xgrammar::GrammarCompiler& compiler
) noexcept {
  try {
    return compiler.CacheLimitBytes();
  } catch (...) {
    return -1;
  }
}

inline bool compiled_serialize_json_safe(
    const xgrammar::CompiledGrammar& compiled,
    std::string* out,
    std::string* err
) noexcept {
  try {
    *out = compiled.SerializeJSON();
    return true;
  } catch (const std::exception& e) {
    if (err)
      *err = e.what();
    return false;
  } catch (...) {
    if (err)
      *err = "unknown error";
    return false;
  }
}

inline std::unique_ptr<xgrammar::CompiledGrammar> compiled_deserialize_json_safe(
    const std::string& json,
    const xgrammar::TokenizerInfo& tokenizer_info,
    std::string* err
) noexcept {
  try {
    auto result =
        xgrammar::CompiledGrammar::DeserializeJSON(json, tokenizer_info);
    if (std::holds_alternative<xgrammar::CompiledGrammar>(result)) {
      return std::make_unique<xgrammar::CompiledGrammar>(
          std::get<xgrammar::CompiledGrammar>(result)
      );
    } else {
      if (err)
        *err = "SerializationError";
      return nullptr;
    }
  } catch (const std::exception& e) {
    if (err)
      *err = e.what();
    return nullptr;
  } catch (...) {
    if (err)
      *err = "unknown error";
    return nullptr;
  }
}

} // namespace cxx_utils

#endif // XGRAMMAR_RS_CXX_UTILS_H_
