use serde::{Deserialize, Serialize};

use crate::types::{Backend, Capability};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TargetConfig {
    /// Optional; defaults to CPU. xgrammar has no Metal/CPU Cargo features — used for
    /// documentation / future backends only.
    #[serde(default = "default_backend")]
    pub backend: Backend,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub capabilities_supported: Vec<Capability>,
    #[serde(default)]
    pub capabilities_default: Vec<Capability>,
    #[serde(default)]
    pub required_envs: Vec<String>,
}

fn default_backend() -> Backend {
    Backend::Cpu
}
