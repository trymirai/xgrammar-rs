use autocxx::c_int;

use crate::ffi::xgrammar::{
    GetMaxRecursionDepth as FFIGetMaxRecursionDepth,
    GetSerializationVersion as FFIGetSerializationVersion,
    SetMaxRecursionDepth as FFISetMaxRecursionDepth,
};

/// Return the serialization version string (e.g., "v5").
pub fn get_serialization_version() -> String {
    FFIGetSerializationVersion().to_string()
}

/// Set the maximum recursion depth used by the parser/matcher.
pub fn set_max_recursion_depth(depth: i32) {
    FFISetMaxRecursionDepth(c_int(depth))
}

/// Get the maximum recursion depth used by the parser/matcher.
pub fn get_max_recursion_depth() -> i32 {
    FFIGetMaxRecursionDepth().0
}
