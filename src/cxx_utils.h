#ifndef XGRAMMAR_RS_CXX_UTILS_H_
#define XGRAMMAR_RS_CXX_UTILS_H_

#include <memory>
#include <string>
#include <vector>

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

#endif  // XGRAMMAR_RS_CXX_UTILS_H_


