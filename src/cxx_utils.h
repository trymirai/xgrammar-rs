#ifndef XGRAMMAR_RS_CXX_UTILS_H_
#define XGRAMMAR_RS_CXX_UTILS_H_

#include <memory>
#include <string>
#include <vector>
#include "xgrammar/xgrammar.h"
#include "dlpack/dlpack.h"

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

inline void string_vec_push_bytes(std::vector<std::string>& v, const char* data, size_t len) {
  v.emplace_back(data, len);
}

}  // namespace cxx_utils

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

}

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
  xgrammar::ApplyTokenBitmaskInplaceCPU(logits, bitmask, vocab_size, std::nullopt);
}

}

#endif  // XGRAMMAR_RS_CXX_UTILS_H_


