use std::{
    env,
    path::{Path, PathBuf},
};

#[path = "build/mod.rs"]
mod build;

/// wasm32-wasi* builds need a WASI sysroot with C++ exceptions support,
/// provided by the user via WASI_SYSROOT (cc-rs picks it up from there).
fn require_wasi_sysroot(target: &str) {
    let sysroot = match env::var("WASI_SYSROOT") {
        Ok(path) if !path.is_empty() && Path::new(&path).is_dir() => {
            PathBuf::from(path)
        }
        Ok(path) => panic!(
            "WASI_SYSROOT is set to '{path}', which is not an existing directory"
        ),
        Err(_) => panic!(
            "building for a wasm32-wasi* target requires a WASI sysroot with \
             C++ exceptions support. Set the WASI_SYSROOT environment variable \
             to its location, e.g. \
             WASI_SYSROOT=/opt/wasi-sdk/share/wasi-sysroot \
             (see the `WebAssembly support` section of README.md)"
        ),
    };

    // The C++ runtime must be at the standard sysroot locations so that
    // every C++-compiling crate finds it via plain --sysroot.
    let target_include = sysroot.join("include").join(target);
    if !target_include.join("c++/v1").is_dir() {
        if target_include.join("eh/c++/v1").is_dir() {
            panic!(
                "WASI_SYSROOT points at a dual (eh/noeh) sysroot as shipped by \
                 wasi-sdk >= 33, with the exception-enabled C++ runtime in \
                 `eh/` subdirectories. Stock clang does not select it \
                 automatically, so overlay the `eh` variant onto the standard \
                 sysroot locations (see the `WebAssembly support` section of \
                 README.md)"
            );
        }
        panic!(
            "WASI_SYSROOT points at {}, which does not contain C++ headers for \
             target {target} at include/{target}/c++/v1. A WASI sysroot with \
             C++ exceptions support is required (see the `WebAssembly support` \
             section of README.md)",
            sysroot.display()
        );
    }
}

fn main() {
    println!("cargo::rerun-if-changed=src/cxx_utils.hpp");
    println!("cargo::rerun-if-changed=src/cxx_utils/");
    println!("cargo::rerun-if-env-changed=WASI_SYSROOT");

    let target = env::var("TARGET").expect("cargo shall set TARGET");

    #[cfg(target_os = "windows")]
    build::windows::configure_libclang();
    let ctx = build::submodules::collect_build_context();

    let mut wasm_c_cxx_flags = vec![];
    if target.starts_with("wasm32-wasi") {
        require_wasi_sysroot(&target);
        // C++ exceptions flags per wasi-sdk docs (may change between releases):
        // https://github.com/WebAssembly/wasi-sdk/blob/wasi-sdk-33/CppExceptions.md#compiling-code-with-c-exceptions
        println!("cargo::rustc-link-arg=-fwasm-exceptions");
        println!("cargo::rustc-link-lib=static=unwind");
        wasm_c_cxx_flags =
            vec!["-fwasm-exceptions", "-mllvm", "-wasm-use-legacy-eh=false"];
    }

    let destination_path =
        build::xgrammar_cmake::build_xgrammar_cmake(&ctx, &wasm_c_cxx_flags);
    build::xgrammar_cmake::link_xgrammar_static(&ctx, &destination_path);

    let mut bridge_builder = cxx_build::bridge("src/lib.rs");

    let mut extra_compiler_flags = vec!["-std=c++17".to_string()];
    if !target.starts_with("wasm32-") {
        #[cfg(target_os = "windows")]
        extra_compiler_flags
            .extend(build::windows::target_clang_args(&ctx.target));
        #[cfg(target_os = "macos")]
        extra_compiler_flags
            .extend(build::macos::target_clang_args(&ctx.target));
        #[cfg(target_os = "linux")]
        extra_compiler_flags.extend(build::linux::clang_include_args(
            &ctx.target,
            bridge_builder.get_compiler().path(),
        ));
    }

    if build::common::is_truthy_env("XGRAMMAR_RS_DEBUG_INCLUDES") {
        println!(
            "cargo::warning=xgrammar-rs: extra_compiler_flags={}",
            extra_compiler_flags.join(" "),
        );
        println!(
            "cargo::warning=xgrammar-rs: wasm_c_cxx_flags={}",
            wasm_c_cxx_flags.join(" "),
        );
    }

    bridge_builder
        .include(ctx.src_include_dir)
        .include(ctx.xgrammar_include_dir)
        .include(ctx.xgrammar_src_dir)
        .include(ctx.dlpack_include_dir)
        .include(ctx.picojson_include_dir)
        .include(ctx.manifest_dir)
        .flags(wasm_c_cxx_flags)
        .flags(extra_compiler_flags);
    bridge_builder.compile("cxxbridge");
}
