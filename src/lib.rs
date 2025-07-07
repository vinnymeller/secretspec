use colored::Colorize;
use directories::ProjectDirs;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

mod provider;
use provider::{Provider, ProviderRegistry};

#[cfg(feature = "codegen")]
pub use secretspec_derive::define_secrets;

// Re-export types for convenience
pub use secretspec_types::*;

#[derive(Error, Debug)]
pub enum SecretSpecError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parsing error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error(
        "Unsupported secretspec revision '{0}'. This version of secretspec only supports revision '1.0'"
    )]
    UnsupportedRevision(String),
    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("Keyring error: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("Dotenv error: {0}")]
    Dotenv(#[from] dotenvy::Error),
    #[error(
        "No provider backend configured.\n\nTo fix this, either:\n  1. Run 'secretspec config init' to set up your default provider\n  2. Use --provider flag (e.g., 'secretspec check --provider keyring')"
    )]
    NoProviderConfigured,
    #[error("Provider backend '{0}' not found")]
    ProviderNotFound(String),
    #[error("Secret '{0}' not found")]
    SecretNotFound(String),
    #[error("Secret '{0}' is required but not set")]
    RequiredSecretMissing(String),
    #[error("No secretspec.toml found in current directory")]
    NoManifest,
    #[error("Project name not found in secretspec.toml")]
    NoProjectName,
    #[error("Provider operation failed: {0}")]
    ProviderOperationFailed(String),
    #[error("User interaction error: {0}")]
    InquireError(#[from] inquire::InquireError),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid profile: {0}")]
    InvalidProfile(String),
}

pub type Result<T> = std::result::Result<T, SecretSpecError>;

#[derive(Debug)]
pub struct ValidationResult {
    pub secrets: HashMap<String, String>,
    pub missing_required: Vec<String>,
    pub missing_optional: Vec<String>,
    pub with_defaults: Vec<(String, String)>,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        self.missing_required.is_empty()
    }
}

// Extension methods for ProjectConfig
pub fn project_config_from_path(from: &Path) -> Result<ProjectConfig> {
    let mut secrets = HashMap::new();

    if from.exists() {
        let env_vars = dotenvy::from_path_iter(from)?;
        for item in env_vars {
            let (key, _) = item?;
            secrets.insert(
                key.clone(),
                SecretConfig {
                    description: format!("{} secret", key),
                    required: true,
                    default: None,
                },
            );
        }
    }

    let mut profiles = HashMap::new();
    profiles.insert("default".to_string(), ProfileConfig { secrets });

    Ok(ProjectConfig {
        project: ProjectInfo {
            name: std::env::current_dir()?
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            revision: "1.0".to_string(),
            extends: None,
        },
        profiles,
    })
}

pub fn get_example_toml() -> &'static str {
    r#"
# Example secrets configuration
# Uncomment and modify the sections you need

# [profiles.default]
# API_KEY = { description = "API key for external service", required = true }
# DATABASE_URL = { description = "Database connection string", required = true }
#
# [profiles.development]
# API_KEY = { description = "API key for external service", required = false, default = "dev-api-key" }
# DATABASE_URL = { description = "Database connection string", required = true, default = "sqlite:///dev.db" }
# JWT_SECRET = { description = "Secret key for JWT token signing", required = true }
# REDIS_URL = { description = "Redis connection URL for caching", required = false, default = "redis://localhost:6379" }
# EMAIL_PROVIDER = { description = "Email service provider", required = false, default = "console" }
# OAUTH_CLIENT_ID = { description = "OAuth client ID", required = false }
# OAUTH_CLIENT_SECRET = { description = "OAuth client secret", required = false }
"#
}

pub struct SecretSpec {
    registry: ProviderRegistry,
    config: ProjectConfig,
    global_config: Option<GlobalConfig>,
}

impl SecretSpec {
    pub fn new(config: ProjectConfig, global_config: Option<GlobalConfig>) -> Self {
        Self {
            registry: ProviderRegistry::new(),
            config,
            global_config,
        }
    }

    pub fn load() -> Result<Self> {
        let project_config = load_project_config()?;
        let global_config = load_global_config()?;
        Ok(Self::new(project_config, global_config))
    }

    fn resolve_secret_config(
        &self,
        name: &str,
        profile: Option<&str>,
    ) -> Option<(bool, Option<String>)> {
        let profile_name = profile.unwrap_or("default");
        let profile_config = self.config.profiles.get(profile_name)?;
        let secret_config = profile_config.secrets.get(name)?;

        Some((secret_config.required, secret_config.default.clone()))
    }

    fn get_provider_backend(
        &self,
        provider_arg: Option<String>,
    ) -> Result<(String, &Box<dyn Provider>)> {
        let provider_name = if let Some(name) = provider_arg {
            name
        } else if let Some(global_config) = &self.global_config {
            global_config
                .projects
                .get(&self.config.project.name)
                .map(|p| p.provider.clone())
                .unwrap_or(global_config.defaults.provider.clone())
        } else {
            return Err(SecretSpecError::NoProviderConfigured);
        };

        let backend = self
            .registry
            .get(&provider_name)
            .ok_or_else(|| SecretSpecError::ProviderNotFound(provider_name.clone()))?;

        Ok((provider_name, backend))
    }

    pub fn init(&self, from: &Path) -> Result<()> {
        println!("Initializing secretspec.toml from {}...", from.display());

        let mut secrets = HashMap::new();

        if from.exists() {
            let env_vars = dotenvy::from_path_iter(from)?;
            for item in env_vars {
                let (key, _) = item?;
                secrets.insert(
                    key.clone(),
                    SecretConfig {
                        description: format!("{} secret", key),
                        required: true,
                        default: None,
                    },
                );
            }
        }

        let manifest = project_config_from_path(from)?;

        let content = toml::to_string_pretty(&manifest)?;
        fs::write("secretspec.toml", content)?;

        let secret_count = manifest
            .profiles
            .values()
            .map(|p| p.secrets.len())
            .sum::<usize>();
        println!(
            "{} Created secretspec.toml with {} secrets",
            "✓".green(),
            secret_count
        );

        if from.exists() {
            println!(
                "\n{} Remove {} after migrating secrets with:",
                "!".yellow(),
                from.display()
            );
            println!("  secretspec set <SECRET_NAME>");
        }

        Ok(())
    }

    pub fn set(
        &self,
        name: &str,
        value: Option<String>,
        provider_arg: Option<String>,
        profile: Option<String>,
    ) -> Result<()> {
        // Check if the secret exists in the spec
        let profile_name = profile.as_deref().unwrap_or("default");
        let profile_config = self.config.profiles.get(profile_name).ok_or_else(|| {
            SecretSpecError::SecretNotFound(format!(
                "Profile '{}' is not defined in secretspec.toml. Available profiles: {}",
                profile_name,
                self.config
                    .profiles
                    .keys()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;

        if !profile_config.secrets.contains_key(name) {
            return Err(SecretSpecError::SecretNotFound(format!(
                "Secret '{}' is not defined in profile '{}'. Available secrets: {}",
                name,
                profile_name,
                profile_config
                    .secrets
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }

        let (provider_name, backend) = self.get_provider_backend(provider_arg)?;
        let profile_display = profile.as_deref().unwrap_or("default");

        // Check if the provider supports setting values
        if !backend.allows_set() {
            return Err(SecretSpecError::ProviderOperationFailed(format!(
                "Provider '{}' is read-only and does not support setting values",
                provider_name
            )));
        }

        let value = if let Some(v) = value {
            v
        } else {
            print!("Enter value for {} (profile: {}): ", name, profile_display);
            io::stdout().flush()?;
            rpassword::read_password()?
        };

        backend.set(&self.config.project.name, name, &value, profile.as_deref())?;
        println!(
            "{} Secret '{}' saved to {} (profile: {})",
            "✓".green(),
            name,
            provider_name,
            profile_display
        );

        Ok(())
    }

    pub fn get(
        &self,
        name: &str,
        provider_arg: Option<String>,
        profile: Option<String>,
    ) -> Result<()> {
        let (_, backend) = self.get_provider_backend(provider_arg)?;
        let (_, default) = self
            .resolve_secret_config(name, profile.as_deref())
            .ok_or_else(|| SecretSpecError::SecretNotFound(name.to_string()))?;

        match backend.get(&self.config.project.name, name, profile.as_deref())? {
            Some(value) => {
                println!("{}", value);
                Ok(())
            }
            None => {
                if let Some(default_value) = default {
                    println!("{}", default_value);
                    Ok(())
                } else {
                    Err(SecretSpecError::SecretNotFound(name.to_string()))
                }
            }
        }
    }

    fn ensure_secrets(
        &self,
        provider_arg: Option<String>,
        profile: Option<String>,
        interactive: bool,
    ) -> Result<ValidationResult> {
        let (provider_name, backend) = self.get_provider_backend(provider_arg.clone())?;
        let profile_display = profile.as_deref().unwrap_or("default");

        // First validate to see what's missing
        let mut validation_result = self.validate(provider_arg.clone(), profile.clone())?;

        // If we're in interactive mode and have missing required secrets, prompt for them
        if interactive && !validation_result.missing_required.is_empty() {
            println!("\nThe following required secrets are missing:");
            for secret_name in &validation_result.missing_required {
                if let Some((_, config)) =
                    self.resolve_secret_config(secret_name, profile.as_deref())
                {
                    if let Some(profile_config) = self.config.profiles.get(profile_display) {
                        if let Some(secret_config) = profile_config.secrets.get(secret_name) {
                            println!("\n{} - {}", secret_name.bold(), secret_config.description);
                            print!(
                                "Enter value for {} (profile: {}): ",
                                secret_name, profile_display
                            );
                            io::stdout().flush()?;
                            let value = rpassword::read_password()?;

                            backend.set(
                                &self.config.project.name,
                                secret_name,
                                &value,
                                profile.as_deref(),
                            )?;
                            println!(
                                "{} Secret '{}' saved to {} (profile: {})",
                                "✓".green(),
                                secret_name,
                                provider_name,
                                profile_display
                            );
                        }
                    }
                }
            }

            println!("\nAll required secrets have been set.");

            // Re-validate to get the updated results
            validation_result = self.validate(provider_arg, profile)?;
        }

        // If we still have missing required secrets, fail
        if !validation_result.is_valid() {
            return Err(SecretSpecError::RequiredSecretMissing(
                validation_result.missing_required.join(", "),
            ));
        }

        Ok(validation_result)
    }

    pub fn check(&self, provider_arg: Option<String>, profile: Option<String>) -> Result<()> {
        let (provider_name, _) = self.get_provider_backend(provider_arg.clone())?;
        let profile_display = profile.as_deref().unwrap_or("default");

        println!(
            "Checking secrets in {} using {} (profile: {})...\n",
            self.config.project.name.bold(),
            provider_name.blue(),
            profile_display.cyan()
        );

        // First get the initial validation result to display status
        let initial_validation = self.validate(provider_arg.clone(), profile.clone())?;

        // Display status for each secret
        let profile_name = profile.as_deref().unwrap_or("default");
        let profile_config = self.config.profiles.get(profile_name).ok_or_else(|| {
            SecretSpecError::SecretNotFound(format!("Profile '{}' not found", profile_name))
        })?;

        for (name, config) in &profile_config.secrets {
            if initial_validation.secrets.contains_key(name) {
                if initial_validation
                    .with_defaults
                    .iter()
                    .any(|(n, _)| n == name)
                {
                    println!(
                        "{} {} - {} {}",
                        "○".yellow(),
                        name,
                        config.description,
                        "(has default)".yellow()
                    );
                } else {
                    println!("{} {} - {}", "✓".green(), name, config.description);
                }
            } else if initial_validation.missing_required.contains(name) {
                println!(
                    "{} {} - {} {}",
                    "✗".red(),
                    name,
                    config.description,
                    "(required)".red()
                );
            } else if initial_validation.missing_optional.contains(name) {
                println!(
                    "{} {} - {} {}",
                    "○".blue(),
                    name,
                    config.description,
                    "(optional)".blue()
                );
            }
        }

        let found_count = initial_validation.secrets.len() - initial_validation.with_defaults.len();
        let missing_count = initial_validation.missing_required.len();

        println!(
            "\nSummary: {} found, {} missing",
            found_count.to_string().green(),
            missing_count.to_string().red()
        );

        // Now ensure all secrets are present (will prompt if needed)
        self.ensure_secrets(provider_arg, profile, true)?;

        Ok(())
    }

    pub fn validate(
        &self,
        provider_arg: Option<String>,
        profile: Option<String>,
    ) -> Result<ValidationResult> {
        let (_, backend) = self.get_provider_backend(provider_arg)?;
        let mut secrets = HashMap::new();
        let mut missing_required = Vec::new();
        let mut missing_optional = Vec::new();
        let mut with_defaults = Vec::new();

        let profile_name = profile.as_deref().unwrap_or("default");
        let profile_config = self.config.profiles.get(profile_name).ok_or_else(|| {
            SecretSpecError::SecretNotFound(format!("Profile '{}' not found", profile_name))
        })?;

        for (name, _config) in &profile_config.secrets {
            let (required, default) = self
                .resolve_secret_config(name, profile.as_deref())
                .expect("Secret should exist in config since we're iterating over it");

            match backend.get(&self.config.project.name, name, profile.as_deref())? {
                Some(value) => {
                    secrets.insert(name.clone(), value);
                }
                None => {
                    if let Some(default_value) = default {
                        secrets.insert(name.clone(), default_value.clone());
                        with_defaults.push((name.clone(), default_value));
                    } else if required {
                        missing_required.push(name.clone());
                    } else {
                        missing_optional.push(name.clone());
                    }
                }
            }
        }

        Ok(ValidationResult {
            secrets,
            missing_required,
            missing_optional,
            with_defaults,
        })
    }

    pub fn run(
        &self,
        command: Vec<String>,
        provider_arg: Option<String>,
        profile: Option<String>,
    ) -> Result<()> {
        if command.is_empty() {
            return Err(SecretSpecError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "No command specified. Usage: secretspec run -- <command> [args...]",
            )));
        }

        // Ensure all secrets are available (will prompt for missing ones if needed)
        let validation_result = self.ensure_secrets(provider_arg, profile, true)?;

        let mut env_vars = env::vars().collect::<HashMap<_, _>>();
        env_vars.extend(validation_result.secrets);

        let mut cmd = Command::new(&command[0]);
        cmd.args(&command[1..]);
        cmd.envs(&env_vars);

        let status = cmd.status()?;
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn get_config_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("", "", "secretspec").ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "Could not find config directory")
    })?;
    Ok(dirs.config_dir().join("config.toml"))
}

pub fn parse_spec(path: &Path) -> Result<ProjectConfig> {
    let content = fs::read_to_string(path).map_err(|_| SecretSpecError::NoManifest)?;
    parse_spec_from_str(&content)
}

pub fn parse_spec_from_str(content: &str) -> Result<ProjectConfig> {
    let config: ProjectConfig = toml::from_str(content)?;

    // Validate revision
    if config.project.revision != "1.0" {
        return Err(SecretSpecError::UnsupportedRevision(
            config.project.revision,
        ));
    }

    Ok(config)
}

fn load_project_config() -> Result<ProjectConfig> {
    parse_spec(Path::new("secretspec.toml"))
}

fn load_global_config() -> Result<Option<GlobalConfig>> {
    let config_path = get_config_path()?;
    if !config_path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&config_path)?;
    Ok(Some(toml::from_str(&content)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_with_undefined_secret() {
        let project_config = ProjectConfig {
            project: ProjectInfo {
                name: "test_project".to_string(),
                revision: "1.0".to_string(),
                extends: None,
            },
            profiles: {
                let mut profiles = HashMap::new();
                let mut secrets = HashMap::new();
                secrets.insert(
                    "DEFINED_SECRET".to_string(),
                    SecretConfig {
                        description: "A defined secret".to_string(),
                        required: true,
                        default: None,
                    },
                );
                profiles.insert("default".to_string(), ProfileConfig { secrets });
                profiles
            },
        };

        let global_config = GlobalConfig {
            defaults: DefaultConfig {
                provider: "env".to_string(),
            },
            projects: HashMap::new(),
        };

        let spec = SecretSpec::new(project_config, Some(global_config));

        // Test setting an undefined secret - env provider is read-only,
        // but we should get the SecretNotFound error before the provider error
        let result = spec.set(
            "UNDEFINED_SECRET",
            Some("test_value".to_string()),
            Some("env".to_string()),
            None,
        );

        assert!(result.is_err());
        match result {
            Err(SecretSpecError::SecretNotFound(msg)) => {
                assert!(msg.contains("UNDEFINED_SECRET"));
                assert!(msg.contains("not defined in profile"));
                assert!(msg.contains("DEFINED_SECRET"));
            }
            _ => panic!("Expected SecretNotFound error"),
        }
    }

    #[test]
    fn test_set_with_defined_secret() {
        use std::env;
        use tempfile::TempDir;

        // Create a temporary directory for dotenv file
        let temp_dir = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&temp_dir).unwrap();

        let project_config = ProjectConfig {
            project: ProjectInfo {
                name: "test_project".to_string(),
                revision: "1.0".to_string(),
                extends: None,
            },
            profiles: {
                let mut profiles = HashMap::new();
                let mut secrets = HashMap::new();
                secrets.insert(
                    "DEFINED_SECRET".to_string(),
                    SecretConfig {
                        description: "A defined secret".to_string(),
                        required: true,
                        default: None,
                    },
                );
                profiles.insert("default".to_string(), ProfileConfig { secrets });
                profiles
            },
        };

        let global_config = GlobalConfig {
            defaults: DefaultConfig {
                provider: "dotenv".to_string(),
            },
            projects: HashMap::new(),
        };

        let spec = SecretSpec::new(project_config, Some(global_config));

        // This should succeed with dotenv provider
        let result = spec.set(
            "DEFINED_SECRET",
            Some("test_value".to_string()),
            Some("dotenv".to_string()),
            None,
        );

        // Restore original directory
        env::set_current_dir(original_dir).unwrap();

        // The set operation should succeed for a defined secret
        assert!(result.is_ok(), "Setting a defined secret should succeed");
    }

    #[test]
    fn test_set_with_readonly_provider() {
        let project_config = ProjectConfig {
            project: ProjectInfo {
                name: "test_project".to_string(),
                revision: "1.0".to_string(),
                extends: None,
            },
            profiles: {
                let mut profiles = HashMap::new();
                let mut secrets = HashMap::new();
                secrets.insert(
                    "DEFINED_SECRET".to_string(),
                    SecretConfig {
                        description: "A defined secret".to_string(),
                        required: true,
                        default: None,
                    },
                );
                profiles.insert("default".to_string(), ProfileConfig { secrets });
                profiles
            },
        };

        let global_config = GlobalConfig {
            defaults: DefaultConfig {
                provider: "env".to_string(),
            },
            projects: HashMap::new(),
        };

        let spec = SecretSpec::new(project_config, Some(global_config));

        // Test setting a defined secret with env provider (which is read-only)
        let result = spec.set(
            "DEFINED_SECRET",
            Some("test_value".to_string()),
            Some("env".to_string()),
            None,
        );

        assert!(result.is_err());
        match result {
            Err(SecretSpecError::ProviderOperationFailed(msg)) => {
                assert!(msg.contains("read-only"));
            }
            _ => panic!("Expected ProviderOperationFailed error for read-only provider"),
        }
    }
}
