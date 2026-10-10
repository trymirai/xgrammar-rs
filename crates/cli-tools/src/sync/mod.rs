use std::path::Path;

mod jsr;
mod license;
mod package_json;
mod pyproject;
mod readme;
mod swift_package;
mod toolchains;

use anyhow::{Ok, Result, anyhow};
pub use jsr::JsrSyncTask;
pub use license::LicenseSyncTask;
pub use package_json::PackageJsonSyncTask;
pub use pyproject::PyprojectSyncTask;
pub use readme::ReadmeSyncTask;
pub use swift_package::SwiftPackageSyncTask;
pub use toolchains::ToolchainsSyncTask;

use crate::configs::{Paths, PlatformsConfig, WorkspaceManifest};

pub trait SyncTask {
    fn process(
        &self,
        platforms: &PlatformsConfig,
        workspace: &WorkspaceManifest,
        input: &str,
    ) -> Result<String>;

    fn run(
        &self,
        platforms: &PlatformsConfig,
        workspace: &WorkspaceManifest,
        input_path: &Path,
        check: bool,
    ) -> Result<()> {
        if !input_path.exists() {
            eprintln!("skip missing sync target: {}", input_path.display());
            return Ok(());
        }
        let input = std::fs::read_to_string(input_path).unwrap_or_default();
        let output = self.process(platforms, workspace, &input)?;
        if check {
            if input != output {
                return Err(anyhow!("The file is out of sync: {}", input_path.display()));
            }
        } else {
            std::fs::write(input_path, output)?;
        }
        Ok(())
    }
}

pub fn run_sync(check: bool) -> Result<()> {
    use crate::types::Language;

    let paths = Paths::new()?;
    let platforms = PlatformsConfig::load()?;
    let workspace = WorkspaceManifest::load()?;
    let root_path = &paths.root_path;

    ToolchainsSyncTask.run(&platforms, &workspace, &root_path.join("rust-toolchain.toml"), check)?;

    // Optional README / binding metadata sync — skipped when templates or files are absent.
    if paths.readme_template_path().exists() {
        ReadmeSyncTask::new(vec![Language::Rust, Language::Python, Language::Swift, Language::TypeScript]).run(
            &platforms,
            &workspace,
            &root_path.join("README.md"),
            check,
        )?;
    }

    PyprojectSyncTask.run(&platforms, &workspace, &root_path.join("bindings/python/pyproject.toml"), check)?;
    LicenseSyncTask.run(&platforms, &workspace, &root_path.join("bindings/python/LICENSE"), check)?;

    SwiftPackageSyncTask.run(&platforms, &workspace, &paths.root_package_swift_path(), check)?;
    LicenseSyncTask.run(&platforms, &workspace, &root_path.join("bindings/swift/LICENSE"), check)?;

    PackageJsonSyncTask.run(&platforms, &workspace, &root_path.join("bindings/typescript/package.json"), check)?;
    JsrSyncTask.run(&platforms, &workspace, &root_path.join("bindings/typescript/jsr.json"), check)?;
    LicenseSyncTask.run(&platforms, &workspace, &root_path.join("bindings/typescript/LICENSE"), check)?;

    Ok(())
}
