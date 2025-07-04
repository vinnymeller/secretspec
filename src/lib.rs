use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use thiserror::Error;

mod storage;
use storage::{StorageBackend, StorageRegistry};

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
    #[error("No storage backend configured. Use --storage or configure user settings")]
    NoStorageConfigured,
    #[error("Storage backend '{0}' not found")]
    StorageNotFound(String),
    #[error("Secret '{0}' not found")]
    SecretNotFound(String),
    #[error("Secret '{0}' is required but not set")]
    RequiredSecretMissing(String),
    #[error("No secretspec.toml found in current directory")]
    NoManifest,
    #[error("Project name not found in secretspec.toml")]
    NoProjectName,
    #[error("Storage operation failed: {0}")]
    StorageOperationFailed(String),
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
                        environments: HashMap::new(),
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
    pub environments: HashMap<String, EnvironmentOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentOverride {
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
    pub storage: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectUserConfig {
    pub storage: String,
}

pub struct SecretSpec {
    registry: StorageRegistry,
    config: ProjectConfig,
    global_config: Option<GlobalConfig>,
}

impl SecretSpec {
    pub fn new(config: ProjectConfig, global_config: Option<GlobalConfig>) -> Self {
        Self {
            registry: StorageRegistry::new(),
            config,
            global_config,
        }
    }

    fn resolve_secret_config(&self, name: &str, environment: Option<&str>) -> Option<(bool, Option<String>)> {
        let secret_config = self.config.secrets.get(name)?;
        
        if let Some(env) = environment {
            if let Some(env_override) = secret_config.environments.get(env) {
                let required = env_override.required.unwrap_or(secret_config.required);
                let default = env_override.default.clone().or_else(|| secret_config.default.clone());
                return Some((required, default));
            }
        }
        
        Some((secret_config.required, secret_config.default.clone()))
    }

    fn get_storage_backend(
        &self,
        storage_arg: Option<String>,
    ) -> Result<(String, &Box<dyn StorageBackend>)> {
        let storage_name = if let Some(name) = storage_arg {
            name
        } else if let Some(global_config) = &self.global_config {
            global_config
                .projects
                .get(&self.config.project.name)
                .map(|p| p.storage.clone())
                .unwrap_or(global_config.defaults.storage.clone())
        } else {
            return Err(SecretSpecError::NoStorageConfigured);
        };

        let backend = self
            .registry
            .get(&storage_name)
            .ok_or_else(|| SecretSpecError::StorageNotFound(storage_name.clone()))?;

        Ok((storage_name, backend))
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
                        environments: HashMap::new(),
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
        storage_arg: Option<String>,
        environment: Option<String>,
    ) -> Result<()> {
        let (storage_name, backend) = self.get_storage_backend(storage_arg)?;
        let env_display = environment.as_deref().unwrap_or("default");

        let value = if let Some(v) = value {
            v
        } else {
            print!("Enter value for {} (env: {}): ", name, env_display);
            io::stdout().flush()?;
            rpassword::read_password()?
        };

        backend.set(&self.config.project.name, name, &value)?;
        println!(
            "{} Secret '{}' saved to {} (env: {})",
            "✓".green(),
            name,
            storage_name,
            env_display
        );

        Ok(())
    }

    pub fn get(&self, name: &str, storage_arg: Option<String>, environment: Option<String>) -> Result<()> {
        let (_, backend) = self.get_storage_backend(storage_arg)?;
        let (_, default) = self.resolve_secret_config(name, environment.as_deref())
            .ok_or_else(|| SecretSpecError::SecretNotFound(name.to_string()))?;

        match backend.get(&self.config.project.name, name)? {
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

    pub fn check(&self, storage_arg: Option<String>, environment: Option<String>) -> Result<()> {
        let (storage_name, backend) = self.get_storage_backend(storage_arg)?;
        let env_display = environment.as_deref().unwrap_or("default");

        println!(
            "Checking secrets in {} using {} (env: {})...\n",
            self.config.project.name.bold(),
            storage_name.blue(),
            env_display.cyan()
        );

        let mut missing = vec![];
        let mut found = vec![];

        for (name, config) in &self.config.secrets {
            let (required, default) = self.resolve_secret_config(name, environment.as_deref()).unwrap();
            
            match backend.get(&self.config.project.name, name)? {
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
            return Err(SecretSpecError::RequiredSecretMissing(missing[0].to_string()));
        }

        Ok(())
    }

    pub fn run(&self, command: Vec<String>, storage_arg: Option<String>, environment: Option<String>) -> Result<()> {
        if command.is_empty() {
            return Err(SecretSpecError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "No command specified",
            )));
        }

        let (_, backend) = self.get_storage_backend(storage_arg)?;

        let mut env_vars = env::vars().collect::<HashMap<_, _>>();

        for (name, _config) in &self.config.secrets {
            let (required, default) = self.resolve_secret_config(name, environment.as_deref()).unwrap();
            
            match backend.get(&self.config.project.name, name)? {
                Some(value) => {
                    env_vars.insert(name.clone(), value);
                }
                None => {
                    if let Some(default_value) = default {
                        env_vars.insert(name.clone(), default_value);
                    } else if required {
                        return Err(SecretSpecError::RequiredSecretMissing(name.clone()));
                    }
                }
            }
        }

        let mut cmd = Command::new(&command[0]);
        cmd.args(&command[1..]);
        cmd.envs(&env_vars);

        let status = cmd.status()?;
        std::process::exit(status.code().unwrap_or(1));
    }
}
