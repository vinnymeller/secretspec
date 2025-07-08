use super::Provider;
use crate::{Result, SecretSpecError};
use http::Uri;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DotEnvConfig {
    pub path: PathBuf,
}

impl Default for DotEnvConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from(".env"),
        }
    }
}

impl DotEnvConfig {
    pub fn from_uri(uri: &Uri) -> Result<Self> {
        let scheme = uri.scheme_str().ok_or_else(|| {
            SecretSpecError::ProviderOperationFailed("URI must have a scheme".to_string())
        })?;

        if scheme != "dotenv" {
            return Err(SecretSpecError::ProviderOperationFailed(format!(
                "Invalid scheme '{}' for dotenv provider",
                scheme
            )));
        }

        // Extract path from URI, default to .env if not specified
        let path = uri.path().trim_start_matches('/');
        let path = if path.is_empty() || path == "/" {
            ".env"
        } else {
            path
        };

        Ok(Self {
            path: PathBuf::from(path),
        })
    }
}

pub struct DotEnvProvider {
    config: DotEnvConfig,
}

impl DotEnvProvider {
    pub fn new(config: DotEnvConfig) -> Self {
        Self { config }
    }

    pub fn from_uri(uri: &Uri) -> Result<Self> {
        let config = DotEnvConfig::from_uri(uri)?;
        Ok(Self::new(config))
    }

    fn load_env_vars(&self) -> Result<HashMap<String, String>> {
        if !self.config.path.exists() {
            return Ok(HashMap::new());
        }

        let mut vars = HashMap::new();
        let env_vars = dotenvy::from_path_iter(&self.config.path)?;
        for item in env_vars {
            let (key, value) = item?;
            vars.insert(key, value);
        }
        Ok(vars)
    }

    fn save_env_vars(&self, vars: &HashMap<String, String>) -> Result<()> {
        let mut content = String::new();
        for (key, value) in vars {
            content.push_str(&format!("{}={}\n", key, value));
        }
        fs::write(&self.config.path, content)?;
        Ok(())
    }
}

impl Provider for DotEnvProvider {
    fn get(&self, _project: &str, key: &str, _profile: Option<&str>) -> Result<Option<String>> {
        let vars = self.load_env_vars()?;
        Ok(vars.get(key).cloned())
    }

    fn set(&self, _project: &str, key: &str, value: &str, _profile: Option<&str>) -> Result<()> {
        let mut vars = self.load_env_vars()?;
        vars.insert(key.to_string(), value.to_string());
        self.save_env_vars(&vars)
    }

    fn name(&self) -> &'static str {
        "dotenv"
    }

    fn description(&self) -> &'static str {
        "Traditional .env files"
    }
}
