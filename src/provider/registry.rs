use crate::provider::{
    DotEnvConfig, DotEnvProvider, EnvConfig, EnvProvider, KeyringConfig, KeyringProvider,
    LastPassConfig, LastPassProvider, OnePasswordConfig, OnePasswordProvider, Provider,
};
use crate::{Result, SecretSpecError};
use http::Uri;

/// Information about a secret storage provider.
///
/// Contains metadata used for displaying available providers to users,
/// including the provider's name, description, and example URIs.
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    /// The canonical name of the provider (e.g., "keyring", "1password").
    pub name: &'static str,
    /// A human-readable description of what the provider does.
    pub description: &'static str,
    /// Example URIs showing how to configure this provider.
    pub examples: Vec<&'static str>,
}

impl ProviderInfo {
    /// Formats the provider information for display, including examples if available.
    ///
    /// # Returns
    ///
    /// A formatted string in one of two formats:
    /// - Without examples: "name: description"
    /// - With examples: "name: description (e.g., example1, example2)"
    ///
    /// # Example
    ///
    /// ```ignore
    /// let info = ProviderInfo {
    ///     name: "1password",
    ///     description: "1Password password manager",
    ///     examples: vec!["1password://vault", "1password://work@Production"],
    /// };
    /// assert_eq!(
    ///     info.display_with_examples(),
    ///     "1password: 1Password password manager (e.g., 1password://vault, 1password://work@Production)"
    /// );
    /// ```
    pub fn display_with_examples(&self) -> String {
        if self.examples.is_empty() {
            format!("{}: {}", self.name, self.description)
        } else {
            format!(
                "{}: {} (e.g., {})",
                self.name,
                self.description,
                self.examples.join(", ")
            )
        }
    }
}

/// Registry for managing secret storage providers.
///
/// The `ProviderRegistry` is responsible for:
/// - Maintaining a list of available providers and their metadata
/// - Creating provider instances from URI strings
/// - Handling URI parsing and normalization for different provider schemes
///
/// # Supported Providers
///
/// - **keyring**: System keychain integration (macOS Keychain, Windows Credential Manager, Linux Secret Service)
/// - **1password**: 1Password password manager with vault support
/// - **dotenv**: Traditional .env files with optional path specification
/// - **env**: Read-only access to environment variables
/// - **lastpass**: LastPass password manager with folder support
pub struct ProviderRegistry;

impl ProviderRegistry {
    /// Returns a list of all available providers with their metadata.
    ///
    /// This includes the provider name, description, and example URIs for each
    /// supported provider type.
    ///
    /// # Returns
    ///
    /// A vector of `ProviderInfo` structs containing metadata for each provider.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let providers = ProviderRegistry::providers();
    /// for provider in providers {
    ///     println!("{}", provider.display_with_examples());
    /// }
    /// ```
    pub fn providers() -> Vec<ProviderInfo> {
        vec![
            ProviderInfo {
                name: "keyring",
                description: "Uses system keychain (Recommended)",
                examples: vec![],
            },
            ProviderInfo {
                name: "1password",
                description: "1Password password manager",
                examples: vec!["1password://vault", "1password://work@Production"],
            },
            ProviderInfo {
                name: "dotenv",
                description: "Traditional .env files",
                examples: vec!["dotenv:/path/to/.env"],
            },
            ProviderInfo {
                name: "env",
                description: "Read-only environment variables",
                examples: vec![],
            },
            ProviderInfo {
                name: "lastpass",
                description: "LastPass password manager",
                examples: vec!["lastpass://folder"],
            },
        ]
    }

    /// Retrieves information about a specific provider by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the provider to look up (e.g., "keyring", "1password")
    ///
    /// # Returns
    ///
    /// `Some(ProviderInfo)` if a provider with the given name exists, `None` otherwise.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(info) = ProviderRegistry::get_info("1password") {
    ///     println!("Provider: {}", info.description);
    /// }
    /// ```
    pub fn get_info(name: &str) -> Option<ProviderInfo> {
        Self::providers().into_iter().find(|p| p.name == name)
    }

    /// Creates a provider instance from a URI string.
    ///
    /// This function handles various URI formats and normalizes them before parsing.
    /// It supports both full URIs and shorthand notations.
    ///
    /// # URI Formats
    ///
    /// - **Full URI**: `scheme://authority/path` (e.g., `1password://vault/Production`)
    /// - **Scheme only**: `scheme` or `scheme:` (e.g., `keyring`, `env:`)
    /// - **Dotenv shorthand**: `dotenv:/path/to/.env` (special case without authority)
    /// - **Provider with path**: `scheme:/path` (normalized to `scheme://localhost/path`)
    ///
    /// # Special Cases
    ///
    /// - **dotenv**: Supports both `dotenv://path` and `dotenv:/path` formats
    /// - **onepassword**: Will error suggesting to use `1password` instead
    /// - **Bare provider names**: Automatically converted to `provider://localhost`
    ///
    /// # Arguments
    ///
    /// * `s` - The URI string to parse and create a provider from
    ///
    /// # Returns
    ///
    /// A boxed provider instance on success, or an error if:
    /// - The URI format is invalid
    /// - The provider scheme is not recognized
    /// - Provider-specific configuration parsing fails
    ///
    /// # Errors
    ///
    /// - `SecretSpecError::ProviderOperationFailed` - Invalid URI format
    /// - `SecretSpecError::ProviderNotFound` - Unknown provider scheme
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Simple provider name
    /// let provider = ProviderRegistry::create_from_string("keyring")?;
    ///
    /// // Full URI with configuration
    /// let provider = ProviderRegistry::create_from_string("1password://vault/Production")?;
    ///
    /// // Dotenv with path
    /// let provider = ProviderRegistry::create_from_string("dotenv:.env.production")?;
    /// ```
    pub fn create_from_string(s: &str) -> Result<Box<dyn Provider>> {
        // Special handling for dotenv with paths
        if s.starts_with("dotenv:") && !s.contains("://") {
            let path = &s[7..]; // Remove "dotenv:" prefix
            let config = if path.is_empty() {
                DotEnvConfig::default()
            } else {
                DotEnvConfig::from_path_string(path)
            };
            return Ok(Box::new(DotEnvProvider::new(config)));
        }

        // Normalize the input to ensure it's a valid URI
        let normalized = if s.contains("://") {
            // Already has scheme separator
            s.to_string()
        } else if s.ends_with(':') {
            // Has colon but no slashes, add dummy authority
            format!("{}//localhost", s)
        } else if s.contains(':') && s.contains('/') {
            // Has colon and slashes but not "://", probably like "dotenv:/path"
            // Insert authority after the colon
            let colon_pos = s.find(':').ok_or_else(|| {
                SecretSpecError::ProviderOperationFailed(
                    "Invalid URI format: missing scheme separator".to_string(),
                )
            })?;
            format!("{}://localhost{}", &s[..colon_pos], &s[colon_pos + 1..])
        } else {
            // Just a provider name, make it a proper URI
            format!("{}://localhost", s)
        };

        // Parse the normalized URI
        let uri = normalized.parse::<Uri>().map_err(|e| {
            SecretSpecError::ProviderOperationFailed(format!(
                "Invalid provider specification '{}': {}",
                s, e
            ))
        })?;

        let scheme = uri.scheme_str().ok_or_else(|| {
            SecretSpecError::ProviderOperationFailed("URI must have a scheme".to_string())
        })?;

        match scheme {
            "1password" | "1password+token" => {
                let config = OnePasswordConfig::from_uri(&uri)?;
                Ok(Box::new(OnePasswordProvider::new(config)))
            }
            "keyring" => {
                let config = KeyringConfig::from_uri(&uri)?;
                Ok(Box::new(KeyringProvider::new(config)))
            }
            "dotenv" => {
                let config = DotEnvConfig::from_uri(&uri)?;
                Ok(Box::new(DotEnvProvider::new(config)))
            }
            "env" => {
                let config = EnvConfig::from_uri(&uri)?;
                Ok(Box::new(EnvProvider::new(config)))
            }
            "lastpass" => {
                let config = LastPassConfig::from_uri(&uri)?;
                Ok(Box::new(LastPassProvider::new(config)))
            }
            "onepassword" => Err(SecretSpecError::ProviderOperationFailed(
                "Invalid scheme 'onepassword'. Use '1password' instead (e.g., 1password://vault/path)".to_string()
            )),
            _ => {
                // Check if it's a known provider name to give a better error
                if Self::providers().iter().any(|p| p.name == scheme) {
                    Err(SecretSpecError::ProviderOperationFailed(
                        format!("Provider '{}' exists but URI parsing failed", scheme)
                    ))
                } else {
                    Err(SecretSpecError::ProviderNotFound(scheme.to_string()))
                }
            }
        }
    }
}
