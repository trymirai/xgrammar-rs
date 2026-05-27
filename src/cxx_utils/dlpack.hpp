#ifndef XGRAMMAR_RS_CXX_UTILS_DLPACK_H_
#define XGRAMMAR_RS_CXX_UTILS_DLPACK_H_

#include <cstdint>

#include "xgrammar/xgrammar.h"

namespace cxx_utils {

/**
 * Mimics dlpack's `DLTensor` but ensures that there's no padding in the layout,
 * including on wasm32.
 */
struct DLTensor_Rust {
  void* data;
  void* _unused;
  DLDevice device;
  int32_t ndim;
  DLDataType dtype;
  int64_t* shape;
  int64_t* strides;
  uint64_t byte_offset;
};

DLTensor_Rust tensor_to_rust_tensor(const DLTensor& tensor) {
  return DLTensor_Rust{
      .data = tensor.data,
      ._unused = 0,
      .device = tensor.device,
      .ndim = tensor.ndim,
      .dtype = tensor.dtype,
      .shape = tensor.shape,
      .strides = tensor.strides,
      .byte_offset = tensor.byte_offset,
  };
}

DLTensor rust_tensor_to_tensor(const DLTensor_Rust& tensor) {
  return DLTensor{
      .data = tensor.data,
      // tensor._unused is unused,
      .device = tensor.device,
      .ndim = tensor.ndim,
      .dtype = tensor.dtype,
      .shape = tensor.shape,
      .strides = tensor.strides,
      .byte_offset = tensor.byte_offset,
  };
}

} // namespace cxx_utils

#endif // XGRAMMAR_RS_CXX_UTILS_DLPACK_H_
