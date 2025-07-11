use super::Provider;
use crate::{Result, SecretSpecError};
use http::Uri;
use keyring::Entry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeyringConfig {}

impl KeyringConfig {
    pub fn from_uri(uri: &Uri) -> Result<Self> {
        let scheme = uri.scheme_str().ok_or_else(|| {
            SecretSpecError::ProviderOperationFailed("URI must have a scheme".to_string())
        })?;

        if scheme != "keyring" {
            return Err(SecretSpecError::ProviderOperationFailed(format!(
                "Invalid scheme '{}' for keyring provider",
                scheme
            )));
        }

        Ok(Self::default())
    }
}

pub struct KeyringProvider {
    _config: KeyringConfig,
}

impl KeyringProvider {
    pub fn new(config: KeyringConfig) -> Self {
        Self { _config: config }
    }

    pub fn from_uri(uri: &Uri) -> Result<Self> {
        let config = KeyringConfig::from_uri(uri)?;
        Ok(Self::new(config))
    }
}

impl Provider for KeyringProvider {
    fn get(&self, project: &str, key: &str, profile: &str) -> Result<Option<String>> {
        let service = format!("secretspec/{}/{}/{}", project, profile, key);

        let entry = Entry::new(&service, &whoami::username())?;
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn set(&self, project: &str, key: &str, value: &str, profile: &str) -> Result<()> {
        let service = format!("secretspec/{}/{}/{}", project, profile, key);

        let entry = Entry::new(&service, &whoami::username())?;
        entry.set_password(value)?;
        Ok(())
    }

    fn name(&self) -> &'static str {
        "keyring"
    }

    fn description(&self) -> &'static str {
        "Uses system keychain (Recommended)"
    }
}
