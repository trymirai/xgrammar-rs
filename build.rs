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
    let picojson_include_dir = xgrammar_src_dir.join("3rdparty/picojson");
    let dlpack_include_dir = xgrammar_src_dir.join("3rdparty/dlpack/include");

    // Rebuild when headers or sources change
    println!("cargo:rerun-if-changed={}", xgrammar_include_dir.display());
    println!("cargo:rerun-if-changed={}/cpp", xgrammar_src_dir.display());
    println!("cargo:rerun-if-changed={}/3rdparty", xgrammar_src_dir.display());

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

    // Run bindgen to generate Rust bindings to the public C++ headers
    let bindings_out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs");
    // Generate bindings from the stable C API header
    let header = xgrammar_include_dir.join("xgrammar/c_api.h");

    // Invalidate when the header changes
    println!("cargo:rerun-if-changed={}", header.display());

    let mut builder = bindgen::Builder::default()
        .header(header.display().to_string())
        // Treat input as C
        .clang_arg("-x")
        .clang_arg("c")
        // Include path for public headers
        .clang_arg(format!("-I{}", xgrammar_include_dir.display()))
        .allowlist_function("xgrammar_.*")
        .allowlist_type("xgrammar_.*")
        .allowlist_var("xgrammar_.*")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .size_t_is_usize(true)
        .generate_inline_functions(false);

    // On Apple, bindgen may need sysroot for headers when using clang on some setups
    if target.contains("apple-darwin") {
        if let Ok(sdkroot) = env::var("SDKROOT") {
            builder = builder.clang_arg(format!("-isysroot{}", sdkroot));
        }
    }

    let bindings = builder
        .generate()
        .expect("Unable to generate xgrammar bindings");

    bindings
        .write_to_file(&bindings_out)
        .expect("Couldn't write bindings!");

    // Expose include dir (optional for downstream crates)
    println!("cargo:include={}", xgrammar_include_dir.display());
}

fn find_xgrammar_lib_dir(root: &Path) -> Option<PathBuf> {
    // Prefer static libs
    let static_candidates = [
        "libxgrammar.a",      // Unix/macOS static
        "xgrammar.lib",       // Windows static
    ];
    let shared_candidates = [
        "libxgrammar.dylib",  // macOS shared
        "libxgrammar.so",     // Linux shared
        "xgrammar.dll",       // Windows shared
        "xgrammar.dll.lib",   // Windows import lib
    ];

    // Scan a few levels deep
    let mut found: Option<PathBuf> = None;
    for entry in WalkDir::new(root).max_depth(6).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if static_candidates.iter().any(|c| name == *c)
            || shared_candidates.iter().any(|c| name == *c)
        {
            found = Some(entry.path().parent().unwrap().to_path_buf());
            break;
        }
    }
    found
}


