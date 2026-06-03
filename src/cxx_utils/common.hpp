#ifndef XGRAMMAR_RS_CXX_UTILS_COMMON_H_
#define XGRAMMAR_RS_CXX_UTILS_COMMON_H_

#include <memory>

namespace cxx_utils {

using c_void = void;

/**
 * Makes a `std::unique_ptr` from rvalue.
 * Unlike `std::make_unique`, the template argument is deduced.
 */
template <class T> inline std::unique_ptr<T> make_unique(T&& value) {
  return std::make_unique<T>(std::move(value));
}

} // namespace cxx_utils

#endif // XGRAMMAR_RS_CXX_UTILS_COMMON_H_
