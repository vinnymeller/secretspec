//! # Provider System
//!
//! The provider module implements a trait-based plugin architecture for managing secrets
//! across different storage backends. Providers handle the actual storage and retrieval
//! of secrets, supporting everything from local files to cloud-based secret managers.
//!
//! ## Architecture
//!
//! The provider system is built around the [`Provider`] trait, which defines a common
//! interface for all storage backends. Each provider implementation handles:
//!
//! - Profile-aware storage (e.g., development vs production secrets)
//! - Project isolation (secrets are namespaced by project)
//! - Optional write support (some providers are read-only)
//!
//! ## Available Providers
//!
//! - [`KeyringProvider`]: System keyring integration (default)
//! - [`DotEnvProvider`]: `.env` file support
//! - [`EnvProvider`]: Environment variables (read-only)
//! - [`OnePasswordProvider`]: OnePassword integration
//! - [`LastPassProvider`]: LastPass integration
//!
//! ## URI-Based Configuration
//!
//! Providers support URI-based configuration for flexibility:
//!
//! ```text
//! keyring://
//! dotenv://.env.production
//! onepassword://vault/items
//! lastpass://folder
//! ```
//!
//! ## Example
//!
//! ```rust,ignore
//! use secretspec::provider::{Provider, ProviderRegistry};
//!
//! // Create a provider from a URI string
//! let provider = ProviderRegistry::create_from_string("keyring://")?;
//!
//! // Store a secret
//! provider.set("myproject", "API_KEY", "secret123", "production")?;
//!
//! // Retrieve a secret
//! if let Some(value) = provider.get("myproject", "API_KEY", "production")? {
//!     println!("API_KEY: {}", value);
//! }
//! ```

use crate::Result;

pub mod dotenv;
pub mod env;
pub mod keyring;
pub mod lastpass;
pub mod onepassword;
pub mod registry;
#[macro_use]
pub mod macros;

#[cfg(test)]
pub(crate) mod tests;

/// Configuration and implementation for `.env` file provider
pub use dotenv::{DotEnvConfig, DotEnvProvider};
/// Configuration and implementation for environment variable provider
pub use env::{EnvConfig, EnvProvider};
/// Configuration and implementation for system keyring provider
pub use keyring::{KeyringConfig, KeyringProvider};
/// Configuration and implementation for LastPass provider
pub use lastpass::{LastPassConfig, LastPassProvider};
/// Macro support types
pub use macros::{PROVIDER_REGISTRY, ProviderRegistration};
/// Configuration and implementation for OnePassword provider
pub use onepassword::{OnePasswordConfig, OnePasswordProvider};
/// Provider registry and metadata types
pub use registry::{ProviderInfo, ProviderRegistry};

/// Trait defining the interface for secret storage providers.
///
/// All secret storage backends must implement this trait to integrate with SecretSpec.
/// The trait is designed to be flexible enough to support various storage mechanisms
/// while maintaining a consistent interface.
///
/// # Thread Safety
///
/// Providers must be `Send + Sync` as they may be used across thread boundaries
/// in multi-threaded applications.
///
/// # Profile Support
///
/// Providers should support profile-based secret isolation, allowing different values
/// for the same key across environments (e.g., development, staging, production).
///
/// # Implementation Guidelines
///
/// - Providers should handle their own error cases and return appropriate `Result` types
/// - Storage paths should follow the pattern: `{provider}/{project}/{profile}/{key}`
/// - Providers may choose to be read-only by overriding [`allows_set`](Provider::allows_set)
/// - Provider names should be lowercase and descriptive
pub trait Provider: Send + Sync {
    /// Retrieves a secret value from the provider.
    ///
    /// # Arguments
    ///
    /// * `project` - The project namespace for the secret
    /// * `key` - The secret key/name to retrieve
    /// * `profile` - The profile context (e.g., "default", "production")
    ///
    /// # Returns
    ///
    /// - `Ok(Some(value))` if the secret exists
    /// - `Ok(None)` if the secret doesn't exist
    /// - `Err` if there was an error accessing the provider
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// match provider.get("myapp", "DATABASE_URL", "production")? {
    ///     Some(url) => println!("Database URL: {}", url),
    ///     None => println!("DATABASE_URL not found"),
    /// }
    /// ```
    fn get(&self, project: &str, key: &str, profile: &str) -> Result<Option<String>>;

    /// Stores a secret value in the provider.
    ///
    /// # Arguments
    ///
    /// * `project` - The project namespace for the secret
    /// * `key` - The secret key/name to store
    /// * `value` - The secret value to store
    /// * `profile` - The profile context (e.g., "default", "production")
    ///
    /// # Returns
    ///
    /// - `Ok(())` if the secret was successfully stored
    /// - `Err` if there was an error or the provider is read-only
    ///
    /// # Errors
    ///
    /// This method should return an error if [`allows_set`](Provider::allows_set) returns `false`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// provider.set("myapp", "API_KEY", "secret123", "production")?;
    /// ```
    fn set(&self, project: &str, key: &str, value: &str, profile: &str) -> Result<()>;

    /// Returns whether this provider supports setting values.
    ///
    /// By default, providers are assumed to support writing. Read-only providers
    /// (like environment variables) should override this to return `false`.
    ///
    /// # Returns
    ///
    /// - `true` if the provider supports [`set`](Provider::set) operations
    /// - `false` if the provider is read-only
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if provider.allows_set() {
    ///     provider.set("myapp", "TOKEN", "value", "default")?;
    /// } else {
    ///     eprintln!("Provider is read-only");
    /// }
    /// ```
    fn allows_set(&self) -> bool {
        true
    }

    /// Returns the name of this provider.
    ///
    /// This should match the name registered with the provider macro.
    fn name(&self) -> &'static str;
}
