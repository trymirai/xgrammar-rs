//! Downloads the upstream xgrammar release this port tracks into the gitignored
//! `<workspace>/external/xgrammar`, where its original Python tests run against this repository's
//! extension. An existing checkout of the pinned commit is reused, so later builds work offline.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const REPOSITORY: &str = "https://github.com/mlc-ai/xgrammar.git";
const TAG: &str = "v0.2.8";
const COMMIT: &str = "97787376faee5ed8466cfad57c99855e4ce2f6aa";

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let workspace = manifest_dir.ancestors().nth(2).expect("the crate lives in <workspace>/crates/");
    let checkout = workspace.join("external").join("xgrammar");
    // Runs again (and downloads again) when the checkout is removed or moved to another commit.
    println!("cargo::rerun-if-changed={}", checkout.join(".git").join("HEAD").display());
    println!("cargo::rustc-env=XGRAMMAR_UPSTREAM_DIR={}", checkout.display());
    if head(&checkout).as_deref() == Some(COMMIT) {
        return;
    }

    // Clone beside the destination and move it into place, so an interrupted download never
    // leaves a partial checkout at the path the tests use.
    let staging = checkout.with_extension(format!("download-{}", std::process::id()));
    let status = Command::new("git")
        .args(["clone", "--quiet", "--depth", "1", "--branch", TAG, REPOSITORY])
        .arg(&staging)
        .status()
        .expect("running git to download the upstream xgrammar release");
    assert!(status.success(), "git could not clone {REPOSITORY} at {TAG}");
    assert_eq!(head(&staging).as_deref(), Some(COMMIT), "{REPOSITORY} tag {TAG} no longer points at {COMMIT}");
    if checkout.exists() {
        fs::remove_dir_all(&checkout).expect("removing the outdated upstream checkout");
    }
    fs::rename(&staging, &checkout).expect("moving the upstream checkout into place");
}

fn head(repository: &Path) -> Option<String> {
    let output = Command::new("git").arg("-C").arg(repository).args(["rev-parse", "HEAD"]).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
