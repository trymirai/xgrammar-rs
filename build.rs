use std::{
    env,
    fs::{copy, create_dir_all},
    path::{Path, PathBuf},
    process::Command,
};

use cmake::Config as CMakeConfig;
use walkdir::WalkDir;

// ============================================================================
// Helper Functions
// ============================================================================

fn abs_path<P: AsRef<Path>>(p: P) -> PathBuf {
    if p.as_ref().is_absolute() {
        p.as_ref().to_path_buf()
    } else {
        env::current_dir().expect("current_dir failed").join(p)
    }
}

fn parse_semver_like(name: &str) -> Option<(u64, u64, u64)> {
    let prefix = "xgrammar-";
    if !name.starts_with(prefix) {
        return None;
    }

    let ver = &name[prefix.len()..];
    let mut parts = ver.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    let patch_str = parts.next().unwrap_or("0");

    let mut patch_digits = String::new();
    for ch in patch_str.chars() {
        if ch.is_ascii_digit() {
            patch_digits.push(ch);
        } else {
            break;
        }
    }

    let patch = patch_digits.parse::<u64>().ok().unwrap_or(0);
    Some((major, minor, patch))
}

fn find_latest_xgrammar_src(external_dir: &Path) -> Option<PathBuf> {
    // 1) If XGRAMMAR_SRC_DIR env is set, use it
    if let Ok(p) = env::var("XGRAMMAR_SRC_DIR") {
        let candidate = abs_path(p);
        if candidate.join("CMakeLists.txt").exists()
            || candidate.join("include").exists()
        {
            return Some(candidate);
        }
    }

    // 2) Choose highest xgrammar-<semver> under external/
    let mut best: Option<(PathBuf, (u64, u64, u64))> = None;
    if let Ok(rd) = std::fs::read_dir(external_dir) {
        for e in rd.flatten() {
            if let Ok(ft) = e.file_type() {
                if ft.is_dir() {
                    let name = e.file_name();
                    let name = name.to_string_lossy();
                    if let Some(ver) = parse_semver_like(&name) {
                        let p = e.path();
                        if best.as_ref().map(|b| ver > b.1).unwrap_or(true) {
                            best = Some((p, ver));
                        }
                    }
                }
            }
        }
    }

    if let Some((p, _)) = best {
        return Some(p);
    }

    // 3) Fallback to external/xgrammar if it looks like a source dir
    let fallback = external_dir.join("xgrammar");
    if fallback.join("CMakeLists.txt").exists()
        || fallback.join("include").exists()
    {
        return Some(fallback);
    }

    None
}

fn find_xgrammar_lib_dir(root: &Path) -> Option<PathBuf> {
    let static_candidates = [
        "libxgrammar.a", // Unix/macOS static
        "xgrammar.lib",  // Windows static
    ];

    for entry in
        WalkDir::new(root).max_depth(6).into_iter().filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let name = entry.file_name().to_string_lossy();
        if static_candidates.iter().any(|c| name == *c) {
            return entry.path().parent().map(|p| p.to_path_buf());
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn find_libclang_windows() -> Option<PathBuf> {
    let vswhere = PathBuf::from(
        r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe",
    );

    let mut candidates: Vec<PathBuf> = Vec::new();

    // 1) Try vswhere to locate VS with LLVM Clang component
    if vswhere.exists() {
        let args = [
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Llvm.Clang",
            "-property",
            "installationPath",
        ];

        if let Ok(out) = Command::new(&vswhere).args(args).output() {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
                    let base = PathBuf::from(line.trim());
                    candidates.push(base.join(r"VC\Tools\Llvm\x64\bin"));
                    candidates.push(base.join(r"VC\Tools\Llvm\bin"));
                }
            }
        }
    }

    // 2) Common fallback locations (VS 2022 editions)
    for edition in ["Community", "Professional", "Enterprise"] {
        candidates.push(PathBuf::from(format!(
            r"C:\Program Files\Microsoft Visual Studio\2022\{}\VC\Tools\Llvm\x64\bin",
            edition
        )));
        candidates.push(PathBuf::from(format!(
            r"C:\Program Files\Microsoft Visual Studio\2022\{}\VC\Tools\Llvm\bin",
            edition
        )));
    }

    // 3) Standalone LLVM installation
    candidates.push(PathBuf::from(r"C:\Program Files\LLVM\bin"));

    // Return the first directory that contains libclang.dll
    for dir in candidates {
        if dir.join("libclang.dll").exists() {
            return Some(dir);
        }
    }

    None
}

#[cfg(not(target_os = "windows"))]
fn find_libclang_windows() -> Option<PathBuf> {
    None
}

// ============================================================================
// Main Build Script
// ============================================================================

fn main() {
    // ========================================================================
    // Step 1: Configure libclang (Windows-specific)
    // ========================================================================
    if env::var("LIBCLANG_PATH").is_err() {
        if cfg!(target_os = "windows") {
            if let Some(dir) = find_libclang_windows() {
                // Make available to this build script and to downstream rustc invocations
                unsafe {
                    env::set_var("LIBCLANG_PATH", &dir);
                }
                println!("cargo:rustc-env=LIBCLANG_PATH={}", dir.display());
            }
        }
    }

    // ========================================================================
    // Step 2: Locate XGrammar source and set up paths
    // ========================================================================

    let manifest_dir = abs_path(
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"),
    );

    let external_dir = manifest_dir.join("external");
    let xgrammar_src_dir = find_latest_xgrammar_src(&external_dir)
        .unwrap_or_else(|| manifest_dir.join("external/xgrammar-0.1.28"));
    let xgrammar_include_dir = xgrammar_src_dir.join("include");
    let dlpack_include_dir = xgrammar_src_dir.join("3rdparty/dlpack/include");
    let picojson_include_dir = xgrammar_src_dir.join("3rdparty/picojson");
    let src_include_dir = manifest_dir.join("src");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));

    println!("cargo:rerun-if-changed={}", xgrammar_include_dir.display());
    println!("cargo:rerun-if-changed={}/cpp", xgrammar_src_dir.display());
    println!("cargo:rerun-if-changed={}/3rdparty", xgrammar_src_dir.display());

    // ========================================================================
    // Step 3: Configure and build XGrammar C++ library with CMake
    // ========================================================================
    let cmake_build_dir = out_dir.join("build");
    create_dir_all(&cmake_build_dir).ok();
    let config_cmake_path = cmake_build_dir.join("config.cmake");
    std::fs::write(
        &config_cmake_path,
        "set(XGRAMMAR_BUILD_PYTHON_BINDINGS OFF)\n\
         set(XGRAMMAR_BUILD_CXX_TESTS OFF)\n\
         set(XGRAMMAR_ENABLE_CPPTRACE OFF)\n\
         set(CMAKE_BUILD_TYPE RelWithDebInfo)\n",
    )
    .expect("Failed to write config.cmake");

    let mut cmake_config = CMakeConfig::new(&xgrammar_src_dir);
    cmake_config.out_dir(&out_dir);
    cmake_config.define("XGRAMMAR_BUILD_PYTHON_BINDINGS", "OFF");
    cmake_config.define("XGRAMMAR_BUILD_CXX_TESTS", "OFF");
    cmake_config.define("XGRAMMAR_ENABLE_CPPTRACE", "OFF");
    cmake_config.define("CMAKE_CXX_STANDARD", "17");
    cmake_config.define("CMAKE_CXX_STANDARD_REQUIRED", "ON");
    cmake_config.define("CMAKE_CXX_EXTENSIONS", "OFF");

    // Disable LTO to avoid linking issues with Rust on some platforms
    cmake_config.define("CMAKE_INTERPROCEDURAL_OPTIMIZATION", "OFF");

    // Platform-specific compiler flags
    let target = env::var("TARGET").unwrap_or_default();
    let is_msvc = target.contains("msvc");
    if !is_msvc {
        cmake_config.cflag("-fno-lto");
        cmake_config.cxxflag("-fno-lto");
    } else {
        // Ensure correct exception semantics for C++ code generated/used via autocxx/cxx
        cmake_config.cxxflag("/EHsc");
    }

    let build_profile =
        match env::var("PROFILE").unwrap_or_else(|_| "release".into()).as_str()
        {
            "debug" => "Debug",
            "release" => "Release",
            other => {
                eprintln!(
                    "Unknown cargo PROFILE '{}' -> using RelWithDebInfo",
                    other
                );
                "RelWithDebInfo"
            },
        };
    cmake_config.profile(build_profile);

    // Apple platform-specific configuration
    if let Ok(target) = env::var("TARGET") {
        if target.contains("apple-darwin") {
            let arch = if target.contains("aarch64") {
                "arm64"
            } else {
                "x86_64"
            };
            cmake_config.define("CMAKE_OSX_ARCHITECTURES", arch);
        } else if target.contains("apple-ios")
            || target.contains("apple-ios-sim")
        {
            let is_sim = target.contains("apple-ios-sim")
                || target.contains("x86_64-apple-ios");
            let arch = if target.contains("aarch64") {
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

    let destination_path = cmake_config.build_target("xgrammar").build();

    // ========================================================================
    // Step 4: Link the built XGrammar library
    // ========================================================================

    let cmake_build_dir = out_dir.join("build");
    let lib_search_dir = find_xgrammar_lib_dir(&cmake_build_dir)
        .or_else(|| find_xgrammar_lib_dir(&destination_path))
        .unwrap_or_else(|| destination_path.join("lib"));
    println!("cargo:rustc-link-search=native={}", lib_search_dir.display());
    println!("cargo:rustc-link-lib=static=xgrammar");

    // ========================================================================
    // Step 5: Generate and compile Rust/C++ bindings with autocxx
    // ========================================================================

    println!("cargo:rerun-if-changed=src/lib.rs");

    // Prepare extra clang args for autocxx
    let mut extra_clang_args = vec!["-std=c++17".to_string()];

    // Platform-specific clang args for autocxx
    let target = env::var("TARGET").unwrap_or_default();

    // Windows: explicitly set the target to avoid ARM NEON header issues
    if target.contains("windows") {
        if target.contains("aarch64") {
            extra_clang_args
                .push("--target=aarch64-pc-windows-msvc".to_string());
        } else if target.contains("x86_64") {
            extra_clang_args
                .push("--target=x86_64-pc-windows-msvc".to_string());
        }
    }

    // iOS Simulator: set correct target triple and sysroot for C++ headers
    if target.contains("apple-ios-sim") || target.contains("x86_64-apple-ios") {
        let arch = if target.contains("aarch64") {
            "arm64"
        } else {
            "x86_64"
        };
        let version = env::var("IPHONEOS_DEPLOYMENT_TARGET")
            .unwrap_or_else(|_| "17.0".into());
        extra_clang_args
            .push(format!("--target={}-apple-ios{}-simulator", arch, version));
        if let Ok(sdkroot) = env::var("SDKROOT") {
            extra_clang_args.push(format!("-isysroot{}", sdkroot));
        }
    }

    let extra_clang_args_refs: Vec<&str> =
        extra_clang_args.iter().map(|s| s.as_str()).collect();

    // Build the autocxx bridge
    let mut autocxx_builder = autocxx_build::Builder::new(
        "src/lib.rs",
        &[
            &src_include_dir,
            &xgrammar_include_dir,
            &dlpack_include_dir,
            &picojson_include_dir,
        ],
    )
    .extra_clang_args(&extra_clang_args_refs) // for libclang parsing
    .build()
    .expect("autocxx build failed");

    autocxx_builder
        .flag_if_supported("-std=c++17")
        .flag_if_supported("/std:c++17")
        .flag_if_supported("/EHsc")
        .include(&src_include_dir)
        .include(&xgrammar_include_dir)
        .include(&dlpack_include_dir)
        .include(&picojson_include_dir)
        .include(&manifest_dir);

    autocxx_builder.compile("xgrammar_rs_bridge");

    // ========================================================================
    // Step 6: Copy headers for generated Rust code
    // ========================================================================

    let rs_dir = out_dir.join("autocxx-build-dir/rs");
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

    // ========================================================================
    // Step 7: Format generated bindings (optional)
    // ========================================================================
    let gen_rs =
        out_dir.join("autocxx-build-dir/rs/autocxx-ffi-default-gen.rs");
    if gen_rs.exists() {
        match Command::new("rustfmt").arg(&gen_rs).status() {
            Ok(status) => {
                if !status.success() {
                    eprintln!(
                        "rustfmt returned non-zero status on {}",
                        gen_rs.display()
                    );
                }
            },
            Err(err) => {
                eprintln!("rustfmt not executed: {}", err);
            },
        }
    }
}
