use std::{
    collections::HashSet,
    env,
    fs::{self, copy, create_dir_all},
    path::{Path, PathBuf},
    process::Command,
};

use cmake::Config as CMakeConfig;
use walkdir::WalkDir;

fn abs_path<P: AsRef<Path>>(p: P) -> PathBuf {
    if p.as_ref().is_absolute() {
        p.as_ref().to_path_buf()
    } else {
        env::current_dir().expect("current_dir failed").join(p)
    }
}

fn parse_include_search_list(stderr: &str) -> Vec<String> {
    let mut includes = Vec::new();
    let mut in_section = false;
    for line in stderr.lines() {
        if line.contains("#include <...> search starts here:") {
            in_section = true;
            continue;
        }
        if in_section {
            if line.contains("End of search list") {
                break;
            }
            let trimmed = line.trim();
            if !trimmed.is_empty() && trimmed.starts_with('/') {
                includes.push(trimmed.to_string());
            }
        }
    }
    includes
}

fn normalize_include_path(path: &str) -> String {
    let p = Path::new(path);
    if p.is_absolute() {
        match fs::canonicalize(p) {
            Ok(p) => p.display().to_string(),
            Err(_) => path.to_string(),
        }
    } else {
        path.to_string()
    }
}

fn gcc_multiarch_triple(target: &str) -> Option<String> {
    if let Ok(out) = Command::new("gcc").arg("-print-multiarch").output() {
        if let Ok(triple) = String::from_utf8(out.stdout) {
            let t = triple.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    if !target.is_empty() {
        return Some(target.to_string());
    }
    None
}

fn gcc_version() -> Option<String> {
    if let Ok(out) = Command::new("gcc").arg("-dumpversion").output() {
        if let Ok(v) = String::from_utf8(out.stdout) {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn probe_compiler_includes(
    compiler: &str,
    target: &str,
) -> Vec<String> {
    let mut args = vec!["-E", "-x", "c++", "-", "-v"];
    if !target.is_empty() {
        args.push("-target");
        args.push(target);
    }

    let output = Command::new(compiler)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output();

    match output {
        Ok(out) => {
            parse_include_search_list(&String::from_utf8_lossy(&out.stderr))
                .into_iter()
                .map(|p| normalize_include_path(&p))
                .collect()
        },
        Err(_) => Vec::new(),
    }
}

fn fallback_include_paths(target: &str) -> Vec<String> {
    let mut paths = Vec::new();

    let mut libc_dirs = Vec::new();
    if let Some(triple) = gcc_multiarch_triple(target) {
        libc_dirs.push(format!("/usr/include/{}", triple));
    }
    libc_dirs.push("/usr/include".to_string());
    libc_dirs.push("/usr/local/include".to_string());

    if let (Some(triple), Some(version)) =
        (gcc_multiarch_triple(target), gcc_version())
    {
        paths.push(format!("/usr/lib/gcc/{}/{}/include", triple, version));
        paths
            .push(format!("/usr/lib/gcc/{}/{}/include-fixed", triple, version));
        paths.push(format!("/usr/include/c++/{}", version));
        paths.push(format!("/usr/include/{}/c++/{}", triple, version));
        paths.extend(libc_dirs.into_iter());
    } else {
        paths.extend(libc_dirs.into_iter());
    }

    if let Ok(out) = Command::new("gcc").arg("-print-libgcc-file-name").output()
    {
        if let Ok(path) = String::from_utf8(out.stdout) {
            let p = PathBuf::from(path.trim());
            if let Some(include_dir) =
                p.parent().and_then(|p| p.parent()).map(|p| p.join("include"))
            {
                paths.push(normalize_include_path(
                    &include_dir.display().to_string(),
                ));
            }
        }
    }

    paths
}

fn collect_system_include_args(target: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut seen = HashSet::new();

    let mut compilers = vec!["clang++", "g++", "gcc", "c++", "cc"];
    if let Ok(cxx) = env::var("CXX") {
        compilers.insert(0, Box::leak(cxx.into_boxed_str()));
    }

    for compiler in compilers {
        let includes = probe_compiler_includes(compiler, target);
        if !includes.is_empty() {
            for path in includes {
                if seen.insert(path.clone()) {
                    args.push(format!(
                        "-isystem{}",
                        normalize_include_path(&path)
                    ));
                }
            }
        }
    }

    // Always add a conservative fallback set to ensure glibc headers are visible,
    // even if probing succeeded but missed multiarch include dirs.
    for path in fallback_include_paths(target) {
        if seen.insert(path.clone()) {
            args.push(format!("-isystem{}", normalize_include_path(&path)));
        }
    }

    args
}

fn windows_target_clang_args(target: &str) -> Vec<String> {
    let mut args = Vec::new();
    if target.contains("windows") {
        if target.contains("aarch64") {
            args.push("--target=aarch64-pc-windows-msvc".to_string());
        } else if target.contains("x86_64") {
            args.push("--target=x86_64-pc-windows-msvc".to_string());
        }
    }
    args
}

fn apple_target_clang_args(target: &str) -> Vec<String> {
    let mut args = Vec::new();
    if target.contains("apple-ios-sim") || target.contains("x86_64-apple-ios") {
        let arch = if target.contains("aarch64") {
            "arm64"
        } else {
            "x86_64"
        };
        let version = env::var("IPHONEOS_DEPLOYMENT_TARGET")
            .unwrap_or_else(|_| "17.0".into());
        args.push(format!("--target={}-apple-ios{}-simulator", arch, version));
        if let Ok(sdkroot) = env::var("SDKROOT") {
            args.push(format!("-isysroot{}", sdkroot));
        }
    }
    args
}

fn linux_clang_include_args(target: &str) -> Vec<String> {
    let mut args = vec!["--sysroot=/".to_string()];
    args.extend(collect_system_include_args(target));
    args
}

fn looks_like_xgrammar_repo_root(dir: &Path) -> bool {
    dir.join("CMakeLists.txt").exists()
        && dir.join("include").exists()
        && dir.join("cpp").exists()
}

fn is_truthy_env(name: &str) -> bool {
    let Ok(v) = env::var(name) else {
        return false;
    };
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

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

fn write_if_changed(
    path: &Path,
    contents: &[u8],
) {
    if let Ok(existing) = fs::read(path) {
        if existing == contents {
            return;
        }
    }
    if let Some(parent) = path.parent() {
        create_dir_all(parent).ok();
    }
    fs::write(path, contents).unwrap_or_else(|e| {
        panic!("Failed to write {}: {}", path.display(), e)
    });
}

fn copy_if_changed(
    src: &Path,
    dst: &Path,
) {
    if let (Ok(src_bytes), Ok(dst_bytes)) = (fs::read(src), fs::read(dst)) {
        if src_bytes == dst_bytes {
            return;
        }
    }
    if let Some(parent) = dst.parent() {
        create_dir_all(parent).ok();
    }
    let _ = copy(src, dst);
}

fn ensure_xgrammar_source_tree(manifest_dir: &Path) -> PathBuf {
    let source_dir = if let Ok(p) = env::var("XGRAMMAR_SRC_DIR") {
        let p = abs_path(p);
        if !looks_like_xgrammar_repo_root(&p) {
            panic!(
                "XGRAMMAR_SRC_DIR={} does not look like an XGrammar repo root (expected CMakeLists.txt + include/ + cpp/)",
                p.display()
            );
        }
        p
    } else {
        let source_dir = manifest_dir.join("xgrammar");
        if !looks_like_xgrammar_repo_root(&source_dir) {
            panic!(
                "XGrammar submodule is not initialized at {}. \
                 Run `git submodule update --init --recursive` or set \
                 XGRAMMAR_SRC_DIR to a checked-out XGrammar repo root.",
                source_dir.display()
            );
        }
        source_dir
    };

    let dlpack_header =
        source_dir.join("3rdparty/dlpack/include/dlpack/dlpack.h");
    if !dlpack_header.exists() {
        panic!(
            "Required git submodule `3rdparty/dlpack` is missing (expected {}). \
             Run `git submodule update --init --recursive` or set \
             XGRAMMAR_SRC_DIR to an XGrammar repo root with submodules initialized.",
            dlpack_header.display()
        );
    }

    source_dir
}

fn find_xgrammar_lib_dir(root: &Path) -> Option<PathBuf> {
    let static_candidates = ["libxgrammar.a", "xgrammar.lib"];

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

fn fix_broken_bindgen_type_aliases(out_dir: &Path) {
    let rs_dir = out_dir.join("autocxx-build-dir/rs");
    let Ok(rd) = fs::read_dir(&rs_dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.starts_with("autocxx-") || !name.ends_with("-gen.rs") {
            continue;
        }
        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        let patched = contents.replace(
            "pub type basic_string___self_view =",
            "pub type basic_string___self_view<_CharT> =",
        );
        if patched != contents {
            let _ = fs::write(&path, patched);
        }
    }
}

fn strip_autocxx_generated_doc_comments(out_dir: &Path) {
    let debug = env::var("XGRAMMAR_RS_DEBUG_DOCSTRIP").is_ok();
    let rs_dir = out_dir.join("autocxx-build-dir/rs");
    if debug {
        println!("cargo:warning=docstrip: scanning {}", rs_dir.display());
    }
    let Ok(rd) = std::fs::read_dir(&rs_dir) else {
        if debug {
            println!("cargo:warning=docstrip: rs dir missing");
        }
        return;
    };
    let entries: Vec<_> = rd.flatten().collect();
    if debug {
        let mut names: Vec<String> = entries
            .iter()
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        names.sort();
        println!("cargo:warning=docstrip: entries={}", names.join(", "));
    }
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !file_name.starts_with("autocxx-") || !file_name.ends_with("-gen.rs")
        {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&path) else {
            if debug {
                println!(
                    "cargo:warning=docstrip: failed to read {}",
                    path.display()
                );
            }
            continue;
        };
        if debug {
            let count = contents.matches("#[doc =").count();
            println!(
                "cargo:warning=docstrip: {} contains {} #[doc =] lines",
                file_name, count
            );
        }
        let mut changed = false;
        let mut removed = 0usize;
        let mut out = String::with_capacity(contents.len());
        for line in contents.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("#[doc =") {
                changed = true;
                removed += 1;
                continue;
            }
            out.push_str(line);
            out.push('\n');
        }
        if changed {
            if debug {
                println!(
                    "cargo:warning=docstrip: {} removed {} doc lines",
                    file_name, removed
                );
            }
            let _ = std::fs::write(&path, out);
        }
    }
}

#[cfg(target_os = "windows")]
fn find_libclang_windows() -> Option<PathBuf> {
    let vswhere = PathBuf::from(
        r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe",
    );

    let mut candidates: Vec<PathBuf> = Vec::new();

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
                    // Prefer host x64 libclang; fall back to ARM64 if present.
                    candidates.push(base.join(r"VC\Tools\Llvm\x64\bin"));
                    candidates.push(base.join(r"VC\Tools\Llvm\bin"));
                    candidates.push(base.join(r"VC\Tools\Llvm\ARM64\bin"));
                }
            }
        }
    }

    for edition in ["Community", "Professional", "Enterprise"] {
        candidates.push(PathBuf::from(format!(
            r"C:\Program Files\Microsoft Visual Studio\2022\{}\VC\Tools\Llvm\x64\bin",
            edition
        )));
        candidates.push(PathBuf::from(format!(
            r"C:\Program Files\Microsoft Visual Studio\2022\{}\VC\Tools\Llvm\bin",
            edition
        )));
        candidates.push(PathBuf::from(format!(
            r"C:\Program Files\Microsoft Visual Studio\2022\{}\VC\Tools\Llvm\ARM64\bin",
            edition
        )));
    }

    candidates.push(PathBuf::from(r"C:\Program Files\LLVM\bin"));

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
// Build orchestration
// ============================================================================

#[derive(Debug, Clone)]
struct BuildContext {
    manifest_dir: PathBuf,
    xgrammar_src_dir: PathBuf,
    out_dir: PathBuf,

    src_include_dir: PathBuf,
    xgrammar_include_dir: PathBuf,
    dlpack_include_dir: PathBuf,
    picojson_include_dir: PathBuf,

    target: String,
}

fn configure_libclang_windows() {
    if env::var("LIBCLANG_PATH").is_err() && cfg!(target_os = "windows") {
        if let Some(dir) = find_libclang_windows() {
            let host_is_arm64 = cfg!(target_arch = "aarch64");
            let base = dir.parent().and_then(|p| p.parent());
            let mut candidates: Vec<PathBuf> = Vec::new();
            if let Some(base) = base {
                if host_is_arm64 {
                    candidates.push(base.join("ARM64").join("bin"));
                    candidates.push(base.join("x64").join("bin"));
                } else {
                    candidates.push(base.join("x64").join("bin"));
                    candidates.push(base.join("ARM64").join("bin"));
                }
            }
            candidates.push(dir.clone());

            let chosen = candidates
                .into_iter()
                .find(|p| p.join("libclang.dll").exists())
                .unwrap_or_else(|| dir.clone());

            unsafe { env::set_var("LIBCLANG_PATH", &chosen) };
            println!("cargo:rustc-env=LIBCLANG_PATH={}", chosen.display());
        }
    }
}

fn collect_build_context() -> BuildContext {
    println!("cargo:rerun-if-env-changed=XGRAMMAR_SRC_DIR");

    let manifest_dir = abs_path(
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"),
    );
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));

    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join(".gitmodules").display()
    );

    let xgrammar_src_dir = ensure_xgrammar_source_tree(&manifest_dir);
    println!(
        "cargo:rerun-if-changed={}",
        xgrammar_src_dir.join("include").display()
    );
    println!("cargo:rerun-if-changed={}/cpp", xgrammar_src_dir.display());
    println!("cargo:rerun-if-changed={}/3rdparty", xgrammar_src_dir.display());
    println!(
        "cargo:rerun-if-changed={}",
        xgrammar_src_dir.join("CMakeLists.txt").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        xgrammar_src_dir.join(".gitmodules").display()
    );

    let xgrammar_include_dir = xgrammar_src_dir.join("include");
    let dlpack_include_dir = xgrammar_src_dir.join("3rdparty/dlpack/include");
    let picojson_include_dir = xgrammar_src_dir.join("3rdparty/picojson");
    let src_include_dir = manifest_dir.join("src");

    let target = env::var("TARGET").unwrap_or_default();

    BuildContext {
        manifest_dir,
        xgrammar_src_dir,
        out_dir,
        src_include_dir,
        xgrammar_include_dir,
        dlpack_include_dir,
        picojson_include_dir,
        target,
    }
}

fn build_xgrammar_cmake(ctx: &BuildContext) -> PathBuf {
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

    let is_msvc = ctx.target.contains("msvc");
    if !is_msvc {
        cmake_config.cflag("-fno-lto");
        cmake_config.cxxflag("-fno-lto");
    } else {
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

fn link_xgrammar_static(
    ctx: &BuildContext,
    destination_path: &Path,
) {
    let cmake_build_dir = ctx.out_dir.join("build");
    let lib_search_dir = find_xgrammar_lib_dir(&cmake_build_dir)
        .or_else(|| find_xgrammar_lib_dir(destination_path))
        .unwrap_or_else(|| destination_path.join("lib"));
    println!("cargo:rustc-link-search=native={}", lib_search_dir.display());
    println!("cargo:rustc-link-lib=static=xgrammar");
}

fn build_autocxx_bridge(ctx: &BuildContext) {
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/cxx_utils.hpp");
    println!("cargo:rerun-if-changed=src/cxx_utils");

    let mut extra_clang_args = vec!["-std=c++17".to_string()];
    extra_clang_args.extend(windows_target_clang_args(&ctx.target));
    extra_clang_args.extend(apple_target_clang_args(&ctx.target));
    if ctx.target.contains("linux") {
        extra_clang_args.extend(linux_clang_include_args(&ctx.target));
    }
    if is_truthy_env("XGRAMMAR_RS_DEBUG_INCLUDES") {
        println!(
            "cargo:warning=xgrammar-rs: extra_clang_args={}",
            extra_clang_args.join(" ")
        );
    }

    let extra_clang_args_refs: Vec<&str> =
        extra_clang_args.iter().map(|s| s.as_str()).collect();

    let mut autocxx_builder = autocxx_build::Builder::new(
        "src/lib.rs",
        &[
            &ctx.src_include_dir,
            &ctx.xgrammar_include_dir,
            &ctx.dlpack_include_dir,
            &ctx.picojson_include_dir,
            &ctx.xgrammar_src_dir,
        ],
    )
    .extra_clang_args(&extra_clang_args_refs)
    .build()
    .expect("autocxx build failed");

    autocxx_builder
        .flag_if_supported("-std=c++17")
        .flag_if_supported("/std:c++17")
        .flag_if_supported("/EHsc")
        .include(&ctx.src_include_dir)
        .include(&ctx.xgrammar_include_dir)
        .include(&ctx.dlpack_include_dir)
        .include(&ctx.picojson_include_dir)
        .include(&ctx.xgrammar_src_dir)
        .include(&ctx.manifest_dir);

    autocxx_builder.compile("xgrammar_rs_bridge");
}

fn copy_headers_for_generated_rust_code(ctx: &BuildContext) {
    let rs_dir = ctx.out_dir.join("autocxx-build-dir/rs");

    let gen_include_dir = ctx.out_dir.join("autocxx-build-dir/include");
    copy_if_changed(
        &gen_include_dir.join("autocxxgen_ffi.h"),
        &rs_dir.join("autocxxgen_ffi.h"),
    );

    let rs_xgrammar_dir = rs_dir.join("xgrammar");
    create_dir_all(&rs_xgrammar_dir).ok();
    copy_if_changed(
        &ctx.xgrammar_include_dir.join("xgrammar/xgrammar.h"),
        &rs_xgrammar_dir.join("xgrammar.h"),
    );

    let rs_dlpack_dir = rs_dir.join("dlpack");
    create_dir_all(&rs_dlpack_dir).ok();
    copy_if_changed(
        &ctx.dlpack_include_dir.join("dlpack/dlpack.h"),
        &rs_dlpack_dir.join("dlpack.h"),
    );
}

fn format_generated_bindings_optional(out_dir: &Path) {
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

fn main() {
    configure_libclang_windows();
    let build_context = collect_build_context();
    let destination_path = build_xgrammar_cmake(&build_context);
    link_xgrammar_static(&build_context, &destination_path);
    build_autocxx_bridge(&build_context);
    copy_headers_for_generated_rust_code(&build_context);
    format_generated_bindings_optional(&build_context.out_dir);
    strip_autocxx_generated_doc_comments(&build_context.out_dir);
    fix_broken_bindgen_type_aliases(&build_context.out_dir);
}
