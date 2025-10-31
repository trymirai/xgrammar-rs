#pragma once

#include <string>
#include "../../external/xgrammar/cpp/testing.h"

namespace cxx_utils {

inline std::string qwen_xml_tool_calling_to_ebnf(const std::string& schema) {
    return xgrammar::_QwenXMLToolCallingToEBNF(schema);
}

}  // namespace cxx_utils

