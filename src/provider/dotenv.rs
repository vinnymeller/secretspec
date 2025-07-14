use super::Provider;
use crate::{Result, SecretSpecError};
use http::Uri;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Configuration for the dotenv provider.
///
/// This struct holds the configuration for accessing .env files,
/// primarily the path to the .env file to read from and write to.
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
/// use secretspec::provider::dotenv::DotEnvConfig;
///
/// let config = DotEnvConfig {
///     path: PathBuf::from(".env.production"),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DotEnvConfig {
    /// Path to the .env file.
    ///
    /// Can be either an absolute path (e.g., `/etc/secrets/.env`)
    /// or a relative path (e.g., `.env`, `config/.env.local`).
    pub path: PathBuf,
}

impl Default for DotEnvConfig {
    /// Creates a default configuration with path set to `.env`.
    ///
    /// This is the conventional default location for dotenv files
    /// in the current working directory.
    fn default() -> Self {
        Self {
            path: PathBuf::from(".env"),
        }
    }
}

impl DotEnvConfig {
    /// Creates a DotEnvConfig from a URI.
    ///
    /// Parses a URI in the format `dotenv://[host]/[path]` to extract
    /// the path to the .env file. The URI parsing handles several cases:
    ///
    /// # URI Formats
    ///
    /// - `dotenv://localhost/absolute/path` - Absolute path on localhost
    /// - `dotenv://localhost/./relative/path` - Relative path (the leading ./ is preserved)
    /// - `dotenv://localhost` - Uses default `.env` in current directory
    /// - `dotenv:relative/path` - Relative path without authority
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI to parse, must have scheme "dotenv"
    ///
    /// # Returns
    ///
    /// * `Ok(DotEnvConfig)` - Successfully parsed configuration
    /// * `Err(SecretSpecError)` - If the URI scheme is not "dotenv" or parsing fails
    ///
    /// # Examples
    ///
    /// ```
    /// use http::Uri;
    /// use secretspec::provider::dotenv::DotEnvConfig;
    ///
    /// let uri = "dotenv://localhost/.env.production".parse::<Uri>().unwrap();
    /// let config = DotEnvConfig::from_uri(&uri).unwrap();
    /// assert_eq!(config.path.to_str().unwrap(), "/.env.production");
    /// ```
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

    /// Creates a DotEnvConfig directly from a path string.
    ///
    /// This is a convenience method for creating a configuration
    /// when you have a plain file path without needing URI parsing.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the .env file. If empty, defaults to ".env"
    ///
    /// # Examples
    ///
    /// ```
    /// use secretspec::provider::dotenv::DotEnvConfig;
    ///
    /// // Use default .env
    /// let config = DotEnvConfig::from_path_string("");
    /// assert_eq!(config.path.to_str().unwrap(), ".env");
    ///
    /// // Use custom path
    /// let config = DotEnvConfig::from_path_string("config/.env.local");
    /// assert_eq!(config.path.to_str().unwrap(), "config/.env.local");
    /// ```
    pub fn from_path_string(path: &str) -> Self {
        Self {
            path: PathBuf::from(if path.is_empty() { ".env" } else { path }),
        }
    }
}

/// Provider for managing secrets in .env files.
///
/// The DotEnvProvider implements the Provider trait to enable reading
/// and writing secrets from/to .env files. It uses the dotenvy crate
/// for parsing and serde-envfile for serialization to ensure proper
/// handling of special characters and escaping.
///
/// # Features
///
/// - Reads environment variables from .env files
/// - Writes new or updated variables back to .env files
/// - Preserves existing variables when updating
/// - Handles proper escaping of values with special characters
/// - Supports both relative and absolute file paths
///
/// # Note
///
/// This provider ignores the project and profile parameters as .env files
/// typically don't have built-in namespacing. All secrets are stored
/// flat in the file.
pub struct DotEnvProvider {
    /// Configuration containing the path to the .env file
    config: DotEnvConfig,
}

impl DotEnvProvider {
    /// Creates a new DotEnvProvider with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration specifying the .env file path
    ///
    /// # Examples
    ///
    /// ```
    /// use secretspec::provider::dotenv::{DotEnvProvider, DotEnvConfig};
    ///
    /// let config = DotEnvConfig::default();
    /// let provider = DotEnvProvider::new(config);
    /// ```
    pub fn new(config: DotEnvConfig) -> Self {
        Self { config }
    }

    /// Creates a new DotEnvProvider from a URI.
    ///
    /// This is a convenience method that parses the URI and creates
    /// the provider in one step.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI to parse, must have scheme "dotenv"
    ///
    /// # Returns
    ///
    /// * `Ok(DotEnvProvider)` - Successfully created provider
    /// * `Err(SecretSpecError)` - If URI parsing fails
    ///
    /// # Examples
    ///
    /// ```
    /// use http::Uri;
    /// use secretspec::provider::dotenv::DotEnvProvider;
    ///
    /// let uri = "dotenv://localhost/.env.staging".parse::<Uri>().unwrap();
    /// let provider = DotEnvProvider::from_uri(&uri).unwrap();
    /// ```
    pub fn from_uri(uri: &Uri) -> Result<Self> {
        let config = DotEnvConfig::from_uri(uri)?;
        Ok(Self::new(config))
    }
}

impl Provider for DotEnvProvider {
    /// Retrieves a secret value from the .env file.
    ///
    /// Reads the .env file and returns the value for the specified key.
    /// The project and profile parameters are ignored as .env files
    /// don't support namespacing.
    ///
    /// # Arguments
    ///
    /// * `_project` - Ignored, .env files don't support project namespacing
    /// * `key` - The environment variable name to look up
    /// * `_profile` - Ignored, .env files don't support profile namespacing
    ///
    /// # Returns
    ///
    /// * `Ok(Some(String))` - The value if the key exists
    /// * `Ok(None)` - If the key doesn't exist or the file doesn't exist
    /// * `Err(SecretSpecError)` - If reading the file fails
    ///
    /// # Implementation Details
    ///
    /// Uses the dotenvy crate for parsing to ensure compatibility with
    /// standard .env file formats and proper handling of quoted values,
    /// multiline strings, and escape sequences.
    fn get(&self, _project: &str, key: &str, _profile: &str) -> Result<Option<String>> {
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

    /// Sets a secret value in the .env file.
    ///
    /// Updates or adds a key-value pair in the .env file. If the file
    /// doesn't exist, it will be created. Existing variables are preserved.
    ///
    /// # Arguments
    ///
    /// * `_project` - Ignored, .env files don't support project namespacing
    /// * `key` - The environment variable name to set
    /// * `value` - The value to store
    /// * `_profile` - Ignored, .env files don't support profile namespacing
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the value was successfully written
    /// * `Err(SecretSpecError)` - If reading or writing the file fails
    ///
    /// # Implementation Details
    ///
    /// 1. Loads existing variables using dotenvy to preserve them
    /// 2. Updates or adds the new key-value pair
    /// 3. Serializes back using serde-envfile for proper escaping
    fn set(&self, _project: &str, key: &str, value: &str, _profile: &str) -> Result<()> {
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

    /// Returns the name of this provider.
    ///
    /// Always returns "dotenv" to identify this provider type.
    fn name(&self) -> &'static str {
        "dotenv"
    }

    /// Returns a human-readable description of this provider.
    ///
    /// Provides a brief description for user interfaces and help text.
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

    #[test]
    fn test_from_path_string() {
        // Test empty path defaults to .env
        let config = DotEnvConfig::from_path_string("");
        assert_eq!(config.path.to_str().unwrap(), ".env");

        // Test relative path
        let config = DotEnvConfig::from_path_string("custom/.env");
        assert_eq!(config.path.to_str().unwrap(), "custom/.env");

        // Test absolute path
        let config = DotEnvConfig::from_path_string("/etc/secrets/.env");
        assert_eq!(config.path.to_str().unwrap(), "/etc/secrets/.env");
    }

    #[test]
    fn test_default_config() {
        let config = DotEnvConfig::default();
        assert_eq!(config.path.to_str().unwrap(), ".env");
    }
}
