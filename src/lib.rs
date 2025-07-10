use colored::Colorize;
use directories::ProjectDirs;
use std::collections::{HashMap, HashSet};
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
    r#"# API_KEY = { description = "API key for external service", required = true }
# DATABASE_URL = { description = "Database connection string", required = true }

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

pub fn generate_toml_with_comments(config: &ProjectConfig) -> Result<String> {
    let mut output = String::new();

    // Project section
    output.push_str("[project]\n");
    output.push_str(&format!("name = \"{}\"\n", config.project.name));
    output.push_str(&format!("revision = \"{}\"\n", config.project.revision));

    // Add extends comment and field if needed
    output.push_str("# Extend configurations from subdirectories\n");
    output.push_str("# extends = [ \"subdir1\", \"subdir2\" ]\n");

    // Profile sections
    for (profile_name, profile_config) in &config.profiles {
        output.push_str(&format!("\n[profiles.{}]\n", profile_name));

        for (secret_name, secret_config) in &profile_config.secrets {
            output.push_str(&format!(
                "{} = {{ description = \"{}\", required = {}",
                secret_name, secret_config.description, secret_config.required
            ));

            if let Some(default) = &secret_config.default {
                output.push_str(&format!(", default = \"{}\"", default));
            }

            output.push_str(" }\n");
        }
    }

    Ok(output)
}

pub struct SecretSpec {
    config: ProjectConfig,
    global_config: Option<GlobalConfig>,
}

pub struct SecretSpecBuilder {
    project_config_path: Option<PathBuf>,
    global_config_path: Option<PathBuf>,
    project_config: Option<ProjectConfig>,
    global_config: Option<GlobalConfig>,
    default_provider: Option<String>,
    default_profile: Option<String>,
}

impl Default for SecretSpecBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretSpecBuilder {
    pub fn new() -> Self {
        Self {
            project_config_path: None,
            global_config_path: None,
            project_config: None,
            global_config: None,
            default_provider: None,
            default_profile: None,
        }
    }

    pub fn project_config_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.project_config_path = Some(path.into());
        self
    }

    pub fn global_config_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.global_config_path = Some(path.into());
        self
    }

    pub fn project_config(mut self, config: ProjectConfig) -> Self {
        self.project_config = Some(config);
        self
    }

    pub fn global_config(mut self, config: GlobalConfig) -> Self {
        self.global_config = Some(config);
        self
    }

    pub fn default_provider(mut self, provider: impl Into<String>) -> Self {
        self.default_provider = Some(provider.into());
        self
    }

    pub fn default_profile(mut self, profile: impl Into<String>) -> Self {
        self.default_profile = Some(profile.into());
        self
    }

    pub fn load(mut self) -> Result<SecretSpec> {
        // Load project config
        let project_config = if let Some(config) = self.project_config {
            config
        } else if let Some(path) = self.project_config_path {
            load_project_config_from_path(&path)?
        } else {
            load_project_config()?
        };

        // Load global config
        let global_config = if let Some(config) = self.global_config {
            Some(config)
        } else if let Some(path) = self.global_config_path {
            load_global_config_from_path(&path)?
        } else {
            let mut gc = load_global_config()?;
            // Apply default overrides if provided
            if let Some(gc) = gc.as_mut() {
                if let Some(provider) = self.default_provider.take() {
                    gc.defaults.provider = provider;
                }
                if let Some(profile) = self.default_profile.take() {
                    gc.defaults.profile = Some(profile);
                }
            }
            gc
        };

        Ok(SecretSpec::new(project_config, global_config))
    }
}

impl SecretSpec {
    pub fn builder() -> SecretSpecBuilder {
        SecretSpecBuilder::new()
    }

    pub fn new(config: ProjectConfig, global_config: Option<GlobalConfig>) -> Self {
        Self {
            config,
            global_config,
        }
    }

    pub fn load() -> Result<Self> {
        SecretSpecBuilder::new().load()
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

        // Check if the secret exists in the requested profile
        let profile_secret = self
            .config
            .profiles
            .get(profile_name)
            .and_then(|p| p.secrets.get(name));

        // Check if the secret exists in the default profile (if we're not already looking at default)
        let default_secret = if profile_name != "default" {
            self.config
                .profiles
                .get("default")
                .and_then(|p| p.secrets.get(name))
        } else {
            None
        };

        // Use the profile secret if it exists, otherwise fall back to default
        let secret_config = profile_secret.or(default_secret)?;

        Some((secret_config.required, secret_config.default.clone()))
    }

    fn get_provider(&self, provider_arg: Option<String>) -> Result<Box<dyn ProviderTrait>> {
        let provider_spec = if let Some(spec) = provider_arg {
            spec
        } else if let Some(global_config) = &self.global_config {
            global_config
                .projects
                .get(&self.config.project.name)
                .map(|p| p.provider.clone())
                .unwrap_or(global_config.defaults.provider.clone())
        } else {
            return Err(SecretSpecError::NoProviderConfigured);
        };

        let backend = ProviderRegistry::create_from_string(&provider_spec)?;

        Ok(backend)
    }

    pub fn write(&self, from: &Path, output_dir: Option<&Path>) -> Result<()> {
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
        let output_path = match output_dir {
            Some(dir) => dir.join("secretspec.toml"),
            None => PathBuf::from("secretspec.toml"),
        };
        fs::write(&output_path, content)?;

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
            println!("  secretspec check");
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

        let backend = self.get_provider(provider_arg)?;
        let profile_display = self.resolve_profile(profile.as_deref());

        // Check if the provider supports setting values
        if !backend.allows_set() {
            return Err(SecretSpecError::ProviderOperationFailed(format!(
                "Provider '{}' is read-only and does not support setting values",
                backend.name()
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
            backend.name(),
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
        let backend = self.get_provider(provider_arg)?;
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
        let backend = self.get_provider(provider_arg.clone())?;
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
                                backend.name(),
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
        let provider = self.get_provider(provider_arg.clone())?;
        let profile_display = self.resolve_profile(profile.as_deref());

        println!(
            "Checking secrets in {} using {} (profile: {})...\n",
            self.config.project.name.bold(),
            provider.name().blue(),
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

    pub fn import(&self, from_provider: &str) -> Result<()> {
        // Get the "to" provider from global config (default)
        let to_provider = self.get_provider(None)?;

        // Get the profile from global config
        let profile = self
            .global_config
            .as_ref()
            .and_then(|gc| gc.defaults.profile.as_deref());
        let profile_display = self.resolve_profile(profile);

        // Create the "from" provider
        let from_provider_backend = ProviderRegistry::create_from_string(from_provider)?;

        println!(
            "Importing secrets from {} to {} (profile: {})...\n",
            from_provider.blue(),
            to_provider.name().blue(),
            profile_display.cyan()
        );

        // Get the profile configuration
        let profile_config = self.config.profiles.get(profile_display).ok_or_else(|| {
            SecretSpecError::SecretNotFound(format!("Profile '{}' not found", profile_display))
        })?;

        let mut imported = 0;
        let mut already_exists = 0;
        let mut not_found = 0;

        // Process each secret in the profile
        for (name, config) in &profile_config.secrets {
            // First check if the secret exists in the "from" provider
            match from_provider_backend.get(&self.config.project.name, name, profile)? {
                Some(value) => {
                    // Secret exists in "from" provider, check if it exists in "to" provider
                    match to_provider.get(&self.config.project.name, name, profile)? {
                        Some(_) => {
                            println!(
                                "{} {} - {} {}",
                                "○".yellow(),
                                name,
                                config.description,
                                "(already exists in target)".yellow()
                            );
                            already_exists += 1;
                        }
                        None => {
                            // Secret doesn't exist in "to" provider, import it
                            to_provider.set(&self.config.project.name, name, &value, profile)?;
                            println!("{} {} - {}", "✓".green(), name, config.description);
                            imported += 1;
                        }
                    }
                }
                None => {
                    // Secret doesn't exist in "from" provider
                    // Check if it exists in the "to" provider
                    match to_provider.get(&self.config.project.name, name, profile)? {
                        Some(_) => {
                            println!(
                                "{} {} - {} {}",
                                "○".blue(),
                                name,
                                config.description,
                                "(already in target, not in source)".blue()
                            );
                            already_exists += 1;
                        }
                        None => {
                            println!(
                                "{} {} - {} {}",
                                "✗".red(),
                                name,
                                config.description,
                                "(not found in source)".red()
                            );
                            not_found += 1;
                        }
                    }
                }
            }
        }

        println!(
            "\nSummary: {} imported, {} already exists, {} not found in source",
            imported.to_string().green(),
            already_exists.to_string().yellow(),
            not_found.to_string().red()
        );

        if imported > 0 {
            println!(
                "\n{} Successfully imported {} secrets from {} to {}",
                "✓".green(),
                imported,
                from_provider,
                to_provider.name()
            );
        }

        Ok(())
    }

    pub fn validate(
        &self,
        provider_arg: Option<String>,
        profile: Option<String>,
    ) -> Result<ValidationResult> {
        let backend = self.get_provider(provider_arg)?;
        let mut secrets = HashMap::new();
        let mut missing_required = Vec::new();
        let mut missing_optional = Vec::new();
        let mut with_defaults = Vec::new();

        let profile_name = self.resolve_profile(profile.as_deref());
        let profile_config = self.config.profiles.get(profile_name).ok_or_else(|| {
            SecretSpecError::SecretNotFound(format!("Profile '{}' not found", profile_name))
        })?;

        // Collect all secrets to check - from current profile and default profile
        let mut all_secrets = HashSet::new();

        // Add secrets from the current profile
        for name in profile_config.secrets.keys() {
            all_secrets.insert(name.clone());
        }

        // If not the default profile, also add secrets from default profile
        if profile_name != "default" {
            if let Some(default_profile) = self.config.profiles.get("default") {
                for name in default_profile.secrets.keys() {
                    all_secrets.insert(name.clone());
                }
            }
        }

        // Now check all secrets
        for name in all_secrets {
            let (required, default) = self
                .resolve_secret_config(&name, profile.as_deref())
                .expect("Secret should exist in config since we're iterating over it");

            match backend.get(&self.config.project.name, &name, profile.as_deref())? {
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

        let provider = secretspec_types::Provider::from_str(backend.name())
            .ok_or_else(|| SecretSpecError::ProviderNotFound(backend.name().to_string()))?;

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

        // Ensure all secrets are available (will error out if missing)
        let validation_result = self.ensure_secrets(provider_arg, profile, false)?;

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

fn load_project_config_from_path(path: &Path) -> Result<ProjectConfig> {
    parse_spec(path)
}

fn load_global_config() -> Result<Option<GlobalConfig>> {
    let config_path = get_config_path()?;
    if !config_path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&config_path)?;
    Ok(Some(toml::from_str(&content)?))
}

fn load_global_config_from_path(path: &Path) -> Result<Option<GlobalConfig>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    Ok(Some(toml::from_str(&content)?))
}

#[cfg(test)]
mod tests;
