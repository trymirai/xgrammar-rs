use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    Metal,
    Cpu,
}

impl Backend {
    pub fn name(self) -> String {
        match self {
            Backend::Metal => "metal".to_string(),
            Backend::Cpu => "cpu".to_string(),
        }
    }

    /// xgrammar-rs has no `backend-*` Cargo features — keep names for logging only.
    pub fn feature(self) -> Option<String> {
        None
    }
}
