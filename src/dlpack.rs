use crate::utils::tie_enum_with_ffi;
use crate::{DLDataType, c_void, ffi};

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

impl ffi::DLTensor {
    /// # Safety
    ///
    /// Pointers must point to valid memory and be consistent with
    /// other fields.
    pub unsafe fn new(
        data: *mut c_void,
        device: DLDevice,
        dim: i32,
        dtype: DLDataType,
        shape: *mut i64,
        strides: *mut i64,
        byte_offset: u64,
    ) -> crate::CxxUniquePtr<Self> {
        // SAFETY: the invariants are up to the caller to uphold
        unsafe {
            ffi::make_tensor(
                data,
                device.into(),
                dim,
                dtype,
                shape,
                strides,
                byte_offset,
            )
        }
    }
}
