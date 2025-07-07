use colored::Colorize;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
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

#[derive(Error, Debug)]
pub enum SecretSpecError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parsing error: {0}")]
    Toml(#[from] toml::de::Error),
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
}

pub type Result<T> = std::result::Result<T, SecretSpecError>;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectInfo,
    pub secrets: HashMap<String, SecretConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
}

impl ProjectConfig {
    pub fn from_path(from: &Path) -> Result<Self> {
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
                        profiles: HashMap::new(),
                    },
                );
            }
        }

        Ok(Self {
            project: ProjectInfo {
                name: std::env::current_dir()?
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            },
            secrets,
        })
    }

    pub fn get_example_toml() -> &'static str {
        r#"
# Example secrets configuration
# Uncomment and modify the sections you need

# [secrets.API_KEY]
# description = "API key for external service"
# required = true
#
# [secrets.API_KEY.development]
# required = false
# default = "dev-api-key"

# [secrets.DATABASE_URL]
# description = "Database connection string"
# required = true
#
# [secrets.DATABASE_URL.development]
# default = "sqlite:///dev.db"

# [secrets.JWT_SECRET]
# description = "Secret key for JWT token signing"
# required = true

# [secrets.REDIS_URL]
# description = "Redis connection URL for caching"
# required = false
# default = "redis://localhost:6379"

# [secrets.EMAIL_PROVIDER]
# description = "Email service provider"
# required = false
# default = "smtp"

# [secrets.OAUTH_CLIENT_ID]
# description = "OAuth client ID"
# required = false

# [secrets.OAUTH_CLIENT_SECRET]
# description = "OAuth client secret"
# required = false
"#
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SecretConfig {
    pub description: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(default, flatten)]
    pub profiles: HashMap<String, ProfileOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectUserConfig {
    pub provider: String,
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
        let secret_config = self.config.secrets.get(name)?;

        if let Some(prof) = profile {
            if let Some(profile_override) = secret_config.profiles.get(prof) {
                let required = profile_override.required.unwrap_or(secret_config.required);
                let default = profile_override
                    .default
                    .clone()
                    .or_else(|| secret_config.default.clone());
                return Some((required, default));
            }
        }

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
                        profiles: HashMap::new(),
                    },
                );
            }
        }

        let manifest = ProjectConfig::from_path(from)?;

        let content = toml::to_string_pretty(&manifest)?;
        fs::write("secretspec.toml", content)?;

        println!(
            "{} Created secretspec.toml with {} secrets",
            "✓".green(),
            manifest.secrets.len()
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
        let (provider_name, backend) = self.get_provider_backend(provider_arg)?;
        let profile_display = profile.as_deref().unwrap_or("default");

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

    pub fn check(&self, provider_arg: Option<String>, profile: Option<String>) -> Result<()> {
        let (provider_name, backend) = self.get_provider_backend(provider_arg)?;
        let profile_display = profile.as_deref().unwrap_or("default");

        println!(
            "Checking secrets in {} using {} (profile: {})...\n",
            self.config.project.name.bold(),
            provider_name.blue(),
            profile_display.cyan()
        );

        let mut missing = vec![];
        let mut found = vec![];

        for (name, config) in &self.config.secrets {
            let (required, default) = self
                .resolve_secret_config(name, profile.as_deref())
                .unwrap();

            match backend.get(&self.config.project.name, name, profile.as_deref())? {
                Some(_) => {
                    found.push(name);
                    println!("{} {} - {}", "✓".green(), name, config.description);
                }
                None => {
                    if required && default.is_none() {
                        missing.push(name);
                        println!(
                            "{} {} - {} {}",
                            "✗".red(),
                            name,
                            config.description,
                            "(required)".red()
                        );
                    } else if default.is_some() {
                        println!(
                            "{} {} - {} {}",
                            "○".yellow(),
                            name,
                            config.description,
                            "(has default)".yellow()
                        );
                    } else {
                        println!(
                            "{} {} - {} {}",
                            "○".blue(),
                            name,
                            config.description,
                            "(optional)".blue()
                        );
                    }
                }
            }
        }

        println!(
            "\nSummary: {} found, {} missing",
            found.len().to_string().green(),
            missing.len().to_string().red()
        );

        if !missing.is_empty() {
            return Err(SecretSpecError::RequiredSecretMissing(
                missing[0].to_string(),
            ));
        }

        Ok(())
    }


    pub fn validate(
        &self,
        provider_arg: Option<String>,
        profile: Option<String>,
    ) -> Result<HashMap<String, String>> {
        let (_, backend) = self.get_provider_backend(provider_arg)?;
        let mut secrets = HashMap::new();

        for (name, _config) in &self.config.secrets {
            let (required, default) = self
                .resolve_secret_config(name, profile.as_deref())
                .unwrap();

            match backend.get(&self.config.project.name, name, profile.as_deref())? {
                Some(value) => {
                    secrets.insert(name.clone(), value);
                }
                None => {
                    if let Some(default_value) = default {
                        secrets.insert(name.clone(), default_value);
                    } else if required {
                        return Err(SecretSpecError::RequiredSecretMissing(name.clone()));
                    }
                }
            }
        }

        Ok(secrets)
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
                "No command specified",
            )));
        }

        let secrets = self.validate(provider_arg, profile)?;

        let mut env_vars = env::vars().collect::<HashMap<_, _>>();
        env_vars.extend(secrets);

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

fn load_project_config() -> Result<ProjectConfig> {
    let content = fs::read_to_string("secretspec.toml").map_err(|_| SecretSpecError::NoManifest)?;
    Ok(toml::from_str(&content)?)
}

fn load_global_config() -> Result<Option<GlobalConfig>> {
    let config_path = get_config_path()?;
    if !config_path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&config_path)?;
    Ok(Some(toml::from_str(&content)?))
}
