use crate::provider::Provider;
use crate::{Result, SecretSpecError};
use url::Url;

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
    pub examples: &'static [&'static str],
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
    ///     name: "onepassword",
    ///     description: "OnePassword password manager",
    ///     examples: &["onepassword://vault", "onepassword://work@Production"],
    /// };
    /// assert_eq!(
    ///     info.display_with_examples(),
    ///     "onepassword: OnePassword password manager (e.g., onepassword://vault, onepassword://work@Production)"
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
/// - **onepassword**: OnePassword password manager with vault support
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
        super::PROVIDER_REGISTRY
            .iter()
            .map(|reg| reg.info.clone())
            .collect()
    }

    /// Retrieves information about a specific provider by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the provider to look up (e.g., "keyring", "onepassword")
    ///
    /// # Returns
    ///
    /// `Some(ProviderInfo)` if a provider with the given name exists, `None` otherwise.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(info) = ProviderRegistry::get_info("onepassword") {
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
    /// - **Full URI**: `scheme://authority/path` (e.g., `onepassword://vault/Production`)
    ///
    /// # Special Cases
    ///
    /// - **1password**: Will error suggesting to use `onepassword` instead
    /// - **Bare provider names**: Automatically converted to `provider://`
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
    /// let provider = ProviderRegistry::create_from_string("onepassword://vault/Production")?;
    ///
    /// // Dotenv with path
    /// let provider = ProviderRegistry::create_from_string("dotenv:.env.production")?;
    /// ```
    pub fn create_from_string(s: &str) -> Result<Box<dyn Provider>> {
        // Parse the scheme from the input string
        let (scheme, rest) = if let Some(pos) = s.find(':') {
            let scheme = &s[..pos];
            let rest = &s[pos + 1..];
            (scheme, rest)
        } else {
            // Just a provider name, no URI components
            (s, "")
        };

        // Validate scheme first
        if scheme == "1password" {
            return Err(SecretSpecError::ProviderOperationFailed(
                "Invalid scheme '1password'. Use 'onepassword' instead (e.g., onepassword://vault/path)".to_string()
            ));
        }

        // Check if the scheme is registered
        let is_valid_scheme = super::PROVIDER_REGISTRY
            .iter()
            .any(|reg| reg.schemes.contains(&scheme));

        if !is_valid_scheme {
            // Check if it's a known provider name to give a better error
            if super::PROVIDER_REGISTRY
                .iter()
                .any(|reg| reg.info.name == scheme)
            {
                return Err(SecretSpecError::ProviderOperationFailed(format!(
                    "Provider '{}' exists but URI parsing failed",
                    scheme
                )));
            } else {
                return Err(SecretSpecError::ProviderNotFound(scheme.to_string()));
            }
        }

        // Build a proper URL with the correct scheme
        let url_string = match rest {
            // Just scheme name (e.g., "keyring")
            "" | ":" => format!("{}://", scheme),
            // Standard URI format already has // (e.g., "onepassword://vault/path")
            s if s.starts_with("//") => format!("{}:{}", scheme, s),
            // Path only format (e.g., "dotenv:/path/to/.env")
            s if s.starts_with('/') => format!("{}://{}", scheme, s),
            // Everything else - assume it's a host or path component
            s => format!("{}://{}", scheme, s),
        };

        let proper_url = Url::parse(&url_string).map_err(|e| {
            SecretSpecError::ProviderOperationFailed(format!(
                "Invalid provider specification '{}': {}",
                s, e
            ))
        })?;

        // Find the provider registration for this scheme
        let registration = super::PROVIDER_REGISTRY
            .iter()
            .find(|reg| reg.schemes.contains(&scheme))
            .ok_or_else(|| SecretSpecError::ProviderNotFound(scheme.to_string()))?;

        // Use the factory function to create the provider
        (registration.factory)(&proper_url)
    }
}
