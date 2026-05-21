use std::{
    env,
    fs::{self, create_dir_all},
    path::Path,
    process::Command,
};

use super::BuildContext;
use super::common::{copy_if_changed, is_truthy_env};

pub fn build_autocxx_bridge(ctx: &BuildContext) {
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/cxx_utils.hpp");
    println!("cargo:rerun-if-changed=src/cxx_utils");

    let mut extra_clang_args = vec!["-std=c++17".to_string()];
    #[cfg(target_os = "windows")]
    extra_clang_args.extend(super::windows::target_clang_args(&ctx.target));
    #[cfg(target_os = "macos")]
    extra_clang_args.extend(super::macos::target_clang_args(&ctx.target));
    #[cfg(target_os = "linux")]
    extra_clang_args.extend(super::linux::clang_include_args(&ctx.target));
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
        [
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

pub fn copy_headers_for_generated_rust_code(ctx: &BuildContext) {
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

pub fn format_generated_bindings_optional(out_dir: &Path) {
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

pub fn strip_autocxx_generated_doc_comments(out_dir: &Path) {
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

pub fn fix_broken_bindgen_type_aliases(out_dir: &Path) {
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
