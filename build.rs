use std::env;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use cmake::Config as CMakeConfig;
use std::fs::{create_dir_all, copy};

fn abs_path<P: AsRef<Path>>(p: P) -> PathBuf {
    if p.as_ref().is_absolute() {
        p.as_ref().to_path_buf()
    } else {
        env::current_dir().expect("current_dir failed").join(p)
    }
}

fn main() {
    let manifest_dir = abs_path(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let xgrammar_src_dir = manifest_dir.join("external/xgrammar");
    let xgrammar_include_dir = xgrammar_src_dir.join("include");
    let dlpack_include_dir = xgrammar_src_dir.join("3rdparty/dlpack/include");
    let src_include_dir = manifest_dir.join("src");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));

    println!("cargo:rerun-if-changed={}", xgrammar_include_dir.display());
    println!("cargo:rerun-if-changed={}/cpp", xgrammar_src_dir.display());
    println!("cargo:rerun-if-changed={}/3rdparty", xgrammar_src_dir.display());

    let mut cmake_config = CMakeConfig::new(&xgrammar_src_dir);
    cmake_config.define("XGRAMMAR_BUILD_PYTHON_BINDINGS", "OFF");
    cmake_config.define("XGRAMMAR_BUILD_CXX_TESTS", "OFF");
    cmake_config.define("XGRAMMAR_ENABLE_CPPTRACE", "OFF");

    let build_profile = match env::var("PROFILE").unwrap_or_else(|_| "release".into()).as_str() {
        "debug" => "Debug",
        "release" => "Release",
        other => {
            eprintln!("Unknown cargo PROFILE '{}' -> using RelWithDebInfo", other);
            "RelWithDebInfo"
        }
    };
    cmake_config.profile(build_profile);

    if let Ok(target) = env::var("TARGET") {
        if target.contains("apple-darwin") {
            let arch = if target.contains("aarch64") { "arm64" } else { "x86_64" };
            cmake_config.define("CMAKE_OSX_ARCHITECTURES", arch);
        } else if target.contains("apple-ios") || target.contains("apple-ios-sim") {
            let is_sim = target.contains("apple-ios-sim") || target.contains("x86_64-apple-ios");
            let arch = if target.contains("aarch64") { "arm64" } else { "x86_64" };
            let sysroot = if is_sim { "iphonesimulator" } else { "iphoneos" };
            cmake_config.define("CMAKE_OSX_ARCHITECTURES", arch);
            cmake_config.define("CMAKE_OSX_SYSROOT", sysroot);
            if let Ok(dep_target) = env::var("IPHONEOS_DEPLOYMENT_TARGET") {
                cmake_config.define("CMAKE_OSX_DEPLOYMENT_TARGET", dep_target);
            }
        }
    }

    let destination_path = cmake_config.build_target("xgrammar").build();

    let cmake_build_dir = out_dir.join("build");
    let lib_search_dir = find_xgrammar_lib_dir(&cmake_build_dir)
        .or_else(|| find_xgrammar_lib_dir(&destination_path))
        .unwrap_or_else(|| destination_path.join("lib"));
    println!("cargo:rustc-link-search=native={}", lib_search_dir.display());
    println!("cargo:rustc-link-lib=static=xgrammar");

    // Link C++ standard library depending on target
    let target = env::var("TARGET").expect("TARGET not set");
    if target.contains("apple-darwin") {
        println!("cargo:rustc-link-lib=dylib=c++");
    } else if target.contains("windows") {
        // MSVC links the C++ runtime automatically
    } else {
        // Linux and others
        println!("cargo:rustc-link-lib=dylib=stdc++");
        println!("cargo:rustc-link-lib=dylib=pthread");
    }

    // Generate and compile C++ bindings with autocxx.
    println!("cargo:rerun-if-changed=src/lib.rs");
    let mut autocxx_builder = autocxx_build::Builder::new(
        "src/lib.rs",
        &[
            &src_include_dir,
            &xgrammar_include_dir,
            &dlpack_include_dir,
        ],
    )
    .extra_clang_args(&["-std=c++17"])
    .build()
    .expect("autocxx build failed");

    autocxx_builder.flag_if_supported("-std=c++17")
        .include(&src_include_dir)
        .include(&xgrammar_include_dir)
        .include(&dlpack_include_dir)
        .include(&manifest_dir)
        .compile("xgrammar_rs_bridge");


    let rs_dir = out_dir.join("autocxx-build-dir/rs");

    // Provide headers expected by generated RS `include!(...)` paths
    // 1) autocxxgen_ffi.h
    let gen_include_dir = out_dir.join("autocxx-build-dir/include");
    let _ = copy(
        gen_include_dir.join("autocxxgen_ffi.h"),
        rs_dir.join("autocxxgen_ffi.h"),
    );
    // 2) xgrammar/xgrammar.h
    let rs_xgrammar_dir = rs_dir.join("xgrammar");
    create_dir_all(&rs_xgrammar_dir).ok();
    let _ = copy(
        xgrammar_include_dir.join("xgrammar/xgrammar.h"),
        rs_xgrammar_dir.join("xgrammar.h"),
    );
    // 3) dlpack/dlpack.h
    let rs_dlpack_dir = rs_dir.join("dlpack");
    create_dir_all(&rs_dlpack_dir).ok();
    let _ = copy(
        dlpack_include_dir.join("dlpack/dlpack.h"),
        rs_dlpack_dir.join("dlpack.h"),
    );
}

fn find_xgrammar_lib_dir(root: &Path) -> Option<PathBuf> {
    let static_candidates = [
        "libxgrammar.a",      // Unix/macOS static
        "xgrammar.lib",       // Windows static
    ];

    // Scan a few levels deep
    let mut found: Option<PathBuf> = None;
    for entry in WalkDir::new(root).max_depth(6).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if static_candidates.iter().any(|c| name == *c) {
            if let Some(parent) = entry.path().parent() {
                found = Some(parent.to_path_buf());
            }
            break;
        }
    }
    found
}


