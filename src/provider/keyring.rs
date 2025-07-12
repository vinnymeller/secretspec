use super::Provider;
use crate::{Result, SecretSpecError};
use http::Uri;
use keyring::Entry;
use serde::{Deserialize, Serialize};

/// Configuration for the keyring provider.
///
/// This struct holds configuration options for the keyring provider,
/// which stores secrets in the system's native keychain service.
/// Currently, no additional configuration is required as the provider
/// uses sensible defaults for all platforms.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeyringConfig {}

impl KeyringConfig {
    /// Creates a new KeyringConfig from a URI.
    ///
    /// The URI must have the scheme "keyring" (e.g., "keyring://").
    /// Currently, no additional parameters are supported in the URI.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI to parse, must have scheme "keyring"
    ///
    /// # Returns
    ///
    /// * `Ok(KeyringConfig)` - A new default configuration
    /// * `Err` - If the URI is invalid or has wrong scheme
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use http::Uri;
    /// # use secretspec::provider::keyring::KeyringConfig;
    /// let uri: Uri = "keyring://".parse().unwrap();
    /// let config = KeyringConfig::from_uri(&uri).unwrap();
    /// ```
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

/// Provider for storing secrets in the system keychain.
///
/// The KeyringProvider uses the operating system's native secure credential
/// storage mechanism:
/// - macOS: Keychain
/// - Windows: Credential Manager
/// - Linux: Secret Service API (via libsecret)
///
/// Secrets are stored with a hierarchical key structure:
/// `secretspec/{project}/{profile}/{key}`
///
/// This ensures secrets are properly namespaced by project and profile,
/// preventing conflicts between different projects or environments.
pub struct KeyringProvider {
    _config: KeyringConfig,
}

impl KeyringProvider {
    /// Creates a new KeyringProvider with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration for the keyring provider
    ///
    /// # Returns
    ///
    /// A new instance of KeyringProvider
    pub fn new(config: KeyringConfig) -> Self {
        Self { _config: config }
    }

    /// Creates a new KeyringProvider from a URI.
    ///
    /// This is a convenience method that parses the URI into a KeyringConfig
    /// and then creates the provider.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI to parse (e.g., "keyring://")
    ///
    /// # Returns
    ///
    /// * `Ok(KeyringProvider)` - A new provider instance
    /// * `Err` - If the URI is invalid
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use http::Uri;
    /// # use secretspec::provider::keyring::KeyringProvider;
    /// let uri: Uri = "keyring://".parse().unwrap();
    /// let provider = KeyringProvider::from_uri(&uri).unwrap();
    /// ```
    pub fn from_uri(uri: &Uri) -> Result<Self> {
        let config = KeyringConfig::from_uri(uri)?;
        Ok(Self::new(config))
    }
}

impl Provider for KeyringProvider {
    /// Retrieves a secret from the system keychain.
    ///
    /// The secret is looked up using a hierarchical key structure:
    /// `secretspec/{project}/{profile}/{key}`
    ///
    /// The current system username is used as the account identifier.
    ///
    /// # Arguments
    ///
    /// * `project` - The project name
    /// * `key` - The secret key to retrieve
    /// * `profile` - The profile/environment name
    ///
    /// # Returns
    ///
    /// * `Ok(Some(String))` - The secret value if found
    /// * `Ok(None)` - If the secret doesn't exist
    /// * `Err` - If there was an error accessing the keychain
    fn get(&self, project: &str, key: &str, profile: &str) -> Result<Option<String>> {
        let service = format!("secretspec/{}/{}/{}", project, profile, key);

        let entry = Entry::new(&service, &whoami::username())?;
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Stores a secret in the system keychain.
    ///
    /// The secret is stored with a hierarchical key structure:
    /// `secretspec/{project}/{profile}/{key}`
    ///
    /// The current system username is used as the account identifier.
    /// If a secret already exists with the same key, it will be overwritten.
    ///
    /// # Arguments
    ///
    /// * `project` - The project name
    /// * `key` - The secret key to store
    /// * `value` - The secret value to store
    /// * `profile` - The profile/environment name
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the secret was stored successfully
    /// * `Err` - If there was an error accessing the keychain
    fn set(&self, project: &str, key: &str, value: &str, profile: &str) -> Result<()> {
        let service = format!("secretspec/{}/{}/{}", project, profile, key);

        let entry = Entry::new(&service, &whoami::username())?;
        entry.set_password(value)?;
        Ok(())
    }

    /// Returns the name identifier for this provider.
    ///
    /// This is used for provider selection and configuration.
    fn name(&self) -> &'static str {
        "keyring"
    }

    /// Returns a human-readable description of this provider.
    ///
    /// This is displayed in help text and provider listings.
    fn description(&self) -> &'static str {
        "Uses system keychain (Recommended)"
    }
}
