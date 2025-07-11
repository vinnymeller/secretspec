use super::Provider;
use crate::{Result, SecretSpecError};
use http::Uri;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnvConfig {}

impl EnvConfig {
    pub fn from_uri(uri: &Uri) -> Result<Self> {
        let scheme = uri.scheme_str().ok_or_else(|| {
            SecretSpecError::ProviderOperationFailed("URI must have a scheme".to_string())
        })?;

        if scheme != "env" {
            return Err(SecretSpecError::ProviderOperationFailed(format!(
                "Invalid scheme '{}' for env provider",
                scheme
            )));
        }

        Ok(Self::default())
    }
}

pub struct EnvProvider {
    _config: EnvConfig,
}

impl EnvProvider {
    pub fn new(config: EnvConfig) -> Self {
        Self { _config: config }
    }

    pub fn from_uri(uri: &Uri) -> Result<Self> {
        let config = EnvConfig::from_uri(uri)?;
        Ok(Self::new(config))
    }
}

impl Provider for EnvProvider {
    fn get(&self, _project: &str, key: &str, _profile: &str) -> Result<Option<String>> {
        Ok(env::var(key).ok())
    }

    fn set(&self, _project: &str, _key: &str, _value: &str, _profile: &str) -> Result<()> {
        // Environment variables are read-only in this backend
        // Setting environment variables at runtime doesn't persist across processes
        Err(crate::SecretSpecError::ProviderOperationFailed(
            "Environment variable provider is read-only. Set variables in your shell or process environment.".to_string()
        ))
    }

    fn allows_set(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "env"
    }

    fn description(&self) -> &'static str {
        "Read-only environment variables"
    }
}
