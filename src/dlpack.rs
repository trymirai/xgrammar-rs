use crate::ffi;
use crate::utils::tie_enum_with_ffi;

/// DLPack data type code enum (`DLDataTypeCode`)
#[derive(PartialEq, Eq, Clone, Debug, Hash)]
#[repr(u32)]
#[allow(non_camel_case_types)]
pub enum DLDataTypeCode {
    kDLInt = 0,
    kDLUInt = 1,
    kDLFloat = 2,
    kDLOpaqueHandle = 3,
    kDLBfloat = 4,
    kDLComplex = 5,
    kDLBool = 6,
}

tie_enum_with_ffi!(
    DLDataTypeCode,
    u32,
    kDLInt,
    kDLUInt,
    kDLFloat,
    kDLOpaqueHandle,
    kDLBfloat,
    kDLComplex,
    kDLBool
);

/// DLPack dervie type enum (`DLDeviceType`)
#[derive(Clone, Debug, Hash)]
#[repr(i32)]
#[allow(non_camel_case_types)]
pub enum DLDeviceType {
    kDLCPU = 1,
    kDLCUDA = 2,
    kDLCUDAHost = 3,
    kDLOpenCL = 4,
    kDLVulkan = 7,
    kDLMetal = 8,
    kDLVPI = 9,
    kDLROCM = 10,
    kDLROCMHost = 11,
    kDLExtDev = 12,
    kDLCUDAManaged = 13,
    kDLOneAPI = 14,
    kDLWebGPU = 15,
    kDLHexagon = 16,
    kDLMAIA = 17,
}

tie_enum_with_ffi!(
    DLDeviceType,
    i32,
    kDLCPU,
    kDLCUDA,
    kDLCUDAHost,
    kDLOpenCL,
    kDLVulkan,
    kDLMetal,
    kDLVPI,
    kDLROCM,
    kDLROCMHost,
    kDLExtDev,
    kDLCUDAManaged,
    kDLOneAPI,
    kDLWebGPU,
    kDLHexagon,
    kDLMAIA
);

/// DLPack device descriptor (`DLDevice`)
#[derive(Clone, Debug, Hash)]
pub struct DLDevice {
    pub device_type: DLDeviceType,
    pub device_id: i32,
}

impl From<ffi::DLDevice> for DLDevice {
    fn from(value: ffi::DLDevice) -> Self {
        Self {
            device_type: value.device_type.into(),
            device_id: value.device_id,
        }
    }
}

impl From<DLDevice> for ffi::DLDevice {
    fn from(value: DLDevice) -> Self {
        Self {
            device_type: value.device_type.into(),
            device_id: value.device_id,
        }
    }
}

/// DLPack tensor view (`DLTensor`) (does not own memory).
pub struct DLTensor {
    pub data: *mut ffi::c_void,
    pub device: DLDevice,
    pub ndim: i32,
    pub dtype: ffi::DLDataType,
    pub shape: *mut i64,
    pub strides: *mut i64,
    pub byte_offset: u64,
}

impl DLTensor {
    pub(crate) fn ffi(&self) -> ffi::DLTensor_Rust {
        ffi::DLTensor_Rust {
            data: self.data,
            _unused: 0 as *mut ffi::c_void,
            device: self.device.clone().into(),
            ndim: self.ndim,
            dtype: self.dtype.clone(),
            shape: self.shape,
            strides: self.strides,
            byte_offset: self.byte_offset,
        }
    }
}
