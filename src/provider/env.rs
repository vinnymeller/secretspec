use super::Provider;
use crate::Result;
use std::env;

pub struct EnvStorage;

impl EnvStorage {
    pub fn new() -> Self {
        Self
    }
}

impl Provider for EnvStorage {
    fn get(&self, _project: &str, key: &str, _profile: Option<&str>) -> Result<Option<String>> {
        Ok(env::var(key).ok())
    }

    fn set(&self, _project: &str, _key: &str, _value: &str, _profile: Option<&str>) -> Result<()> {
        // Environment variables are read-only in this backend
        // Setting environment variables at runtime doesn't persist across processes
        Err(crate::SecretSpecError::ProviderOperationFailed(
            "Environment variable provider is read-only. Set variables in your shell or process environment.".to_string()
        ))
    }
}
