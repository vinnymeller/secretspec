//! # SecretSpec Core
//!
//! This crate provides the core type definitions and parsing logic for the SecretSpec 
//! configuration system.
//!
//! SecretSpec uses a declarative TOML-based configuration format to define secrets
//! and their requirements across different environments (profiles). The type system
//! supports configuration inheritance, allowing projects to extend shared configurations
//! while maintaining type safety and preventing circular dependencies.
//!
//! ## Key Features
//!
//! - **Profile-based configuration**: Define different sets of secrets for development, staging, production, etc.
//! - **Configuration inheritance**: Extend other configurations to share common secrets
//! - **Provider abstraction**: Support for multiple secret storage backends
//! - **Type-safe parsing**: Strong typing with comprehensive error handling
//!
//! ## Configuration Structure
//!
//! A typical `secretspec.toml` file has this structure:
//!
//! ```toml
//! [project]
//! name = "my-app"
//! revision = "1.0"
//! extends = ["../shared/common"]  # Optional inheritance
//!
//! [profiles.default]
//! DATABASE_URL = { description = "PostgreSQL connection string", required = true }
//! API_KEY = { description = "External API key", required = false, default = "dev-key" }
//!
//! [profiles.production]
//! DATABASE_URL = { description = "Production database", required = true }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// The root configuration structure for a SecretSpec project.
///
/// This is the top-level type that represents the entire `secretspec.toml` file.
/// It contains project metadata and profile-specific secret definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Project metadata including name, revision, and optional inheritance
    pub project: ProjectInfo,
    /// Map of profile names to their configurations (e.g., "default", "production", "staging")
    pub profiles: HashMap<String, ProfileConfig>,
}

impl ProjectConfig {

    /// Validate the configuration.
    ///
    /// Ensures that:
    /// - Project name is not empty
    /// - At least one profile is defined
    /// - All secrets have valid configurations
    /// - Secret names are valid identifiers
    ///
    /// # Errors
    ///
    /// Returns a `ParseError` if validation fails.
    pub fn validate(&self) -> Result<(), ParseError> {
        if self.project.name.is_empty() {
            return Err(ParseError::Validation("Project name cannot be empty".into()));
        }

        if self.profiles.is_empty() {
            return Err(ParseError::Validation("At least one profile must be defined".into()));
        }

        // Validate each profile
        for (profile_name, profile) in &self.profiles {
            profile.validate()
                .map_err(|e| ParseError::Validation(format!("Profile '{}': {}", profile_name, e)))?;
        }

        Ok(())
    }

    /// Get a profile by name.
    pub fn get_profile(&self, name: &str) -> Option<&ProfileConfig> {
        self.profiles.get(name)
    }

    /// Get a mutable profile by name.
    pub fn get_profile_mut(&mut self, name: &str) -> Option<&mut ProfileConfig> {
        self.profiles.get_mut(name)
    }

    /// Merge another configuration into this one.
    ///
    /// The current configuration takes precedence - values from `other`
    /// are only used if not already present.
    pub fn merge_with(&mut self, other: ProjectConfig) {
        // Merge profiles
        for (profile_name, profile_config) in other.profiles {
            match self.profiles.get_mut(&profile_name) {
                Some(existing_profile) => {
                    existing_profile.merge_with(profile_config);
                }
                None => {
                    self.profiles.insert(profile_name, profile_config);
                }
            }
        }
    }

    // Internal methods

    fn from_path_with_visited(
        path: &Path,
        visited: &mut HashSet<PathBuf>,
    ) -> Result<Self, ParseError> {
        // Get canonical path to handle symlinks and relative paths consistently
        let canonical_path = path.canonicalize().map_err(|e| {
            ParseError::Io(io::Error::new(
                e.kind(),
                format!("Failed to resolve path {}: {}", path.display(), e),
            ))
        })?;

        // Check for circular dependency
        if !visited.insert(canonical_path.clone()) {
            return Err(ParseError::CircularDependency(format!(
                "Configuration file {} is part of a circular dependency chain",
                canonical_path.display()
            )));
        }

        let content = fs::read_to_string(path)?;
        Self::from_str_with_visited(&content, Some(path), visited)
    }

    fn from_str_with_visited(
        content: &str,
        base_path: Option<&Path>,
        visited: &mut HashSet<PathBuf>,
    ) -> Result<Self, ParseError> {
        let mut config: ProjectConfig = toml::from_str(content)?;

        // Validate revision
        if config.project.revision != "1.0" {
            return Err(ParseError::UnsupportedRevision(config.project.revision));
        }

        // Process extends if present
        if let Some(extends_paths) = config.project.extends.clone() {
            if let Some(base) = base_path {
                let base_dir = base.parent().unwrap_or(Path::new("."));
                config = Self::merge_extended_configs(config, &extends_paths, base_dir, visited)?;
            }
        }

        Ok(config)
    }

    fn merge_extended_configs(
        mut base_config: ProjectConfig,
        extends_paths: &[String],
        base_dir: &Path,
        visited: &mut HashSet<PathBuf>,
    ) -> Result<ProjectConfig, ParseError> {
        for extend_path in extends_paths {
            let full_path = base_dir.join(extend_path).join("secretspec.toml");

            if !full_path.exists() {
                return Err(ParseError::Io(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Extended config file not found: {}", full_path.display()),
                )));
            }

            let extended_config = Self::from_path_with_visited(&full_path, visited)?;
            base_config.merge_with(extended_config);
        }

        Ok(base_config)
    }
}

impl FromStr for ProjectConfig {
    type Err = ParseError;

    /// Parse configuration from a TOML string.
    ///
    /// Note: Configuration inheritance (`extends`) is not supported when parsing
    /// from a string since there's no base path to resolve relative paths.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut visited = HashSet::new();
        Self::from_str_with_visited(s, None, &mut visited)
    }
}

impl TryFrom<&Path> for ProjectConfig {
    type Error = ParseError;

    /// Load configuration from a file path.
    ///
    /// This supports configuration inheritance via `extends` and circular dependency detection.
    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        let mut visited = HashSet::new();
        Self::from_path_with_visited(path, &mut visited)
    }
}

/// Project metadata and inheritance configuration.
///
/// Contains essential project information and optional configuration inheritance.
/// The `extends` field allows projects to inherit secrets from other configurations,
/// enabling shared configuration patterns across multiple projects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    /// The name of the project, used for identification and namespacing
    pub name: String,
    /// Configuration format revision (currently must be "1.0")
    pub revision: String,
    /// Optional list of relative paths to other SecretSpec projects to inherit from
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<Vec<String>>,
}

/// Configuration for a specific profile (environment).
///
/// A profile represents a specific environment or context (e.g., "default", "production", "staging").
/// Each profile contains its own set of secret definitions with their requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    /// Map of secret names to their configurations, flattened in TOML for cleaner syntax
    #[serde(flatten)]
    pub secrets: HashMap<String, SecretConfig>,
}

impl ProfileConfig {
    /// Create a new empty profile configuration.
    pub fn new() -> Self {
        Self {
            secrets: HashMap::new(),
        }
    }

    /// Validate the profile configuration.
    ///
    /// Ensures all secrets have valid names and configurations.
    pub fn validate(&self) -> Result<(), String> {
        if self.secrets.is_empty() {
            return Err("Profile must define at least one secret".into());
        }

        for (name, secret) in &self.secrets {
            // Validate secret name is a valid identifier
            if !is_valid_identifier(name) {
                return Err(format!("Invalid secret name '{}': must be a valid identifier (alphanumeric and underscores, not starting with a number)", name));
            }

            secret.validate()
                .map_err(|e| format!("Secret '{}': {}", name, e))?;
        }

        Ok(())
    }

    /// Merge another profile configuration into this one.
    ///
    /// The current profile takes precedence - secrets from `other`
    /// are only added if they don't already exist.
    pub fn merge_with(&mut self, other: ProfileConfig) {
        for (secret_name, secret_config) in other.secrets {
            self.secrets.entry(secret_name).or_insert(secret_config);
        }
    }

}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self::new()
    }
}


/// Configuration for an individual secret.
///
/// Defines the properties of a secret including its documentation,
/// whether it's required, and an optional default value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretConfig {
    /// Human-readable description of what this secret is used for
    pub description: String,
    /// Whether this secret must be provided (no default value)
    /// Defaults to true if not specified
    #[serde(default = "default_true")]
    pub required: bool,
    /// Optional default value if the secret is not provided
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

impl SecretConfig {
    /// Validate the secret configuration.
    ///
    /// Ensures that required secrets don't have default values.
    pub fn validate(&self) -> Result<(), String> {
        if self.description.is_empty() {
            return Err("Secret description cannot be empty".into());
        }

        if self.required && self.default.is_some() {
            return Err("Required secrets cannot have default values".into());
        }

        Ok(())
    }
}


fn default_true() -> bool {
    true
}

/// Check if a string is a valid identifier.
fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let mut chars = s.chars();
    if let Some(first) = chars.next() {
        if !first.is_alphabetic() && first != '_' {
            return false;
        }
    }

    chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// Available secret storage providers.
///
/// This enum represents the different backends that can be used to store and retrieve secrets.
/// Each provider has its own characteristics and use cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    /// System keyring (OS-specific secure storage)
    Keyring,
    /// Local `.env` files
    Dotenv,
    /// Environment variables (read-only)
    Env,
    /// 1Password secret manager
    #[serde(rename = "1password")]
    OnePassword,
    /// LastPass password manager
    Lastpass,
}

impl Provider {

    /// Get the string representation of this provider.
    ///
    /// This is the canonical name used in configuration files and CLI arguments.
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::Keyring => "keyring",
            Provider::Dotenv => "dotenv",
            Provider::Env => "env",
            Provider::OnePassword => "1password",
            Provider::Lastpass => "lastpass",
        }
    }
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<'a> TryFrom<&'a str> for Provider {
    type Error = String;

    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        // Extract the scheme from the URI-like string
        // Handle both "provider" and "provider://..." formats
        let scheme = if let Some(colon_pos) = s.find(':') {
            &s[..colon_pos]
        } else {
            s
        };

        match scheme {
            "keyring" => Ok(Provider::Keyring),
            "dotenv" => Ok(Provider::Dotenv),
            "env" => Ok(Provider::Env),
            "1password" => Ok(Provider::OnePassword),
            "lastpass" => Ok(Provider::Lastpass),
            _ => Err(format!("Unknown provider: {}", scheme)),
        }
    }
}

/// Global user configuration for SecretSpec.
///
/// This configuration is stored in the user's config directory and provides
/// defaults that apply across all projects.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalConfig {
    /// Default settings
    #[serde(default)]
    pub defaults: GlobalDefaults,
}

/// Default settings in the global configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalDefaults {
    /// Default provider to use when not specified
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Default profile to use when not specified
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

/// Container for resolved secrets with their context.
///
/// This generic struct wraps the actual secret values along with
/// information about which provider and profile were used to retrieve them.
/// The generic parameter `T` is typically a struct generated by the
/// `secretspec-derive` macro containing the actual secret values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretSpecSecrets<T> {
    /// The actual secret values, typically a generated struct
    pub secrets: T,
    /// The provider that was used to retrieve these secrets
    pub provider: Provider,
    /// The profile that was active when retrieving these secrets
    pub profile: String,
}

impl<T> SecretSpecSecrets<T> {
    /// Create a new container for secrets with their retrieval context.
    ///
    /// # Arguments
    ///
    /// * `secrets` - The actual secret values
    /// * `provider` - The provider used to retrieve the secrets
    /// * `profile` - The active profile when the secrets were retrieved
    pub fn new(secrets: T, provider: Provider, profile: String) -> Self {
        Self {
            secrets,
            provider,
            profile,
        }
    }
}

/// Errors that can occur when parsing SecretSpec configuration files.
///
/// This enum represents various failure modes when loading and parsing
/// configuration files, including I/O errors, TOML syntax errors,
/// validation failures, and circular dependency detection.
#[derive(Debug)]
pub enum ParseError {
    /// I/O error when reading configuration files
    Io(io::Error),
    /// TOML parsing error
    Toml(toml::de::Error),
    /// Unsupported configuration revision
    UnsupportedRevision(String),
    /// Circular dependency detected in configuration inheritance
    CircularDependency(String),
    /// Validation error
    Validation(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Io(e) => write!(f, "I/O error: {}", e),
            ParseError::Toml(e) => write!(f, "TOML parsing error: {}", e),
            ParseError::UnsupportedRevision(rev) => {
                write!(f, "Unsupported revision '{}'. Only '1.0' is supported.", rev)
            }
            ParseError::CircularDependency(msg) => {
                write!(f, "Circular dependency detected: {}", msg)
            }
            ParseError::Validation(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ParseError::Io(e) => Some(e),
            ParseError::Toml(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for ParseError {
    fn from(e: io::Error) -> Self {
        ParseError::Io(e)
    }
}

impl From<toml::de::Error> for ParseError {
    fn from(e: toml::de::Error) -> Self {
        ParseError::Toml(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_try_from_simple() {
        assert_eq!(Provider::try_from("keyring").unwrap(), Provider::Keyring);
        assert_eq!(Provider::try_from("dotenv").unwrap(), Provider::Dotenv);
        assert_eq!(Provider::try_from("env").unwrap(), Provider::Env);
        assert_eq!(Provider::try_from("1password").unwrap(), Provider::OnePassword);
        assert_eq!(Provider::try_from("lastpass").unwrap(), Provider::Lastpass);
    }

    #[test]
    fn test_provider_try_from_with_uri() {
        assert_eq!(Provider::try_from("keyring://").unwrap(), Provider::Keyring);
        assert_eq!(Provider::try_from("dotenv://.env").unwrap(), Provider::Dotenv);
        assert_eq!(Provider::try_from("dotenv://.env.production").unwrap(), Provider::Dotenv);
        assert_eq!(Provider::try_from("1password://vault").unwrap(), Provider::OnePassword);
        assert_eq!(Provider::try_from("1password://vault/Production").unwrap(), Provider::OnePassword);
    }

    #[test]
    fn test_provider_try_from_unknown() {
        assert!(Provider::try_from("unknown").is_err());
        assert!(Provider::try_from("unknown://something").is_err());
    }
}

