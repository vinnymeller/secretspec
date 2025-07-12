//! # SecretSpec Types
//!
//! This crate provides the core type definitions for the SecretSpec configuration system.
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

/// Override configuration for a secret in a specific profile.
///
/// Allows overriding the `required` status and `default` value of a secret
/// that was defined in another profile or inherited configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileOverride {
    /// Override whether this secret is required in this profile
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// Override the default value for this profile
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
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
    pub required: bool,
    /// Optional default value to use if the secret is not provided
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// Global configuration for SecretSpec.
///
/// Typically stored in the user's configuration directory,
/// this defines system-wide defaults for SecretSpec behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Default settings for provider and profile selection
    pub defaults: DefaultConfig,
}

/// Default configuration settings.
///
/// Specifies the default provider and optionally the default profile
/// to use when not explicitly specified via CLI or environment variables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultConfig {
    /// Default secret storage provider (e.g., "keyring", "dotenv", "1password")
    pub provider: String,
    /// Optional default profile to use (falls back to "default" if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

/// Errors that can occur when parsing SecretSpec configuration files.
#[derive(Debug)]
pub enum ParseError {
    /// I/O error when reading configuration files
    Io(io::Error),
    /// TOML parsing error for invalid configuration syntax
    Toml(toml::de::Error),
    /// The configuration specifies an unsupported revision number
    UnsupportedRevision(String),
    /// A circular dependency was detected in the configuration inheritance chain
    CircularDependency(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Io(e) => write!(f, "IO error: {}", e),
            ParseError::Toml(e) => write!(f, "TOML parsing error: {}", e),
            ParseError::UnsupportedRevision(rev) => write!(f, "Unsupported revision: {}", rev),
            ParseError::CircularDependency(msg) => {
                write!(f, "Circular dependency detected: {}", msg)
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Supported secret storage providers.
///
/// Each provider implements a different method of storing and retrieving secrets.
/// Providers can be selected via configuration, CLI flags, or environment variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    /// System keyring (Keychain on macOS, Credential Manager on Windows, Secret Service on Linux)
    Keyring,
    /// .env files for local development
    Dotenv,
    /// Environment variables (read-only)
    Env,
    /// 1Password password manager integration
    #[serde(rename = "1password")]
    OnePassword,
    /// LastPass password manager integration
    Lastpass,
}

impl Provider {
    /// Parse a provider from its string representation.
    ///
    /// Returns `None` if the string doesn't match any known provider.
    ///
    /// # Examples
    ///
    /// ```
    /// use secretspec_types::Provider;
    ///
    /// assert_eq!(Provider::from_str("keyring"), Some(Provider::Keyring));
    /// assert_eq!(Provider::from_str("1password"), Some(Provider::OnePassword));
    /// assert_eq!(Provider::from_str("unknown"), None);
    /// ```
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "keyring" => Some(Provider::Keyring),
            "dotenv" => Some(Provider::Dotenv),
            "env" => Some(Provider::Env),
            "1password" => Some(Provider::OnePassword),
            "lastpass" => Some(Provider::Lastpass),
            _ => None,
        }
    }

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

/// Parse a SecretSpec configuration from a string.
///
/// This function parses TOML content and handles configuration inheritance
/// if `extends` is specified in the project section.
///
/// # Arguments
///
/// * `content` - The TOML content to parse
/// * `base_path` - Optional base path for resolving relative paths in `extends`
///
/// # Errors
///
/// Returns a `ParseError` if:
/// - The TOML syntax is invalid
/// - The revision is not "1.0"
/// - Extended configuration files cannot be found
/// - A circular dependency is detected
pub fn parse_spec_from_str(
    content: &str,
    base_path: Option<&Path>,
) -> Result<ProjectConfig, ParseError> {
    let mut visited = HashSet::new();
    parse_spec_from_str_with_visited(content, base_path, &mut visited)
}

/// Internal function to parse configuration with circular dependency detection.
///
/// This maintains a set of visited paths to detect and prevent circular
/// dependencies in the configuration inheritance chain.
fn parse_spec_from_str_with_visited(
    content: &str,
    base_path: Option<&Path>,
    visited: &mut HashSet<PathBuf>,
) -> Result<ProjectConfig, ParseError> {
    let mut config: ProjectConfig = toml::from_str(content)?;

    // Validate revision
    if config.project.revision != "1.0" {
        return Err(ParseError::UnsupportedRevision(config.project.revision));
    }

    // Process extends if present
    if let Some(extends_paths) = config.project.extends.clone() {
        if let Some(base) = base_path {
            let base_dir = base.parent().unwrap_or(Path::new("."));
            config =
                merge_extended_configs_with_visited(config, &extends_paths, base_dir, visited)?;
        }
    }

    Ok(config)
}

/// Parse a SecretSpec configuration from a file path.
///
/// This is the main entry point for loading configuration files.
/// It reads the file, parses the TOML content, validates the format,
/// and processes any configuration inheritance.
///
/// # Arguments
///
/// * `path` - Path to the `secretspec.toml` file
///
/// # Errors
///
/// Returns a `ParseError` if:
/// - The file cannot be read
/// - The TOML syntax is invalid
/// - The revision is not "1.0"
/// - Extended configuration files cannot be found
/// - A circular dependency is detected
///
/// # Example
///
/// ```no_run
/// use secretspec_types::parse_spec;
/// use std::path::Path;
///
/// let config = parse_spec(Path::new("secretspec.toml"))?;
/// println!("Project: {}", config.project.name);
/// # Ok::<(), secretspec_types::ParseError>(())
/// ```
pub fn parse_spec(path: &Path) -> Result<ProjectConfig, ParseError> {
    let mut visited = HashSet::new();
    parse_spec_with_visited(path, &mut visited)
}

/// Internal function to parse a file with circular dependency detection.
///
/// Canonicalizes the path to handle symlinks and relative paths consistently,
/// then checks if the file has already been visited to prevent infinite loops.
fn parse_spec_with_visited(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<ProjectConfig, ParseError> {
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
    parse_spec_from_str_with_visited(&content, Some(path), visited)
}

/// Merge extended configurations into the base configuration.
///
/// This function implements the configuration inheritance mechanism.
/// It loads each extended configuration file and merges its profiles
/// and secrets into the base configuration. The base configuration
/// takes precedence - extended values are only used if not already
/// defined in the base.
///
/// # Merge Rules
///
/// - If a profile exists in both base and extended configs, secrets are merged
/// - If a secret exists in both, the base configuration value is kept
/// - New profiles from extended configs are added entirely
/// - Extended configs are processed in order, with earlier ones taking precedence
fn merge_extended_configs_with_visited(
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

        let extended_config = parse_spec_with_visited(&full_path, visited)?;

        // Merge profiles from extended config into base config
        for (profile_name, profile_config) in extended_config.profiles {
            match base_config.profiles.get_mut(&profile_name) {
                Some(base_profile) => {
                    // Merge secrets within the profile
                    for (secret_name, secret_config) in profile_config.secrets {
                        // Base config takes precedence, only add if not already present
                        base_profile
                            .secrets
                            .entry(secret_name)
                            .or_insert(secret_config);
                    }
                }
                None => {
                    // Profile doesn't exist in base, add it entirely
                    base_config.profiles.insert(profile_name, profile_config);
                }
            }
        }
    }

    Ok(base_config)
}
