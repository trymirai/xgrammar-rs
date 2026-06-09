#[path = "build/mod.rs"]
mod build;

fn main() {
    println!("cargo::rerun-if-changed=src/cxx_utils.hpp");
    println!("cargo::rerun-if-changed=src/cxx_utils/");

    #[cfg(target_os = "windows")]
    build::windows::configure_libclang();
    let ctx = build::submodules::collect_build_context();
    let destination_path = build::cmake::build_xgrammar_cmake(&ctx);
    build::cmake::link_xgrammar_static(&ctx, &destination_path);

    let mut bridge_builder = cxx_build::bridge("src/lib.rs");

    let mut extra_clang_args = vec!["-std=c++17".to_string()];
    if !std::env::var("TARGET").expect("TARGET is unset").starts_with("wasm32-")
    {
        #[cfg(target_os = "windows")]
        extra_clang_args.extend(build::windows::target_clang_args(&ctx.target));
        #[cfg(target_os = "macos")]
        extra_clang_args.extend(build::macos::target_clang_args(&ctx.target));
        #[cfg(target_os = "linux")]
        extra_clang_args.extend(build::linux::clang_include_args(
            &ctx.target,
            bridge_builder.get_compiler().path(),
        ));
    }

    if build::common::is_truthy_env("XGRAMMAR_RS_DEBUG_INCLUDES") {
        println!(
            "cargo::warning=xgrammar-rs: extra_clang_args={}",
            extra_clang_args.join(" "),
        );
    }

    bridge_builder
        .include(ctx.src_include_dir)
        .include(ctx.xgrammar_include_dir)
        .include(ctx.xgrammar_src_dir)
        .include(ctx.dlpack_include_dir)
        .include(ctx.picojson_include_dir)
        .include(ctx.manifest_dir)
        .flags(extra_clang_args);
    bridge_builder.compile("cxxbridge");
}
