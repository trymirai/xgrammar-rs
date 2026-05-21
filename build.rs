#[path = "build/mod.rs"]
mod build;

fn main() {
    #[cfg(target_os = "windows")]
    build::windows::configure_libclang();
    let build_context = build::submodules::collect_build_context();
    let destination_path = build::cmake::build_xgrammar_cmake(&build_context);
    build::cmake::link_xgrammar_static(&build_context, &destination_path);
    build::autocxx::build_autocxx_bridge(&build_context);
    build::autocxx::copy_headers_for_generated_rust_code(&build_context);
    build::autocxx::format_generated_bindings_optional(&build_context.out_dir);
    build::autocxx::strip_autocxx_generated_doc_comments(
        &build_context.out_dir,
    );
    build::autocxx::fix_broken_bindgen_type_aliases(&build_context.out_dir);
}
