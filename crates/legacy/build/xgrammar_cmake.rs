use std::{
    env,
    fs::{self, create_dir_all},
    path::{Path, PathBuf},
};

use cmake::Config as CMakeConfig;

use super::{
    BuildContext,
    common::{find_xgrammar_lib_dir, write_if_changed},
};

fn maybe_clear_cmake_build_dir(
    build_dir: &Path,
    source_dir: &Path,
) {
    let cache = build_dir.join("CMakeCache.txt");
    let Ok(contents) = fs::read_to_string(&cache) else {
        return;
    };
    let expected_source =
        source_dir.canonicalize().unwrap_or_else(|_| source_dir.to_path_buf());
    let expected_build =
        build_dir.canonicalize().unwrap_or_else(|_| build_dir.to_path_buf());
    for line in contents.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        let expected = if key.starts_with("CMAKE_HOME_DIRECTORY:") {
            &expected_source
        } else if key.starts_with("CMAKE_CACHEFILE_DIR:") {
            &expected_build
        } else {
            continue;
        };

        let needs_cleanup = fs::canonicalize(value)
            .ok()
            .map(|path| path.as_path() != expected.as_path())
            .unwrap_or(true);

        if needs_cleanup {
            let _ = fs::remove_dir_all(build_dir);
            break;
        }
    }
}

pub fn build_xgrammar_cmake(
    ctx: &BuildContext,
    extra_c_cxx_flags: &[&str],
) -> PathBuf {
    let cmake_build_dir = ctx.out_dir.join("build");
    maybe_clear_cmake_build_dir(&cmake_build_dir, &ctx.xgrammar_src_dir);
    create_dir_all(&cmake_build_dir).ok();

    let config_cmake_path = cmake_build_dir.join("config.cmake");
    write_if_changed(
        &config_cmake_path,
        b"set(XGRAMMAR_BUILD_PYTHON_BINDINGS OFF)\n\
          set(XGRAMMAR_BUILD_CXX_TESTS OFF)\n\
          set(XGRAMMAR_ENABLE_CPPTRACE OFF)\n\
          set(CMAKE_BUILD_TYPE RelWithDebInfo)\n",
    );

    let mut cmake_config = CMakeConfig::new(&ctx.xgrammar_src_dir);
    cmake_config.out_dir(&ctx.out_dir);
    cmake_config.define("XGRAMMAR_BUILD_PYTHON_BINDINGS", "OFF");
    cmake_config.define("XGRAMMAR_BUILD_CXX_TESTS", "OFF");
    cmake_config.define("XGRAMMAR_ENABLE_CPPTRACE", "OFF");
    cmake_config.define("CMAKE_CXX_STANDARD", "17");
    cmake_config.define("CMAKE_CXX_STANDARD_REQUIRED", "ON");
    cmake_config.define("CMAKE_CXX_EXTENSIONS", "OFF");

    cmake_config.define("CMAKE_INTERPROCEDURAL_OPTIMIZATION", "OFF");

    for flag in extra_c_cxx_flags {
        cmake_config.cflag(flag);
        cmake_config.cxxflag(flag);
    }

    let is_msvc = ctx.target.contains("msvc");
    if !is_msvc {
        cmake_config.cflag("-fno-lto");
        cmake_config.cxxflag("-fno-lto");
    } else {
        cmake_config.cxxflag("/EHsc");
    }

    let build_profile = match env::var("PROFILE")
        .unwrap_or_else(|_| "release".into())
        .as_str()
    {
        "debug" => "Debug",
        "release" => "Release",
        other => {
            println!(
                "cargo::warning=Unknown cargo PROFILE '{}' -> using RelWithDebInfo",
                other
            );
            "RelWithDebInfo"
        },
    };
    cmake_config.profile(build_profile);

    if !ctx.target.is_empty() {
        if ctx.target.contains("apple-darwin") {
            let arch = if ctx.target.contains("aarch64") {
                "arm64"
            } else {
                "x86_64"
            };
            cmake_config.define("CMAKE_OSX_ARCHITECTURES", arch);
            // Keep CMake builds aligned with Rust's default macOS minimum; avoids
            // linking objects built for a newer SDK than rustc links against.
            let dep_target = env::var("MACOSX_DEPLOYMENT_TARGET")
                .unwrap_or_else(|_| "11.0".into());
            cmake_config.define("CMAKE_OSX_DEPLOYMENT_TARGET", dep_target);
        } else if ctx.target.contains("apple-ios")
            || ctx.target.contains("apple-ios-sim")
        {
            let is_sim = ctx.target.contains("apple-ios-sim")
                || ctx.target.contains("x86_64-apple-ios");
            let arch = if ctx.target.contains("aarch64") {
                "arm64"
            } else {
                "x86_64"
            };
            let sysroot = if is_sim {
                "iphonesimulator"
            } else {
                "iphoneos"
            };
            cmake_config.define("CMAKE_OSX_ARCHITECTURES", arch);
            cmake_config.define("CMAKE_OSX_SYSROOT", sysroot);
            if let Ok(dep_target) = env::var("IPHONEOS_DEPLOYMENT_TARGET") {
                cmake_config.define("CMAKE_OSX_DEPLOYMENT_TARGET", dep_target);
            }
        }
    }

    if ctx.target.contains("msvc") {
        // Force the release CRT (/MD) even for debug builds to avoid
        // _ITERATOR_DEBUG_LEVEL and runtime mismatches when linking with Rust.
        cmake_config.define("CMAKE_MSVC_RUNTIME_LIBRARY", "MultiThreadedDLL");
        cmake_config.define("CMAKE_C_FLAGS_DEBUG", "/MD");
        cmake_config.define("CMAKE_CXX_FLAGS_DEBUG", "/MD");
    }

    cmake_config.build_target("xgrammar").build()
}

pub fn link_xgrammar_static(
    ctx: &BuildContext,
    destination_path: &Path,
) {
    let cmake_build_dir = ctx.out_dir.join("build");
    let lib_search_dir = find_xgrammar_lib_dir(&cmake_build_dir)
        .or_else(|| find_xgrammar_lib_dir(destination_path))
        .unwrap_or_else(|| destination_path.join("lib"));
    println!("cargo::rustc-link-search=native={}", lib_search_dir.display());
    println!("cargo::rustc-link-lib=static=xgrammar");
}
