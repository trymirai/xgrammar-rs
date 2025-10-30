#ifndef XGRAMMAR_RS_CXX_UTILS_MATCHER_H_
#define XGRAMMAR_RS_CXX_UTILS_MATCHER_H_

#include <cstddef>
#include <cstdint>
#include <optional>
#include <vector>

#include "xgrammar/xgrammar.h"

namespace cxx_utils {

inline xgrammar::GrammarMatcher make_grammar_matcher(
    const xgrammar::CompiledGrammar& compiled_grammar,
    bool has_override_stop_tokens,
    const int32_t* override_stop_tokens_ptr,
    size_t override_stop_tokens_len,
    bool terminate_without_stop_token,
    int max_rollback_tokens
) {
  if (!has_override_stop_tokens) {
    return xgrammar::GrammarMatcher(
        compiled_grammar,
        std::nullopt,
        terminate_without_stop_token,
        max_rollback_tokens
    );
  }
  std::vector<int> tmp;
  tmp.reserve(override_stop_tokens_len);
  for (size_t i = 0; i < override_stop_tokens_len; ++i) {
    tmp.push_back(static_cast<int>(override_stop_tokens_ptr[i]));
  }
  return xgrammar::GrammarMatcher(
      compiled_grammar,
      std::optional<std::vector<int>>(std::move(tmp)),
      terminate_without_stop_token,
      max_rollback_tokens
  );
}

} // namespace cxx_utils

#endif // XGRAMMAR_RS_CXX_UTILS_MATCHER_H_
