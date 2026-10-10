use serde::{Deserialize, Serialize};

use crate::types::Bindings;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LanguageMetadata {
    #[serde(default)]
    pub image_url: String,
    #[serde(default)]
    pub badges: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LanguageConfig {
    pub targets: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub bindings: Vec<Bindings>,
    pub examples_path: String,
    #[serde(default)]
    pub metadata: LanguageMetadata,
}
