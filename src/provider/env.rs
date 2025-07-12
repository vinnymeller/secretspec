use super::Provider;
use crate::{Result, SecretSpecError};
use http::Uri;
use serde::{Deserialize, Serialize};
use std::env;

/// Configuration for the environment variables provider.
///
/// This struct represents the configuration for the read-only environment
/// variables provider. It contains no fields as the provider reads directly
/// from the process environment without additional configuration.
///
/// # Example
///
/// ```
/// # use secretspec::provider::env::EnvConfig;
/// let config = EnvConfig::default();
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnvConfig {}

impl EnvConfig {
    /// Creates an `EnvConfig` from a URI.
    ///
    /// This method validates that the URI has the correct scheme ("env")
    /// and returns an `EnvConfig` instance. The environment provider
    /// doesn't require any additional configuration from the URI.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI to parse, must have scheme "env"
    ///
    /// # Returns
    ///
    /// * `Ok(EnvConfig)` - If the URI is valid with "env" scheme
    /// * `Err(SecretSpecError)` - If the URI is invalid or has wrong scheme
    ///
    /// # Example
    ///
    /// ```
    /// # use http::Uri;
    /// # use secretspec::provider::env::EnvConfig;
    /// let uri = "env://".parse::<Uri>().unwrap();
    /// let config = EnvConfig::from_uri(&uri).unwrap();
    /// ```
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

/// A read-only provider that reads secrets from environment variables.
///
/// The `EnvProvider` reads secrets directly from the process environment
/// variables. This provider is **read-only** and cannot persist values
/// across process boundaries. Attempts to set values will return an error.
///
/// # Read-only Nature
///
/// This provider is intentionally read-only because:
/// - Environment variables set at runtime only affect the current process
/// - Changes don't persist after the process exits
/// - Child processes inherit a copy of the parent's environment
///
/// To set environment variables, use your shell, process manager, or
/// container orchestration system.
///
/// # Example
///
/// ```
/// # use secretspec::provider::env::{EnvProvider, EnvConfig};
/// let provider = EnvProvider::new(EnvConfig::default());
/// // Can only read values, not set them
/// ```
pub struct EnvProvider {
    _config: EnvConfig,
}

impl EnvProvider {
    /// Creates a new `EnvProvider` with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration for the provider (currently unused)
    ///
    /// # Example
    ///
    /// ```
    /// # use secretspec::provider::env::{EnvProvider, EnvConfig};
    /// let config = EnvConfig::default();
    /// let provider = EnvProvider::new(config);
    /// ```
    pub fn new(config: EnvConfig) -> Self {
        Self { _config: config }
    }

    /// Creates an `EnvProvider` from a URI.
    ///
    /// This is a convenience method that parses the URI into an `EnvConfig`
    /// and then creates the provider.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI to parse, must have scheme "env"
    ///
    /// # Returns
    ///
    /// * `Ok(EnvProvider)` - If the URI is valid
    /// * `Err(SecretSpecError)` - If the URI is invalid
    ///
    /// # Example
    ///
    /// ```
    /// # use http::Uri;
    /// # use secretspec::provider::env::EnvProvider;
    /// let uri = "env://".parse::<Uri>().unwrap();
    /// let provider = EnvProvider::from_uri(&uri).unwrap();
    /// ```
    pub fn from_uri(uri: &Uri) -> Result<Self> {
        let config = EnvConfig::from_uri(uri)?;
        Ok(Self::new(config))
    }
}

impl Provider for EnvProvider {
    /// Retrieves a secret value from environment variables.
    ///
    /// This method reads the value directly from the process environment
    /// using the provided key. The project and profile parameters are
    /// ignored as environment variables are global to the process.
    ///
    /// # Arguments
    ///
    /// * `_project` - Project name (ignored)
    /// * `key` - The environment variable name to read
    /// * `_profile` - Profile name (ignored)
    ///
    /// # Returns
    ///
    /// * `Ok(Some(String))` - If the environment variable exists
    /// * `Ok(None)` - If the environment variable doesn't exist
    /// * `Err` - Never returns an error in practice
    ///
    /// # Example
    ///
    /// ```
    /// # use secretspec::provider::{Provider, env::{EnvProvider, EnvConfig}};
    /// # std::env::set_var("MY_SECRET", "value123");
    /// let provider = EnvProvider::new(EnvConfig::default());
    /// let value = provider.get("myproject", "MY_SECRET", "production").unwrap();
    /// assert_eq!(value, Some("value123".to_string()));
    /// ```
    fn get(&self, _project: &str, key: &str, _profile: &str) -> Result<Option<String>> {
        Ok(env::var(key).ok())
    }

    /// Attempts to set a secret value (always fails).
    ///
    /// This method always returns an error because the environment provider
    /// is read-only. Environment variables set at runtime don't persist
    /// across process boundaries and would create confusing behavior.
    ///
    /// # Arguments
    ///
    /// * `_project` - Project name (ignored)
    /// * `_key` - Environment variable name (ignored)
    /// * `_value` - Value to set (ignored)
    /// * `_profile` - Profile name (ignored)
    ///
    /// # Returns
    ///
    /// Always returns `Err(SecretSpecError::ProviderOperationFailed)` with
    /// an explanatory message about the read-only nature of this provider.
    ///
    /// # Example
    ///
    /// ```
    /// # use secretspec::provider::{Provider, env::{EnvProvider, EnvConfig}};
    /// let provider = EnvProvider::new(EnvConfig::default());
    /// let result = provider.set("myproject", "MY_SECRET", "value", "production");
    /// assert!(result.is_err());
    /// ```
    fn set(&self, _project: &str, _key: &str, _value: &str, _profile: &str) -> Result<()> {
        // Environment variables are read-only in this backend
        // Setting environment variables at runtime doesn't persist across processes
        Err(crate::SecretSpecError::ProviderOperationFailed(
            "Environment variable provider is read-only. Set variables in your shell or process environment.".to_string()
        ))
    }

    /// Indicates whether this provider supports setting values.
    ///
    /// Always returns `false` for the environment provider since it's
    /// a read-only provider. This allows the CLI and other consumers
    /// to check capabilities before attempting operations.
    ///
    /// # Returns
    ///
    /// Always returns `false`
    fn allows_set(&self) -> bool {
        false
    }

    /// Returns the name identifier for this provider.
    ///
    /// # Returns
    ///
    /// Always returns `"env"`
    fn name(&self) -> &'static str {
        "env"
    }

    /// Returns a human-readable description of this provider.
    ///
    /// # Returns
    ///
    /// Always returns `"Read-only environment variables"`
    fn description(&self) -> &'static str {
        "Read-only environment variables"
    }
}
