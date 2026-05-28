pub mod autocxx;
pub mod cmake;
pub mod common;
pub mod submodules;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct BuildContext {
    pub manifest_dir: PathBuf,
    pub xgrammar_src_dir: PathBuf,
    pub out_dir: PathBuf,

    pub src_include_dir: PathBuf,
    pub xgrammar_include_dir: PathBuf,
    pub dlpack_include_dir: PathBuf,
    pub picojson_include_dir: PathBuf,

    pub target: String,
}
