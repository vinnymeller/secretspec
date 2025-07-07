use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectInfo,
    pub profiles: HashMap<String, ProfileConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    #[serde(default = "default_revision")]
    pub revision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<Vec<String>>,
}

fn default_revision() -> String {
    "1.0".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileConfig {
    #[serde(flatten)]
    pub secrets: HashMap<String, SecretConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SecretConfig {
    pub description: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub defaults: DefaultConfig,
    #[serde(default)]
    pub projects: HashMap<String, ProjectUserConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DefaultConfig {
    pub provider: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectUserConfig {
    pub provider: String,
}
