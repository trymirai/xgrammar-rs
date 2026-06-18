use std::{
    env,
    path::{Path, PathBuf},
};

use cmake::Config as CMakeConfig;
use which::which;

pub fn find_or_build_and_set() {
    let wasi_sdk_path = "build/wasi-sdk";

    if env::var("WASI_SYSROOT").is_err() {
        let built_wasi_sdk = build_wasi_sdk(&wasi_sdk_path);
        // Note: the `build/` prefix in the path below is created by the `cmake` crate (hardcoded, as of this writing)
        let wasi_sysroot_path =
            built_wasi_sdk.join("build/install/share/wasi-sysroot/");
        // Note: need to set this as an environment variable, since `cc-rs` does not provide a method
        // for specifying wasi sysroot location, and only checks an environment variable.
        // SAFETY: The function is safe in single-threaded programs (like this build script)
        unsafe { env::set_var("WASI_SYSROOT", &wasi_sysroot_path) };
    }
}

static NEED_CLANG_LIKE_COMPILER_ERR_MSG: &str =
    "Wasi-sdk requires clang-like C and C++ compilers, but:";

fn wasi_c_compiler_cfg() -> Option<cc::Build> {
    let mut c_build_config = cc::Build::new();
    c_build_config.cpp(false);
    let default_c_compiler = c_build_config.get_compiler();
    if default_c_compiler.is_like_clang() {
        // Need to set the compiler to itself 🤪
        // That's because we want to fix this compiler (which we know is clang-like).
        // Otherwise `cc` may decide to change it later
        c_build_config.compiler(c_build_config.get_compiler().path());
    } else {
        println!("cargo::warning=C: not like clang");
        println!("cargo::warning={NEED_CLANG_LIKE_COMPILER_ERR_MSG}");
        println!(
            "cargo::warning=C compiler at {:?} is not clang-like (you may set the `CC` environment variable to the path to your C compiler)",
            default_c_compiler.path()
        );
        let Ok(path_to_clang) = which("clang") else {
            println!("cargo::error=Could not find `clang` on the system");
            return None;
        };
        println!("cargo::warning=Using {:?} as a C compiler", path_to_clang);
        c_build_config.compiler(path_to_clang);
    }
    Some(c_build_config)
}

fn wasi_cpp_compiler_cfg() -> Option<cc::Build> {
    let mut cpp_build_config = cc::Build::new();
    cpp_build_config.cpp(true);
    let default_cpp_compiler = cpp_build_config.get_compiler();
    if default_cpp_compiler.is_like_clang() {
        // Need to set the compiler to itself 🤪
        // That's because we want to fix this compiler (which we know is clang-like).
        // Otherwise `cc` may decide to change it later
        cpp_build_config.compiler(cpp_build_config.get_compiler().path());
    } else {
        println!("cargo::warning={NEED_CLANG_LIKE_COMPILER_ERR_MSG}");
        println!(
            "cargo::warning=C++ compiler {:?} is not clang-like (you may set the `CXX` environment variable to the path to your C++ compiler)",
            default_cpp_compiler.path()
        );
        let Ok(path_to_clang) = which("clang++") else {
            println!("cargo::error=Could not find `clang++` on the system");
            return None;
        };
        println!("cargo::warning=Using {:?} as a C++ compiler", path_to_clang);
        cpp_build_config.compiler(path_to_clang);
    }
    Some(cpp_build_config)
}

fn build_wasi_sdk(wasi_sdk_path: &dyn AsRef<Path>) -> PathBuf {
    let wasi_sdk_path = wasi_sdk_path.as_ref();

    // Call the two functions together, so that all of their warnings/errors are emitted, even if one of them fails.
    let (Some(c_compiler_cfg), Some(cpp_compiler_cfg)) =
        (wasi_c_compiler_cfg(), wasi_cpp_compiler_cfg())
    else {
        // Error messages were printed by `wasi_{c,cpp}_compiler`
        std::process::exit(1);
    };

    let mut cmake_config = CMakeConfig::new(wasi_sdk_path);
    let wasi_build_dir =
        PathBuf::from(env::var("OUT_DIR").expect("cargo shall set OUT_DIR"))
            .join("wasi_sysroot");
    cmake_config.out_dir(wasi_build_dir)
        .target(&env::var("HOST").expect("cargo shall set HOST"))
        .no_default_flags(true) // We are in a cross-compiling environment but this build is for the host
        .init_c_cfg(c_compiler_cfg)
        .init_cxx_cfg(cpp_compiler_cfg)
        .define("WASI_SDK_EXCEPTIONS", "ON")
        .define("WASI_SDK_INCLUDE_TESTS", "OFF")
        .profile(match env::var("PROFILE").expect("PROFILE is not set").as_str() {
            "debug" => "Debug",
            "release" => "Release",
            unknown => {
                println!("cargo::warning=Unknown cargo PROFILE '{unknown}', building as RelWithDebInfo");
                "RelWithDebInfo"
            }
        });
    cmake_config.build()
}
