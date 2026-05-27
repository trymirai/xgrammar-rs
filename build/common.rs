use std::{
    env,
    fs::{self, create_dir_all},
    path::{Path, PathBuf},
};

use walkdir::WalkDir;

pub fn abs_path<P: AsRef<Path>>(p: P) -> PathBuf {
    if p.as_ref().is_absolute() {
        p.as_ref().to_path_buf()
    } else {
        env::current_dir().expect("current_dir failed").join(p)
    }
}

pub fn looks_like_xgrammar_repo_root(dir: &Path) -> bool {
    dir.join("CMakeLists.txt").exists()
        && dir.join("include").exists()
        && dir.join("cpp").exists()
}

pub fn is_truthy_env(name: &str) -> bool {
    let Ok(v) = env::var(name) else {
        return false;
    };
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

pub fn write_if_changed(
    path: &Path,
    contents: &[u8],
) {
    if let Ok(existing) = fs::read(path) {
        if existing == contents {
            return;
        }
    }
    if let Some(parent) = path.parent() {
        create_dir_all(parent).ok();
    }
    fs::write(path, contents).unwrap_or_else(|e| {
        panic!("Failed to write {}: {}", path.display(), e)
    });
}

pub fn find_xgrammar_lib_dir(root: &Path) -> Option<PathBuf> {
    let static_candidates = ["libxgrammar.a", "xgrammar.lib"];

    for entry in
        WalkDir::new(root).max_depth(6).into_iter().filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let name = entry.file_name().to_string_lossy();
        if static_candidates.iter().any(|c| name == *c) {
            return entry.path().parent().map(|p| p.to_path_buf());
        }
    }

    None
}
