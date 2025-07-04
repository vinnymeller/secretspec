use super::StorageBackend;
use crate::Result;
use std::env;

pub struct EnvStorage;

impl EnvStorage {
    pub fn new() -> Self {
        Self
    }
}

impl StorageBackend for EnvStorage {
    fn get(&self, _project: &str, key: &str) -> Result<Option<String>> {
        Ok(env::var(key).ok())
    }

    fn set(&self, _project: &str, _key: &str, _value: &str) -> Result<()> {
        // Environment variables are read-only in this backend
        // Setting environment variables at runtime doesn't persist across processes
        Err(crate::SecretSpecError::StorageOperationFailed(
            "Environment variable storage is read-only. Set variables in your shell or process environment.".to_string()
        ))
    }
}
