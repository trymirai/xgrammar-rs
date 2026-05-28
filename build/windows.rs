use std::{env, path::PathBuf, process::Command};

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

    candidates.into_iter().find(|dir| dir.join("libclang.dll").exists())
}

pub fn configure_libclang() {
    if env::var("LIBCLANG_PATH").is_err() {
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

            // SAFETY: The function is always safe to call on Windows.
            unsafe { env::set_var("LIBCLANG_PATH", &chosen) };
            println!("cargo:rustc-env=LIBCLANG_PATH={}", chosen.display());
        }
    }
}

pub fn target_clang_args(target: &str) -> Vec<String> {
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
