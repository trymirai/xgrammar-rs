use std::{
    env,
    path::{Path, PathBuf},
};

use super::{
    BuildContext,
    common::{abs_path, looks_like_xgrammar_repo_root},
};

pub fn ensure_xgrammar_source_tree(manifest_dir: &Path) -> PathBuf {
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

pub fn collect_build_context() -> BuildContext {
    println!("cargo::rerun-if-env-changed=XGRAMMAR_SRC_DIR");

    let manifest_dir = abs_path(
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"),
    );
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));

    println!(
        "cargo::rerun-if-changed={}",
        manifest_dir.join(".gitmodules").display()
    );

    let xgrammar_src_dir = ensure_xgrammar_source_tree(&manifest_dir);
    println!(
        "cargo::rerun-if-changed={}",
        xgrammar_src_dir.join("include").display()
    );
    println!("cargo::rerun-if-changed={}/cpp", xgrammar_src_dir.display());
    println!("cargo::rerun-if-changed={}/3rdparty", xgrammar_src_dir.display());
    println!(
        "cargo::rerun-if-changed={}",
        xgrammar_src_dir.join("CMakeLists.txt").display()
    );
    println!(
        "cargo::rerun-if-changed={}",
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
