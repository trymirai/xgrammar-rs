use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Grammar,
    #[serde(rename = "cli")]
    CLI,
}

impl Capability {
    /// xgrammar-rs has no `capability-*` Cargo features — keep enum for CLI compatibility.
    pub fn feature(&self) -> Option<String> {
        None
    }
}
