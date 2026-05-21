use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn parse_include_search_list(stderr: &str) -> Vec<String> {
    let mut includes = Vec::new();
    let mut in_section = false;
    for line in stderr.lines() {
        if line.contains("#include <...> search starts here:") {
            in_section = true;
            continue;
        }
        if in_section {
            if line.contains("End of search list") {
                break;
            }
            let trimmed = line.trim();
            if !trimmed.is_empty() && trimmed.starts_with('/') {
                includes.push(trimmed.to_string());
            }
        }
    }
    includes
}

fn normalize_include_path(path: &str) -> String {
    let p = Path::new(path);
    if p.is_absolute() {
        match fs::canonicalize(p) {
            Ok(p) => p.display().to_string(),
            Err(_) => path.to_string(),
        }
    } else {
        path.to_string()
    }
}

fn gcc_multiarch_triple(target: &str) -> Option<String> {
    if let Ok(out) = Command::new("gcc").arg("-print-multiarch").output() {
        if let Ok(triple) = String::from_utf8(out.stdout) {
            let t = triple.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    if !target.is_empty() {
        return Some(target.to_string());
    }
    None
}

fn gcc_version() -> Option<String> {
    if let Ok(out) = Command::new("gcc").arg("-dumpversion").output() {
        if let Ok(v) = String::from_utf8(out.stdout) {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn probe_compiler_includes(
    compiler: &str,
    target: &str,
) -> Vec<String> {
    let mut args = vec!["-E", "-x", "c++", "-", "-v"];
    if !target.is_empty() {
        args.push("-target");
        args.push(target);
    }

    let output = Command::new(compiler)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output();

    match output {
        Ok(out) => {
            parse_include_search_list(&String::from_utf8_lossy(&out.stderr))
                .into_iter()
                .map(|p| normalize_include_path(&p))
                .collect()
        },
        Err(_) => Vec::new(),
    }
}

fn fallback_include_paths(target: &str) -> Vec<String> {
    let mut paths = Vec::new();

    let mut libc_dirs = Vec::new();
    if let Some(triple) = gcc_multiarch_triple(target) {
        libc_dirs.push(format!("/usr/include/{}", triple));
    }
    libc_dirs.push("/usr/include".to_string());
    libc_dirs.push("/usr/local/include".to_string());

    if let (Some(triple), Some(version)) =
        (gcc_multiarch_triple(target), gcc_version())
    {
        paths.push(format!("/usr/lib/gcc/{}/{}/include", triple, version));
        paths
            .push(format!("/usr/lib/gcc/{}/{}/include-fixed", triple, version));
        paths.push(format!("/usr/include/c++/{}", version));
        paths.push(format!("/usr/include/{}/c++/{}", triple, version));
        paths.extend(libc_dirs.into_iter());
    } else {
        paths.extend(libc_dirs.into_iter());
    }

    if let Ok(out) = Command::new("gcc").arg("-print-libgcc-file-name").output()
    {
        if let Ok(path) = String::from_utf8(out.stdout) {
            let p = PathBuf::from(path.trim());
            if let Some(include_dir) =
                p.parent().and_then(|p| p.parent()).map(|p| p.join("include"))
            {
                paths.push(normalize_include_path(
                    &include_dir.display().to_string(),
                ));
            }
        }
    }

    paths
}

fn collect_system_include_args(target: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut seen = HashSet::new();

    let mut compilers = vec!["clang++", "g++", "gcc", "c++", "cc"];
    if let Ok(cxx) = env::var("CXX") {
        compilers.insert(0, Box::leak(cxx.into_boxed_str()));
    }

    for compiler in compilers {
        let includes = probe_compiler_includes(compiler, target);
        if !includes.is_empty() {
            for path in includes {
                if seen.insert(path.clone()) {
                    args.push(format!(
                        "-isystem{}",
                        normalize_include_path(&path)
                    ));
                }
            }
        }
    }

    // Always add a conservative fallback set to ensure glibc headers are visible,
    // even if probing succeeded but missed multiarch include dirs.
    for path in fallback_include_paths(target) {
        if seen.insert(path.clone()) {
            args.push(format!("-isystem{}", normalize_include_path(&path)));
        }
    }

    args
}

// Platform-specific args for Linux
pub fn clang_include_args(target: &str) -> Vec<String> {
    let mut args = vec!["--sysroot=/".to_string()];
    args.extend(collect_system_include_args(target));
    args
}
