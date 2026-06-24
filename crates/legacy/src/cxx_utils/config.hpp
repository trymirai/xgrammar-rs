#ifndef XGRAMMAR_RS_CXX_UTILS_CONFIG_H_
#define XGRAMMAR_RS_CXX_UTILS_CONFIG_H_

#include <memory>
#include <string>

#include "xgrammar/grammar.h"

#include "common.hpp"

namespace cxx_utils {

inline std::unique_ptr<std::string> GetSerializationVersion() {
  return make_unique(xgrammar::GetSerializationVersion());
}

} // namespace cxx_utils

#endif // XGRAMMAR_RS_CXX_UTILS_CONFIG_H_
