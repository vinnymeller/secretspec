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

        // For dotenv URIs, we want to handle paths specially
        // The URI might be in the form:
        // - dotenv://localhost/absolute/path
        // - dotenv://localhost (default .env)
        // - dotenv:relative/path (gets normalized with authority)

        let path = if uri.authority().is_some()
            && uri.authority().map(|a| a.host()) == Some("localhost")
        {
            // URI was normalized with localhost authority
            let uri_path = uri.path();
            if uri_path.is_empty() || uri_path == "/" {
                ".env"
            } else if uri_path.starts_with("/./") {
                // Handle relative paths that were normalized with leading /
                &uri_path[1..]
            } else {
                // Path from URI with authority always starts with /
                uri_path
            }
        } else {
            // No authority or non-localhost authority, use path as-is
            let uri_path = uri.path();
            if uri_path.is_empty() {
                ".env"
            } else {
                uri_path
            }
        };

        Ok(Self {
            path: PathBuf::from(path),
        })
    }

    /// Create a DotEnvConfig directly from a path string
    /// This is useful when we have a plain file path without URI parsing
    pub fn from_path_string(path: &str) -> Self {
        Self {
            path: PathBuf::from(if path.is_empty() { ".env" } else { path }),
        }
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
}

impl Provider for DotEnvProvider {
    fn get(&self, _project: &str, key: &str, _profile: Option<&str>) -> Result<Option<String>> {
        if !self.config.path.exists() {
            return Ok(None);
        }

        // Use dotenvy for reading to ensure compatibility
        let mut vars = HashMap::new();
        let env_vars = dotenvy::from_path_iter(&self.config.path)?;
        for item in env_vars {
            let (k, v) = item?;
            vars.insert(k, v);
        }

        Ok(vars.get(key).cloned())
    }

    fn set(&self, _project: &str, key: &str, value: &str, _profile: Option<&str>) -> Result<()> {
        // Load existing vars using dotenvy
        let mut vars = HashMap::new();
        if self.config.path.exists() {
            let env_vars = dotenvy::from_path_iter(&self.config.path)?;
            for item in env_vars {
                let (k, v) = item?;
                vars.insert(k, v);
            }
        }

        // Update the value
        vars.insert(key.to_string(), value.to_string());

        // Save back to file using serde-envfile for proper escaping
        let content = serde_envfile::to_string(&vars).map_err(|e| {
            SecretSpecError::ProviderOperationFailed(format!(
                "Failed to serialize .env file: {}",
                e
            ))
        })?;

        fs::write(&self.config.path, content)?;
        Ok(())
    }

    fn name(&self) -> &'static str {
        "dotenv"
    }

    fn description(&self) -> &'static str {
        "Traditional .env files"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dotenv_absolute_path() {
        // Test with absolute path - should preserve the leading slash
        let uri = "dotenv://localhost/tmp/test/.env".parse::<Uri>().unwrap();
        let config = DotEnvConfig::from_uri(&uri).unwrap();
        assert_eq!(config.path.to_str().unwrap(), "/tmp/test/.env");

        // Test with relative path
        let uri = "dotenv://localhost/./test/.env".parse::<Uri>().unwrap();
        let config = DotEnvConfig::from_uri(&uri).unwrap();
        assert_eq!(config.path.to_str().unwrap(), "./test/.env");

        // Test with default path
        let uri = "dotenv://localhost".parse::<Uri>().unwrap();
        let config = DotEnvConfig::from_uri(&uri).unwrap();
        assert_eq!(config.path.to_str().unwrap(), ".env");
    }
}
