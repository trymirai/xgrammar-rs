#pragma once

#include <string>
#include <vector>
#include "../../external/xgrammar-0.1.26/cpp/testing.h"

namespace cxx_utils {

inline std::string qwen_xml_tool_calling_to_ebnf(const std::string& schema) {
    return xgrammar::_QwenXMLToolCallingToEBNF(schema);
}

inline std::vector<int32_t> get_masked_tokens_from_bitmask(
    const DLTensor* bitmask,
    int32_t vocab_size,
    int32_t index
) {
    std::vector<int> result;
    xgrammar::_DebugGetMaskedTokensFromBitmask(&result, *bitmask, vocab_size, index);
    std::vector<int32_t> output;
    output.assign(result.begin(), result.end());
    return output;
}

struct SingleTokenResult {
    bool is_single;
    int32_t token_id;
    
    bool get_is_single() const { return is_single; }
    int32_t get_token_id() const { return token_id; }
};

inline SingleTokenResult is_single_token_bitmask(
    const DLTensor* bitmask,
    int32_t vocab_size,
    int32_t index
) {
    auto pair = xgrammar::_IsSingleTokenBitmask(*bitmask, vocab_size, index);
    return SingleTokenResult{pair.first, pair.second};
}

}  // namespace cxx_utils

