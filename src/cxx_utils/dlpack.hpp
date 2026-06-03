#ifndef XGRAMMAR_RS_CXX_UTILS_DLPACK_H_
#define XGRAMMAR_RS_CXX_UTILS_DLPACK_H_

#include <cstdint>
#include <memory>

#include "dlpack/dlpack.h"

#include "common.hpp"

namespace cxx_utils {

std::unique_ptr<DLTensor> make_tensor(
    void* data,
    DLDevice device,
    int32_t ndim,
    DLDataType dtype,
    int64_t* shape,
    int64_t* strides,
    uint64_t byte_offset
) {
  return make_unique(
      DLTensor{
          .data = data,
          .device = device,
          .ndim = ndim,
          .dtype = dtype,
          .shape = shape,
          .strides = strides,
          .byte_offset = byte_offset
      }
  );
}

} // namespace cxx_utils

#endif // XGRAMMAR_RS_CXX_UTILS_DLPACK_H_
