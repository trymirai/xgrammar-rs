use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};

use crate::{
    configs::{ALL_TARGET, Paths, PlatformsConfig},
    languages::{LanguageBackend, LanguageBackendTarget},
    types::{Capability, Command, Configuration, Language},
    utilities::fs::copy_directory,
};

pub struct SwiftLanguageBackend {
    config: PlatformsConfig,
}

impl SwiftLanguageBackend {
    pub fn new(config: PlatformsConfig) -> Self {
        Self {
            config,
        }
    }
}

impl LanguageBackend for SwiftLanguageBackend {
    fn config(&self) -> PlatformsConfig {
        self.config.clone()
    }

    fn language(&self) -> Language {
        Language::Swift
    }

    fn build_targets(
        &self,
        configuration: Configuration,
        targets: Vec<LanguageBackendTarget>,
    ) -> Result<()> {
        let paths = Paths::new()?;
        let crate_path = paths.crate_path(&paths.bindings_crate);
        let xcframework_path = paths.swift_xcframework_path();
        let generated_sources_path = paths.swift_generated_sources_path();

        let slices_path = paths.swift_slices_path();
        let output_path = crate_path.join(&paths.bindings_lib);

        if slices_path.exists() {
            fs::remove_dir_all(&slices_path)?;
        }
        fs::create_dir_all(&slices_path)?;

        for target in targets.iter() {
            if output_path.exists() {
                fs::remove_dir_all(&output_path)?;
            }

            Command::cargo_swift_package(
                paths.bindings_lib.clone(),
                target.name.clone(),
                target.features.clone(),
                configuration,
            )
            .with_current_path(&crate_path)
            .with_envs(self.config.required_envs_for_target(target.name.clone())?)
            .run()?;

            let slice_dir = slices_path.join(&target.name);
            fs::rename(&output_path, &slice_dir).context("Moving cargo-swift output to slice dir")?;
        }

        if xcframework_path.exists() {
            fs::remove_dir_all(&xcframework_path)?;
        }

        // Prefer merging multi-slice static libs when available; otherwise copy the
        // single cargo-swift FFI xcframework (typical for a host-only build).
        match collect_slice_libs_with_headers(&slices_path) {
            Result::Ok(slice_libs_with_headers) if slice_libs_with_headers.len() > 1 => {
                Command::xcodebuild_create_xcframework(slice_libs_with_headers, xcframework_path.clone()).run()?;
                Command::codesign_adhoc(xcframework_path.clone()).run()?;
            },
            Result::Ok(_) | Err(_) => {
                let any_slice = fs::read_dir(&slices_path)?.next().context("No slices produced")??.path();
                let ffi = find_ffi_xcframework(&any_slice)?;
                copy_directory(&ffi, &xcframework_path)?;
                let _ = Command::codesign_adhoc(xcframework_path.clone()).run();
            },
        }

        let any_slice_path = fs::read_dir(&slices_path)?.next().context("No slices produced")??;
        let sources_path = any_slice_path.path().join("Sources").join(&paths.bindings_lib);
        if generated_sources_path.exists() {
            // Keep hand-written XGrammar.swift; refresh generated UniFFI sources.
            for entry in fs::read_dir(&generated_sources_path)? {
                let entry = entry?;
                let path = entry.path();
                if path.file_name().and_then(|n| n.to_str()) == Some("XGrammar.swift") {
                    continue;
                }
                if path.is_file() {
                    fs::remove_file(&path)?;
                }
            }
        } else {
            fs::create_dir_all(&generated_sources_path)?;
        }
        if sources_path.exists() {
            for entry in fs::read_dir(&sources_path)? {
                let entry = entry?;
                let dest = generated_sources_path.join(entry.file_name());
                fs::copy(entry.path(), dest)?;
            }
        }

        Ok(())
    }

    fn test_target(
        &self,
        _configuration: Configuration,
        _target: LanguageBackendTarget,
    ) -> Result<()> {
        let paths = Paths::new()?;
        Command::swift_test().with_current_path(&paths.root_path).run()
    }

    fn example_target(
        &self,
        name: &str,
        _configuration: Configuration,
        _target: LanguageBackendTarget,
    ) -> Result<()> {
        let paths = Paths::new()?;
        let name = self.language().convert_command_name(name);
        Command::swift_run_example(name).with_current_path(&paths.root_path).run()
    }

    fn release(
        &self,
        _version: &str,
    ) -> Result<()> {
        self.build(Configuration::Release, vec![ALL_TARGET.to_string()], Vec::<Capability>::new())?;
        let paths = Paths::new()?;
        let xcframework_path = paths.swift_xcframework_path();
        if !xcframework_path.exists() {
            anyhow::bail!("Missing xcframework at {}", xcframework_path.display());
        }
        let spm_root = paths.release_swift_spm_path();
        fs::create_dir_all(&spm_root)?;
        Ok(())
    }
}

fn find_ffi_xcframework(slice_path: &Path) -> Result<PathBuf> {
    for entry in fs::read_dir(slice_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("xcframework") {
            return Ok(path);
        }
    }
    Err(anyhow!("No .xcframework found in {}", slice_path.display()))
}

fn collect_slice_libs_with_headers(slices_path: &Path) -> Result<Vec<(PathBuf, PathBuf)>> {
    let mut results = Vec::new();
    for entry in fs::read_dir(slices_path)? {
        let entry = entry?;
        let ffi = match find_ffi_xcframework(&entry.path()) {
            Result::Ok(path) => path,
            Err(_) => continue,
        };
        for entry in fs::read_dir(&ffi)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let static_lib_path = find_static_lib(&path)?;
            let headers_path = find_modulemap_directory(&path.join("Headers"))?;
            results.push((static_lib_path, headers_path));
        }
    }
    if results.is_empty() {
        return Err(anyhow!("No slices produced"));
    }
    Ok(results)
}

fn find_modulemap_directory(headers_path: &Path) -> Result<PathBuf> {
    if headers_path.join("module.modulemap").exists() {
        return Ok(headers_path.to_path_buf());
    }
    for entry in fs::read_dir(headers_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && path.join("module.modulemap").exists() {
            return Ok(path);
        }
    }
    Err(anyhow!("module.modulemap not found in {}", headers_path.display()))
}

fn find_static_lib(slice_path: &Path) -> Result<PathBuf> {
    fn walk(path: &Path) -> Option<PathBuf> {
        let entries = fs::read_dir(path).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|extension| extension.to_str()) == Some("a") {
                return Some(path);
            }
            if path.is_dir() {
                if let Some(found) = walk(&path) {
                    return Some(found);
                }
            }
        }
        None
    }
    walk(slice_path).context("Static lib (.a) not found in the slice")
}
