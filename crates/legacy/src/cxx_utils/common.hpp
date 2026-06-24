#ifndef XGRAMMAR_RS_CXX_UTILS_COMMON_H_
#define XGRAMMAR_RS_CXX_UTILS_COMMON_H_

#include <cstdint>
#include <memory>
#include <variant>

#include "xgrammar/exception.h"

namespace cxx_utils {

using c_void = void;

/**
 * Makes a `std::unique_ptr` from rvalue.
 * Unlike `std::make_unique`, the template argument is deduced.
 */
template <class T> inline std::unique_ptr<T> make_unique(T&& value) {
  return std::make_unique<T>(std::move(value));
}

inline int32_t serialization_error_kind(const xgrammar::SerializationError& err) {
  if (std::holds_alternative<xgrammar::DeserializeVersionError>(err)) {
    return 1;
  }
  if (std::holds_alternative<xgrammar::InvalidJSONError>(err)) {
    return 2;
  }
  if (std::holds_alternative<xgrammar::DeserializeFormatError>(err)) {
    return 3;
  }
  return 0;
}

inline int32_t structural_tag_error_kind(const xgrammar::StructuralTagError& err) {
  if (std::holds_alternative<xgrammar::InvalidJSONError>(err)) {
    return 2;
  }
  if (std::holds_alternative<xgrammar::InvalidJSONSchemaError>(err)) {
    return 4;
  }
  if (std::holds_alternative<xgrammar::InvalidStructuralTagError>(err)) {
    return 5;
  }
  return 0;
}

} // namespace cxx_utils

#endif // XGRAMMAR_RS_CXX_UTILS_COMMON_H_
