#ifndef XGRAMMAR_RS_CXX_UTILS_TOKENIZER_INFO_H_
#define XGRAMMAR_RS_CXX_UTILS_TOKENIZER_INFO_H_

#include <cstdint>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "xgrammar/xgrammar.h"

namespace cxx_utils {

inline xgrammar::TokenizerInfo make_tokenizer_info(
    const std::vector<std::string>& encoded_vocab,
    xgrammar::VocabType vocab_type,
    bool has_vocab_size,
    int32_t vocab_size,
    bool has_stop_ids,
    const int32_t* stop_token_ids_ptr,
    size_t stop_token_ids_len,
    bool add_prefix_space
) {
  std::optional<int> vs = std::nullopt;
  if (has_vocab_size)
    vs = vocab_size;

  std::optional<std::vector<int32_t>> stops = std::nullopt;
  if (has_stop_ids) {
    std::vector<int32_t> tmp;
    tmp.reserve(stop_token_ids_len);
    for (size_t i = 0; i < stop_token_ids_len; ++i) {
      tmp.push_back(stop_token_ids_ptr[i]);
    }
    stops = std::move(tmp);
  }

  return xgrammar::TokenizerInfo(
      encoded_vocab,
      vocab_type,
      vs,
      stops,
      add_prefix_space
  );
}

} // namespace cxx_utils

#endif // XGRAMMAR_RS_CXX_UTILS_TOKENIZER_INFO_H_
