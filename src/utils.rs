use std::ffi::c_char;

#[cfg(not(any(
    all(target_os = "windows", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86"),
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "x86"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64")
)))]
#[inline]
pub fn bytes_as_c_char_ptr(bytes: &[u8]) -> *const c_char {
    bytes.as_ptr() as *const c_char
}

#[cfg(any(
    all(target_os = "windows", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86"),
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "x86"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64")
))]
#[inline]
pub fn bytes_as_c_char_ptr(bytes: &[u8]) -> *const c_char {
    bytes.as_ptr() as *const i8
}

macro_rules! tie_enum_with_ffi {
    ($enum_name:ident, $repr_type:ty, $($variant:ident),+) => {
        // Compile-time checks that all the variants correspond to each other.
        // Additionally ensures that a correct `$repr_type` is specified,
        // otherwise the condition won't type-check.
        $(
        const _: () = assert!(
            ffi::$enum_name::$variant.repr == $enum_name::$variant as $repr_type,
            concat!("Enum values mismatch: ffi::", stringify!($enum_name), ":", stringify!($variant), " != ", stringify!($enum_name), "::", stringify!($variant))
        );
        )+

        // Implement conversion from public to ffi.
        // Additionally ensures that all the variants were listed (otherwise, a non-exhaustive match).
        impl From<$enum_name> for ffi::$enum_name {
            fn from(value: $enum_name) -> Self {
                match value {
                    $(
                    $enum_name::$variant => Self::$variant,
                    )+
                }
            }
        }

        // Implement conversion from ffi to public.
        impl From<ffi::$enum_name> for $enum_name {
            fn from(value: ffi::$enum_name) -> Self {
                match value {
                    $(
                    ffi::$enum_name::$variant => Self::$variant,
                    )+
                    _ => panic!("Recieved an invalid `{}` value from C++", stringify!($enum_name))
                }
            }
        }
    };
}
pub(crate) use tie_enum_with_ffi;
