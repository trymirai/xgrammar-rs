use std::env;

#[path = "build/mod.rs"]
mod build;

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
        build::wasi_sysroot::find_or_build_and_set();
        // Flags for C++ exceptions to work correctly with wasi-sdk:
        // https://github.com/WebAssembly/wasi-sdk/blob/3a57aa06289ee679a62119d8842ca9ee7a4e5ee9/CppExceptions.md#compiling-code-with-c-exceptions
        // The set of flags may change for future wasi-sdk releases.
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
