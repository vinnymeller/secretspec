use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectInfo,
    pub profiles: HashMap<String, ProfileConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub revision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileConfig {
    #[serde(flatten)]
    pub secrets: HashMap<String, SecretConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SecretConfig {
    pub description: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub defaults: DefaultConfig,
    #[serde(default)]
    pub projects: HashMap<String, ProjectUserConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DefaultConfig {
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectUserConfig {
    pub provider: String,
}

#[derive(Debug)]
pub enum ParseError {
    Io(io::Error),
    Toml(toml::de::Error),
    UnsupportedRevision(String),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Keyring,
    Dotenv,
    Env,
    #[serde(rename = "1password")]
    OnePassword,
    Lastpass,
}

impl Provider {
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

#[derive(Debug, Clone)]
pub struct SecretSpecSecrets<T> {
    pub secrets: T,
    pub provider: Provider,
    pub profile: String,
}

impl<T> SecretSpecSecrets<T> {
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

pub fn parse_spec_from_str(
    content: &str,
    base_path: Option<&Path>,
) -> Result<ProjectConfig, ParseError> {
    let mut visited = HashSet::new();
    parse_spec_from_str_with_visited(content, base_path, &mut visited)
}

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

pub fn parse_spec(path: &Path) -> Result<ProjectConfig, ParseError> {
    let mut visited = HashSet::new();
    parse_spec_with_visited(path, &mut visited)
}

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
