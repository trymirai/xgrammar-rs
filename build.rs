use std::env;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

fn abs_path<P: AsRef<Path>>(p: P) -> PathBuf {
    if p.as_ref().is_absolute() {
        p.as_ref().to_path_buf()
    } else {
        env::current_dir().unwrap().join(p)
    }
}

fn main() {
    let manifest_dir = abs_path(env::var("CARGO_MANIFEST_DIR").unwrap());
    let xgrammar_src_dir = manifest_dir.join("external/xgrammar");
    let xgrammar_include_dir = xgrammar_src_dir.join("include");
    let dlpack_include_dir = xgrammar_src_dir.join("3rdparty/dlpack/include");
    let src_include_dir = manifest_dir.join("src");
    let crate_root = manifest_dir.clone();

    // Rebuild when headers or sources change
    println!("cargo:rerun-if-changed={}", xgrammar_include_dir.display());
    println!("cargo:rerun-if-changed={}/cpp", xgrammar_src_dir.display());
    println!("cargo:rerun-if-changed={}/3rdparty", xgrammar_src_dir.display());
    // No longer using cxx bridge files

    // Configure CMake build
    let mut cfg = cmake::Config::new(&xgrammar_src_dir);
    cfg.define("XGRAMMAR_BUILD_PYTHON_BINDINGS", "OFF");
    cfg.define("XGRAMMAR_BUILD_CXX_TESTS", "OFF");
    cfg.define("XGRAMMAR_ENABLE_CPPTRACE", "OFF");
    // Respect cargo profile
    let profile = match env::var("PROFILE").unwrap_or_else(|_| "release".into()).as_str() {
        "debug" => "Debug",
        "release" => "Release",
        other => {
            // Map custom profiles to RelWithDebInfo by default
            eprintln!("Unknown cargo PROFILE '{}' -> using RelWithDebInfo", other);
            "RelWithDebInfo"
        }
    };
    cfg.profile(profile);

    // macOS architectures (arm64/x86_64)
    if let Ok(target) = env::var("TARGET") {
        if target.contains("apple-darwin") {
            let arch = if target.contains("aarch64") { "arm64" } else { "x86_64" };
            cfg.define("CMAKE_OSX_ARCHITECTURES", arch);
        }
    }

    // Build only the static library target; do not attempt to run `install`.
    let dst = cfg.build_target("xgrammar").build();

    // Try to locate the built library robustly
    // Prefer the cmake build directory where artifacts are produced
    let cmake_build_dir = PathBuf::from(env::var("OUT_DIR").unwrap()).join("build");
    let lib_search_dir = find_xgrammar_lib_dir(&cmake_build_dir)
        .or_else(|| find_xgrammar_lib_dir(&dst))
        .unwrap_or_else(|| dst.join("lib"));
    println!("cargo:rustc-link-search=native={}", lib_search_dir.display());
    println!("cargo:rustc-link-lib=static=xgrammar");

    // Link C++ standard library depending on target
    let target = env::var("TARGET").unwrap();
    if target.contains("apple-darwin") {
        println!("cargo:rustc-link-lib=dylib=c++");
        // On some macOS setups, c++abi is needed; uncomment if linking fails
        // println!("cargo:rustc-link-lib=dylib=c++abi");
    } else if target.contains("windows") {
        // MSVC links the C++ runtime automatically
    } else {
        // Linux and others
        println!("cargo:rustc-link-lib=dylib=stdc++");
        println!("cargo:rustc-link-lib=dylib=pthread");
    }

    // Generate and compile C++ bindings with autocxx.
    println!("cargo:rerun-if-changed=src/lib.rs");
    let mut b = autocxx_build::Builder::new(
        "src/lib.rs",
        &[
            &src_include_dir,
            &xgrammar_include_dir,
            &dlpack_include_dir,
        ],
    )
    .extra_clang_args(&["-std=c++17"]) // ensure libclang sees C++17 features like optional/variant
    .build()
    .expect("autocxx build failed");

    b.flag_if_supported("-std=c++17")
        .include(&src_include_dir)
        .include(&xgrammar_include_dir)
        .include(&dlpack_include_dir)
        .include(&crate_root)
        .compile("xgrammar_rs_bridge");

    // Ensure headers referenced by generated RS via `include!("src/...")` are present
    // next to the generated file so that the Rust compiler can resolve them.
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let rs_dir = out_dir.join("autocxx-build-dir/rs");
    // No legacy includes needed anymore
    // Provide headers expected by generated RS `include!(...)` paths
    // 1) autocxxgen_ffi.h
    let gen_include_dir = out_dir.join("autocxx-build-dir/include");
    let _ = std::fs::copy(
        gen_include_dir.join("autocxxgen_ffi.h"),
        rs_dir.join("autocxxgen_ffi.h"),
    );
    // 2) xgrammar/xgrammar.h
    let rs_xgrammar_dir = rs_dir.join("xgrammar");
    std::fs::create_dir_all(&rs_xgrammar_dir).ok();
    let _ = std::fs::copy(
        xgrammar_include_dir.join("xgrammar/xgrammar.h"),
        rs_xgrammar_dir.join("xgrammar.h"),
    );
}

fn find_xgrammar_lib_dir(root: &Path) -> Option<PathBuf> {
    // Prefer static libs
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
            found = Some(entry.path().parent().unwrap().to_path_buf());
            break;
        }
    }
    found
}


