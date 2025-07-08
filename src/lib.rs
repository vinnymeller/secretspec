use colored::Colorize;
use directories::ProjectDirs;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

pub mod provider;
use provider::{Provider as ProviderTrait, ProviderRegistry};

#[cfg(feature = "macros")]
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
    pub provider: secretspec_types::Provider,
    pub profile: String,
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

# API_KEY = { description = "API key for external service", required = true }
# DATABASE_URL = { description = "Database connection string", required = true }

# Extend configurations from subdirectories
# extends = [ "subdir1", "subdir2" ]

[profiles.development]
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

    fn resolve_profile<'a>(&'a self, profile: Option<&'a str>) -> &'a str {
        profile.unwrap_or_else(|| {
            self.global_config
                .as_ref()
                .and_then(|gc| gc.defaults.profile.as_deref())
                .unwrap_or("default")
        })
    }

    fn resolve_secret_config(
        &self,
        name: &str,
        profile: Option<&str>,
    ) -> Option<(bool, Option<String>)> {
        let profile_name = self.resolve_profile(profile);
        let profile_config = self.config.profiles.get(profile_name)?;
        let secret_config = profile_config.secrets.get(name)?;

        Some((secret_config.required, secret_config.default.clone()))
    }

    fn get_provider_backend(
        &self,
        provider_arg: Option<String>,
    ) -> Result<(String, &Box<dyn ProviderTrait>)> {
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
        let profile_name = self.resolve_profile(profile.as_deref());
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
        let profile_display = self.resolve_profile(profile.as_deref());

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
        let profile_display = self.resolve_profile(profile.as_deref());

        // First validate to see what's missing
        let mut validation_result = self.validate(provider_arg.clone(), profile.clone())?;

        // If we're in interactive mode and have missing required secrets, prompt for them
        if interactive && !validation_result.missing_required.is_empty() {
            println!("\nThe following required secrets are missing:");
            for secret_name in &validation_result.missing_required {
                if let Some((_, _config)) =
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
        let profile_display = self.resolve_profile(profile.as_deref());

        println!(
            "Checking secrets in {} using {} (profile: {})...\n",
            self.config.project.name.bold(),
            provider_name.blue(),
            profile_display.cyan()
        );

        // First get the initial validation result to display status
        let initial_validation = self.validate(provider_arg.clone(), profile.clone())?;

        // Display status for each secret
        let profile_name = self.resolve_profile(profile.as_deref());
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
        let (provider_name, backend) = self.get_provider_backend(provider_arg)?;
        let mut secrets = HashMap::new();
        let mut missing_required = Vec::new();
        let mut missing_optional = Vec::new();
        let mut with_defaults = Vec::new();

        let profile_name = self.resolve_profile(profile.as_deref());
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

        let provider = secretspec_types::Provider::from_str(&provider_name)
            .ok_or_else(|| SecretSpecError::ProviderNotFound(provider_name.clone()))?;

        Ok(ValidationResult {
            secrets,
            missing_required,
            missing_optional,
            with_defaults,
            provider,
            profile: profile_name.to_string(),
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
    secretspec_types::parse_spec(path).map_err(|e| match e {
        secretspec_types::ParseError::Io(io_err) => {
            if io_err.kind() == io::ErrorKind::NotFound {
                SecretSpecError::NoManifest
            } else {
                SecretSpecError::Io(io_err)
            }
        }
        secretspec_types::ParseError::Toml(toml_err) => SecretSpecError::Toml(toml_err),
        secretspec_types::ParseError::UnsupportedRevision(rev) => {
            SecretSpecError::UnsupportedRevision(rev)
        }
        secretspec_types::ParseError::CircularDependency(msg) => {
            SecretSpecError::Io(io::Error::new(io::ErrorKind::InvalidData, msg))
        }
    })
}

pub fn parse_spec_from_str(content: &str, base_path: Option<&Path>) -> Result<ProjectConfig> {
    secretspec_types::parse_spec_from_str(content, base_path).map_err(|e| match e {
        secretspec_types::ParseError::Io(io_err) => SecretSpecError::Io(io_err),
        secretspec_types::ParseError::Toml(toml_err) => SecretSpecError::Toml(toml_err),
        secretspec_types::ParseError::UnsupportedRevision(rev) => {
            SecretSpecError::UnsupportedRevision(rev)
        }
        secretspec_types::ParseError::CircularDependency(msg) => {
            SecretSpecError::Io(io::Error::new(io::ErrorKind::InvalidData, msg))
        }
    })
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
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_extends_functionality() {
        // Create temporary directory structure for testing
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create directory structure
        fs::create_dir_all(base_path.join("common")).unwrap();
        fs::create_dir_all(base_path.join("auth")).unwrap();
        fs::create_dir_all(base_path.join("base")).unwrap();

        // Create common config
        let common_config = r#"
[project]
name = "common"
revision = "1.0"

[profiles.default]
DATABASE_URL = { description = "Database connection string", required = true }
REDIS_URL = { description = "Redis connection URL", required = false, default = "redis://localhost:6379" }

[profiles.development]
DATABASE_URL = { description = "Database connection string", required = false, default = "sqlite:///dev.db" }
REDIS_URL = { description = "Redis connection URL", required = false, default = "redis://localhost:6379" }
"#;
        fs::write(base_path.join("common/secretspec.toml"), common_config).unwrap();

        // Create auth config
        let auth_config = r#"
[project]
name = "auth"
revision = "1.0"

[profiles.default]
JWT_SECRET = { description = "Secret key for JWT token signing", required = true }
OAUTH_CLIENT_ID = { description = "OAuth client ID", required = false }
"#;
        fs::write(base_path.join("auth/secretspec.toml"), auth_config).unwrap();

        // Create base config that extends from common and auth
        let base_config = r#"
[project]
name = "test_project"
revision = "1.0"
extends = ["../common", "../auth"]

[profiles.default]
API_KEY = { description = "API key for external service", required = true }
# This should override the common one
DATABASE_URL = { description = "Override database connection", required = true }

[profiles.development]
API_KEY = { description = "API key for external service", required = false, default = "dev-api-key" }
"#;
        fs::write(base_path.join("base/secretspec.toml"), base_config).unwrap();

        // Parse the config
        let config = secretspec_types::parse_spec(&base_path.join("base/secretspec.toml")).unwrap();

        // Verify the config has merged correctly
        assert_eq!(config.project.name, "test_project");
        assert_eq!(config.project.revision, "1.0");
        assert_eq!(
            config.project.extends,
            Some(vec!["../common".to_string(), "../auth".to_string()])
        );

        // Check that all secrets are present
        let default_profile = config.profiles.get("default").unwrap();
        assert!(default_profile.secrets.contains_key("API_KEY"));
        assert!(default_profile.secrets.contains_key("DATABASE_URL"));
        assert!(default_profile.secrets.contains_key("REDIS_URL"));
        assert!(default_profile.secrets.contains_key("JWT_SECRET"));
        assert!(default_profile.secrets.contains_key("OAUTH_CLIENT_ID"));

        // Check that base config takes precedence (DATABASE_URL should be overridden)
        let database_url_config = default_profile.secrets.get("DATABASE_URL").unwrap();
        assert_eq!(
            database_url_config.description,
            "Override database connection"
        );

        // Check that extended secrets are included
        let redis_config = default_profile.secrets.get("REDIS_URL").unwrap();
        assert_eq!(redis_config.description, "Redis connection URL");
        assert!(!redis_config.required);
        assert_eq!(
            redis_config.default,
            Some("redis://localhost:6379".to_string())
        );

        let jwt_config = default_profile.secrets.get("JWT_SECRET").unwrap();
        assert_eq!(jwt_config.description, "Secret key for JWT token signing");
        assert!(jwt_config.required);
    }

    #[test]
    fn test_extends_with_real_world_example() {
        // Test a real-world scenario with multiple extends and profile overrides
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create directory structure
        fs::create_dir_all(base_path.join("common")).unwrap();
        fs::create_dir_all(base_path.join("auth")).unwrap();
        fs::create_dir_all(base_path.join("base")).unwrap();

        // Create common config with database and cache settings
        let common_config = r#"
[project]
name = "common"
revision = "1.0"

[profiles.default]
DATABASE_URL = { description = "Main database connection string", required = true }
REDIS_URL = { description = "Redis cache connection", required = false, default = "redis://localhost:6379" }

[profiles.development]
DATABASE_URL = { description = "Development database", required = false, default = "sqlite:///dev.db" }
REDIS_URL = { description = "Redis cache connection", required = false, default = "redis://localhost:6379" }

[profiles.production]
DATABASE_URL = { description = "Production database", required = true }
REDIS_URL = { description = "Redis cache connection", required = true }
"#;
        fs::write(base_path.join("common/secretspec.toml"), common_config).unwrap();

        // Create auth config with authentication settings
        let auth_config = r#"
[project]
name = "auth"
revision = "1.0"

[profiles.default]
JWT_SECRET = { description = "Secret for JWT signing", required = true }
OAUTH_CLIENT_ID = { description = "OAuth client identifier", required = false }
OAUTH_CLIENT_SECRET = { description = "OAuth client secret", required = false }

[profiles.production]
JWT_SECRET = { description = "Secret for JWT signing", required = true }
OAUTH_CLIENT_ID = { description = "OAuth client identifier", required = true }
OAUTH_CLIENT_SECRET = { description = "OAuth client secret", required = true }
"#;
        fs::write(base_path.join("auth/secretspec.toml"), auth_config).unwrap();

        // Create base config that extends from both common and auth
        let base_config = r#"
[project]
name = "my_app"
revision = "1.0"
extends = ["../common", "../auth"]

[profiles.default]
API_KEY = { description = "External API key", required = true }
# Override the database description from common
DATABASE_URL = { description = "Custom database for my app", required = true }

[profiles.development]
API_KEY = { description = "External API key", required = false, default = "dev-key-123" }

[profiles.production]
API_KEY = { description = "External API key", required = true }
MONITORING_TOKEN = { description = "Token for monitoring service", required = true }
"#;
        fs::write(base_path.join("base/secretspec.toml"), base_config).unwrap();

        // Parse the config
        let config = secretspec_types::parse_spec(&base_path.join("base/secretspec.toml")).unwrap();

        // Verify project info
        assert_eq!(config.project.name, "my_app");
        assert_eq!(config.project.revision, "1.0");
        assert_eq!(
            config.project.extends,
            Some(vec!["../common".to_string(), "../auth".to_string()])
        );

        // Verify default profile has all merged secrets
        let default_profile = config.profiles.get("default").unwrap();
        assert_eq!(default_profile.secrets.len(), 6); // API_KEY, DATABASE_URL, REDIS_URL, JWT_SECRET, OAUTH_CLIENT_ID, OAUTH_CLIENT_SECRET

        // Verify base config overrides common config
        let database_url = default_profile.secrets.get("DATABASE_URL").unwrap();
        assert_eq!(database_url.description, "Custom database for my app");
        assert!(database_url.required);

        // Verify inherited secrets from common
        let redis_url = default_profile.secrets.get("REDIS_URL").unwrap();
        assert_eq!(redis_url.description, "Redis cache connection");
        assert!(!redis_url.required);
        assert_eq!(
            redis_url.default,
            Some("redis://localhost:6379".to_string())
        );

        // Verify inherited secrets from auth
        let jwt_secret = default_profile.secrets.get("JWT_SECRET").unwrap();
        assert_eq!(jwt_secret.description, "Secret for JWT signing");
        assert!(jwt_secret.required);

        // Verify development profile
        let dev_profile = config.profiles.get("development").unwrap();
        let dev_api_key = dev_profile.secrets.get("API_KEY").unwrap();
        assert!(!dev_api_key.required);
        assert_eq!(dev_api_key.default, Some("dev-key-123".to_string()));

        let dev_database_url = dev_profile.secrets.get("DATABASE_URL").unwrap();
        assert_eq!(dev_database_url.description, "Development database");
        assert!(!dev_database_url.required);
        assert_eq!(
            dev_database_url.default,
            Some("sqlite:///dev.db".to_string())
        );

        // Verify production profile has all required secrets
        let prod_profile = config.profiles.get("production").unwrap();
        assert!(prod_profile.secrets.get("API_KEY").unwrap().required);
        assert!(prod_profile.secrets.get("DATABASE_URL").unwrap().required);
        assert!(prod_profile.secrets.get("REDIS_URL").unwrap().required);
        assert!(prod_profile.secrets.get("JWT_SECRET").unwrap().required);
        assert!(
            prod_profile
                .secrets
                .get("OAUTH_CLIENT_ID")
                .unwrap()
                .required
        );
        assert!(
            prod_profile
                .secrets
                .get("OAUTH_CLIENT_SECRET")
                .unwrap()
                .required
        );
        assert!(
            prod_profile
                .secrets
                .get("MONITORING_TOKEN")
                .unwrap()
                .required
        );
    }

    #[test]
    fn test_extends_with_direct_circular_dependency() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create directory structure
        fs::create_dir_all(base_path.join("a")).unwrap();
        fs::create_dir_all(base_path.join("b")).unwrap();

        // Create config A that extends B
        let config_a = r#"
[project]
name = "config_a"
revision = "1.0"
extends = ["../b"]

[profiles.default]
SECRET_A = { description = "Secret A", required = true }
"#;
        fs::write(base_path.join("a/secretspec.toml"), config_a).unwrap();

        // Create config B that extends A (circular dependency)
        let config_b = r#"
[project]
name = "config_b"
revision = "1.0"
extends = ["../a"]

[profiles.default]
SECRET_B = { description = "Secret B", required = true }
"#;
        fs::write(base_path.join("b/secretspec.toml"), config_b).unwrap();

        // Parse should fail with circular dependency error
        let result = secretspec_types::parse_spec(&base_path.join("a/secretspec.toml"));
        assert!(result.is_err());
        match result {
            Err(secretspec_types::ParseError::CircularDependency(msg)) => {
                assert!(msg.contains("circular dependency"));
            }
            _ => panic!("Expected CircularDependency error"),
        }
    }

    #[test]
    fn test_extends_with_indirect_circular_dependency() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create directory structure
        fs::create_dir_all(base_path.join("a")).unwrap();
        fs::create_dir_all(base_path.join("b")).unwrap();
        fs::create_dir_all(base_path.join("c")).unwrap();

        // Create config A that extends B
        let config_a = r#"
[project]
name = "config_a"
revision = "1.0"
extends = ["../b"]

[profiles.default]
SECRET_A = { description = "Secret A", required = true }
"#;
        fs::write(base_path.join("a/secretspec.toml"), config_a).unwrap();

        // Create config B that extends C
        let config_b = r#"
[project]
name = "config_b"
revision = "1.0"
extends = ["../c"]

[profiles.default]
SECRET_B = { description = "Secret B", required = true }
"#;
        fs::write(base_path.join("b/secretspec.toml"), config_b).unwrap();

        // Create config C that extends A (circular dependency through chain)
        let config_c = r#"
[project]
name = "config_c"
revision = "1.0"
extends = ["../a"]

[profiles.default]
SECRET_C = { description = "Secret C", required = true }
"#;
        fs::write(base_path.join("c/secretspec.toml"), config_c).unwrap();

        // Parse should fail with circular dependency error
        let result = secretspec_types::parse_spec(&base_path.join("a/secretspec.toml"));
        assert!(result.is_err());
        match result {
            Err(secretspec_types::ParseError::CircularDependency(msg)) => {
                assert!(msg.contains("circular dependency"));
            }
            _ => panic!("Expected CircularDependency error"),
        }
    }

    #[test]
    fn test_nested_extends() {
        // Test A extends B, B extends C scenario
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create directory structure
        fs::create_dir_all(base_path.join("a")).unwrap();
        fs::create_dir_all(base_path.join("b")).unwrap();
        fs::create_dir_all(base_path.join("c")).unwrap();

        // Create config C (base config)
        let config_c = r#"
[project]
name = "config_c"
revision = "1.0"

[profiles.default]
SECRET_C = { description = "Secret C from base", required = true }
COMMON_SECRET = { description = "Common secret from C", required = true }

[profiles.production]
SECRET_C = { description = "Secret C for production", required = true }
"#;
        fs::write(base_path.join("c/secretspec.toml"), config_c).unwrap();

        // Create config B that extends C
        let config_b = r#"
[project]
name = "config_b"
revision = "1.0"
extends = ["../c"]

[profiles.default]
SECRET_B = { description = "Secret B", required = true }
COMMON_SECRET = { description = "Common secret overridden by B", required = false, default = "default-b" }

[profiles.staging]
SECRET_B = { description = "Secret B for staging", required = true }
"#;
        fs::write(base_path.join("b/secretspec.toml"), config_b).unwrap();

        // Create config A that extends B (which extends C)
        let config_a = r#"
[project]
name = "config_a"
revision = "1.0"
extends = ["../b"]

[profiles.default]
SECRET_A = { description = "Secret A", required = true }

[profiles.staging]
SECRET_A = { description = "Secret A for staging", required = false, default = "staging-a" }
"#;
        fs::write(base_path.join("a/secretspec.toml"), config_a).unwrap();

        // Parse config A
        let config = secretspec_types::parse_spec(&base_path.join("a/secretspec.toml")).unwrap();

        // Verify project info
        assert_eq!(config.project.name, "config_a");

        // Verify default profile has all secrets from A, B, and C
        let default_profile = config.profiles.get("default").unwrap();
        assert_eq!(default_profile.secrets.len(), 4); // SECRET_A, SECRET_B, SECRET_C, COMMON_SECRET

        // Verify secrets are inherited correctly
        assert!(default_profile.secrets.contains_key("SECRET_A"));
        assert!(default_profile.secrets.contains_key("SECRET_B"));
        assert!(default_profile.secrets.contains_key("SECRET_C"));
        assert!(default_profile.secrets.contains_key("COMMON_SECRET"));

        // Verify B's override of COMMON_SECRET takes precedence over C's
        let common_secret = default_profile.secrets.get("COMMON_SECRET").unwrap();
        assert_eq!(common_secret.description, "Common secret overridden by B");
        assert!(!common_secret.required);
        assert_eq!(common_secret.default, Some("default-b".to_string()));

        // Verify staging profile exists from both A and B
        let staging_profile = config.profiles.get("staging").unwrap();
        assert!(staging_profile.secrets.contains_key("SECRET_A"));
        assert!(staging_profile.secrets.contains_key("SECRET_B"));

        // Verify production profile exists only from C
        let prod_profile = config.profiles.get("production").unwrap();
        assert!(prod_profile.secrets.contains_key("SECRET_C"));
        assert!(!prod_profile.secrets.contains_key("SECRET_A")); // A doesn't define production
        assert!(!prod_profile.secrets.contains_key("SECRET_B")); // B doesn't define production
    }

    #[test]
    fn test_extends_with_path_resolution_edge_cases() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create complex directory structure
        fs::create_dir_all(base_path.join("project/src")).unwrap();
        fs::create_dir_all(base_path.join("shared/common")).unwrap();
        fs::create_dir_all(base_path.join("shared/auth")).unwrap();

        // Create common config
        let common_config = r#"
[project]
name = "common"
revision = "1.0"

[profiles.default]
COMMON_SECRET = { description = "Common secret", required = true }
"#;
        fs::write(
            base_path.join("shared/common/secretspec.toml"),
            common_config,
        )
        .unwrap();

        // Create auth config
        let auth_config = r#"
[project]
name = "auth"
revision = "1.0"

[profiles.default]
AUTH_SECRET = { description = "Auth secret", required = true }
"#;
        fs::write(base_path.join("shared/auth/secretspec.toml"), auth_config).unwrap();

        // Test 1: Relative path with ../..
        let config_relative = r#"
[project]
name = "project"
revision = "1.0"
extends = ["../../shared/common", "../../shared/auth"]

[profiles.default]
PROJECT_SECRET = { description = "Project secret", required = true }
"#;
        fs::write(
            base_path.join("project/src/secretspec.toml"),
            config_relative,
        )
        .unwrap();

        let config =
            secretspec_types::parse_spec(&base_path.join("project/src/secretspec.toml")).unwrap();
        let default_profile = config.profiles.get("default").unwrap();
        assert_eq!(default_profile.secrets.len(), 3);
        assert!(default_profile.secrets.contains_key("COMMON_SECRET"));
        assert!(default_profile.secrets.contains_key("AUTH_SECRET"));
        assert!(default_profile.secrets.contains_key("PROJECT_SECRET"));

        // Test 2: Path with ./ prefix
        let config_dot_slash = r#"
[project]
name = "project2"
revision = "1.0"
extends = ["./../../shared/common"]

[profiles.default]
PROJECT2_SECRET = { description = "Project2 secret", required = true }
"#;
        fs::write(
            base_path.join("project/src/secretspec2.toml"),
            config_dot_slash,
        )
        .unwrap();

        let config2 =
            secretspec_types::parse_spec(&base_path.join("project/src/secretspec2.toml")).unwrap();
        let default_profile2 = config2.profiles.get("default").unwrap();
        assert_eq!(default_profile2.secrets.len(), 2);
        assert!(default_profile2.secrets.contains_key("COMMON_SECRET"));
        assert!(default_profile2.secrets.contains_key("PROJECT2_SECRET"));

        // Test 3: Path with spaces (if supported by the OS)
        let dir_with_spaces = base_path.join("dir with spaces");
        if fs::create_dir_all(&dir_with_spaces).is_ok() {
            let config_spaces = r#"
[project]
name = "spaces"
revision = "1.0"

[profiles.default]
SPACE_SECRET = { description = "Secret in dir with spaces", required = true }
"#;
            fs::write(dir_with_spaces.join("secretspec.toml"), config_spaces).unwrap();

            let config_extends_spaces = r#"
[project]
name = "project3"
revision = "1.0"
extends = ["../dir with spaces"]

[profiles.default]
PROJECT3_SECRET = { description = "Project3 secret", required = true }
"#;
            fs::write(
                base_path.join("project/secretspec3.toml"),
                config_extends_spaces,
            )
            .unwrap();

            let config3 =
                secretspec_types::parse_spec(&base_path.join("project/secretspec3.toml")).unwrap();
            let default_profile3 = config3.profiles.get("default").unwrap();
            assert_eq!(default_profile3.secrets.len(), 2);
            assert!(default_profile3.secrets.contains_key("SPACE_SECRET"));
            assert!(default_profile3.secrets.contains_key("PROJECT3_SECRET"));
        }
    }

    #[test]
    fn test_empty_extends_array() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create config with empty extends array
        let config_empty_extends = r#"
[project]
name = "project"
revision = "1.0"
extends = []

[profiles.default]
SECRET_A = { description = "Secret A", required = true }

[profiles.production]
SECRET_B = { description = "Secret B", required = false, default = "prod-b" }
"#;
        fs::write(base_path.join("secretspec.toml"), config_empty_extends).unwrap();

        // Parse should succeed with empty extends
        let config = secretspec_types::parse_spec(&base_path.join("secretspec.toml")).unwrap();

        // Verify config is parsed correctly
        assert_eq!(config.project.name, "project");
        assert_eq!(config.project.extends, Some(vec![]));

        // Verify profiles and secrets are intact
        let default_profile = config.profiles.get("default").unwrap();
        assert_eq!(default_profile.secrets.len(), 1);
        assert!(default_profile.secrets.contains_key("SECRET_A"));

        let prod_profile = config.profiles.get("production").unwrap();
        assert_eq!(prod_profile.secrets.len(), 1);
        assert!(prod_profile.secrets.contains_key("SECRET_B"));
    }

    #[test]
    fn test_self_extension() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Test 1: Config that tries to extend itself with "."
        let config_self_dot = r#"
[project]
name = "self_extend"
revision = "1.0"
extends = ["."]

[profiles.default]
SECRET_A = { description = "Secret A", required = true }
"#;
        fs::write(base_path.join("secretspec.toml"), config_self_dot).unwrap();

        // This should fail with circular dependency
        let result = secretspec_types::parse_spec(&base_path.join("secretspec.toml"));
        assert!(result.is_err());
        match result {
            Err(secretspec_types::ParseError::CircularDependency(msg)) => {
                assert!(msg.contains("circular dependency"));
            }
            _ => panic!("Expected CircularDependency error for self-extension"),
        }

        // Test 2: Config in subdirectory that tries to extend its parent which extends it back
        fs::create_dir_all(base_path.join("subdir")).unwrap();

        let parent_config = r#"
[project]
name = "parent"
revision = "1.0"
extends = ["./subdir"]

[profiles.default]
PARENT_SECRET = { description = "Parent secret", required = true }
"#;
        fs::write(base_path.join("secretspec.toml"), parent_config).unwrap();

        let child_config = r#"
[project]
name = "child"
revision = "1.0"
extends = [".."]

[profiles.default]
CHILD_SECRET = { description = "Child secret", required = true }
"#;
        fs::write(base_path.join("subdir/secretspec.toml"), child_config).unwrap();

        // This should also fail with circular dependency
        let result2 = secretspec_types::parse_spec(&base_path.join("secretspec.toml"));
        assert!(result2.is_err());
        match result2 {
            Err(secretspec_types::ParseError::CircularDependency(msg)) => {
                assert!(msg.contains("circular dependency"));
            }
            _ => panic!("Expected CircularDependency error for parent-child circular reference"),
        }
    }

    #[test]
    fn test_property_overrides() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create directory structure
        fs::create_dir_all(base_path.join("base")).unwrap();
        fs::create_dir_all(base_path.join("override")).unwrap();

        // Create base config with various secret properties
        let base_config = r#"
[project]
name = "base"
revision = "1.0"

[profiles.default]
SECRET_A = { description = "Original description A", required = true }
SECRET_B = { description = "Original description B", required = true, default = "original-b" }
SECRET_C = { description = "Original description C", required = false }
SECRET_D = { description = "Original description D", required = false, default = "original-d" }
"#;
        fs::write(base_path.join("base/secretspec.toml"), base_config).unwrap();

        // Create override config that selectively overrides properties
        let override_config = r#"
[project]
name = "override"
revision = "1.0"
extends = ["../base"]

[profiles.default]
# Override just description
SECRET_A = { description = "New description A", required = true }
# Override just required flag
SECRET_B = { description = "Original description B", required = false, default = "original-b" }
# Override just default value
SECRET_C = { description = "Original description C", required = false, default = "new-c" }
# Override multiple properties
SECRET_D = { description = "New description D", required = true }
# Add new secret
SECRET_E = { description = "New secret E", required = true }
"#;
        fs::write(base_path.join("override/secretspec.toml"), override_config).unwrap();

        // Parse the override config
        let config =
            secretspec_types::parse_spec(&base_path.join("override/secretspec.toml")).unwrap();
        let default_profile = config.profiles.get("default").unwrap();

        // Verify SECRET_A: only description changed
        let secret_a = default_profile.secrets.get("SECRET_A").unwrap();
        assert_eq!(secret_a.description, "New description A");
        assert!(secret_a.required);
        assert_eq!(secret_a.default, None);

        // Verify SECRET_B: only required flag changed
        let secret_b = default_profile.secrets.get("SECRET_B").unwrap();
        assert_eq!(secret_b.description, "Original description B");
        assert!(!secret_b.required); // Changed from true to false
        assert_eq!(secret_b.default, Some("original-b".to_string()));

        // Verify SECRET_C: only default value added
        let secret_c = default_profile.secrets.get("SECRET_C").unwrap();
        assert_eq!(secret_c.description, "Original description C");
        assert!(!secret_c.required);
        assert_eq!(secret_c.default, Some("new-c".to_string())); // Added default

        // Verify SECRET_D: multiple properties changed
        let secret_d = default_profile.secrets.get("SECRET_D").unwrap();
        assert_eq!(secret_d.description, "New description D");
        assert!(secret_d.required); // Changed from false to true
        assert_eq!(secret_d.default, None); // Default removed when required=true

        // Verify SECRET_E: new secret added
        let secret_e = default_profile.secrets.get("SECRET_E").unwrap();
        assert_eq!(secret_e.description, "New secret E");
        assert!(secret_e.required);

        // Verify total count
        assert_eq!(default_profile.secrets.len(), 5);
    }

    #[test]
    fn test_extends_with_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create base config with non-existent extend path
        let base_config = r#"
[project]
name = "test_project"
revision = "1.0"
extends = ["../nonexistent"]

[profiles.default]
API_KEY = { description = "API key for external service", required = true }
"#;
        fs::write(base_path.join("secretspec.toml"), base_config).unwrap();

        // Parse should fail with missing file error
        let result = secretspec_types::parse_spec(&base_path.join("secretspec.toml"));
        assert!(result.is_err());
        match result {
            Err(secretspec_types::ParseError::Io(e)) => {
                assert_eq!(e.kind(), io::ErrorKind::NotFound);
            }
            _ => panic!("Expected IO error for missing file"),
        }
    }

    #[test]
    fn test_extends_with_invalid_inputs() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Test 1: Extend to a file instead of directory
        let some_file = base_path.join("notadir.txt");
        fs::write(&some_file, "not a directory").unwrap();

        let config_extend_file = r#"
[project]
name = "test"
revision = "1.0"
extends = ["./notadir.txt"]

[profiles.default]
SECRET_A = { description = "Secret A", required = true }
"#;
        fs::write(base_path.join("secretspec.toml"), config_extend_file).unwrap();

        let result = secretspec_types::parse_spec(&base_path.join("secretspec.toml"));
        assert!(result.is_err());
        match result {
            Err(secretspec_types::ParseError::Io(e)) => {
                assert_eq!(e.kind(), io::ErrorKind::NotFound);
            }
            _ => panic!("Expected IO NotFound error for extending to file"),
        }

        // Test 2: Extend with empty string
        let config_empty_string = r#"
[project]
name = "test2"
revision = "1.0"
extends = [""]

[profiles.default]
SECRET_B = { description = "Secret B", required = true }
"#;
        fs::write(base_path.join("secretspec2.toml"), config_empty_string).unwrap();

        let result2 = secretspec_types::parse_spec(&base_path.join("secretspec2.toml"));
        assert!(result2.is_err());

        // Test 3: Extend to non-existent directory
        let config_no_dir = r#"
[project]
name = "test3"
revision = "1.0"
extends = ["./does_not_exist"]

[profiles.default]
SECRET_C = { description = "Secret C", required = true }
"#;
        fs::write(base_path.join("secretspec3.toml"), config_no_dir).unwrap();

        let result3 = secretspec_types::parse_spec(&base_path.join("secretspec3.toml"));
        assert!(result3.is_err());
        match result3 {
            Err(secretspec_types::ParseError::Io(e)) => {
                assert_eq!(e.kind(), io::ErrorKind::NotFound);
                // Verify error message mentions the missing file
                assert!(e.to_string().contains("Extended config file not found"));
            }
            _ => panic!("Expected IO NotFound error for non-existent directory"),
        }
    }

    #[test]
    fn test_extends_with_different_revisions() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create directory
        fs::create_dir_all(base_path.join("old")).unwrap();

        // Create config with unsupported revision
        let old_config = r#"
[project]
name = "old"
revision = "0.9"

[profiles.default]
OLD_SECRET = { description = "Old secret", required = true }
"#;
        fs::write(base_path.join("old/secretspec.toml"), old_config).unwrap();

        // Create config that tries to extend the old revision
        let new_config = r#"
[project]
name = "new"
revision = "1.0"
extends = ["./old"]

[profiles.default]
NEW_SECRET = { description = "New secret", required = true }
"#;
        fs::write(base_path.join("secretspec.toml"), new_config).unwrap();

        // This should fail with unsupported revision error
        let result = secretspec_types::parse_spec(&base_path.join("secretspec.toml"));
        assert!(result.is_err());
        match result {
            Err(secretspec_types::ParseError::UnsupportedRevision(rev)) => {
                assert_eq!(rev, "0.9");
            }
            _ => panic!("Expected UnsupportedRevision error"),
        }
    }

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
                profile: None,
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
                profile: None,
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
                profile: None,
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
