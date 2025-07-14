//! Core secrets management functionality

use crate::error::{Result, SecretSpecError};
use crate::provider::Provider as ProviderTrait;
use crate::validation::ValidatedSecrets;
use colored::Colorize;
use secretspec_core::{Config, GlobalConfig};
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

/// The main entry point for the secretspec library
///
/// `Secrets` manages the loading, validation, and retrieval of secrets
/// based on the project and global configuration files.
///
/// # Example
///
/// ```no_run
/// use secretspec::Secrets;
///
/// // Load configuration and validate secrets
/// let spec = Secrets::load().unwrap();
/// spec.check(None, None).unwrap();
/// ```
pub struct Secrets {
    /// The project-specific configuration
    config: Config,
    /// Optional global user configuration
    global_config: Option<GlobalConfig>,
}

impl Secrets {
    /// Creates a new `Secrets` instance with the given configurations
    ///
    /// # Arguments
    ///
    /// * `config` - The project configuration
    /// * `global_config` - Optional global user configuration
    ///
    /// # Returns
    ///
    /// A new `Secrets` instance
    pub fn new(config: Config, global_config: Option<GlobalConfig>) -> Self {
        Self {
            config,
            global_config,
        }
    }

    /// Get a reference to the project configuration (for testing)
    #[cfg(test)]
    pub(crate) fn config(&self) -> &Config {
        &self.config
    }

    /// Get a reference to the global configuration (for testing)
    #[cfg(test)]
    pub(crate) fn global_config(&self) -> &Option<GlobalConfig> {
        &self.global_config
    }

    /// Loads a `Secrets` using default configuration paths
    ///
    /// This method looks for:
    /// - `secretspec.toml` in the current directory for project configuration
    /// - User configuration in the system config directory
    ///
    /// # Returns
    ///
    /// A loaded `Secrets` instance
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No `secretspec.toml` file is found
    /// - Configuration files are invalid
    /// - The project revision is unsupported
    pub fn load() -> Result<Self> {
        let project_config = Config::try_from(Path::new("secretspec.toml"))?;
        let global_config = GlobalConfig::load()?;
        Ok(Secrets::new(project_config, global_config))
    }

    /// Resolves the profile to use based on the provided value and configuration
    ///
    /// Profile resolution order:
    /// 1. Provided profile argument
    /// 2. Global configuration default profile
    /// 3. "default" profile
    ///
    /// # Arguments
    ///
    /// * `profile` - Optional profile name to use
    ///
    /// # Returns
    ///
    /// The resolved profile name
    pub(crate) fn resolve_profile<'a>(&'a self, profile: Option<&'a str>) -> &'a str {
        profile.unwrap_or_else(|| {
            self.global_config
                .as_ref()
                .and_then(|gc| gc.defaults.profile.as_deref())
                .unwrap_or("default")
        })
    }

    /// Resolves the configuration for a specific secret
    ///
    /// This method looks for the secret in the specified profile, falling back
    /// to the default profile if not found.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the secret
    /// * `profile` - Optional profile to search in
    ///
    /// # Returns
    ///
    /// A tuple of (is_required, default_value) if the secret is found, `None` otherwise
    pub(crate) fn resolve_secret_config(
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

    /// Gets the provider instance to use for secret operations
    ///
    /// Provider resolution order:
    /// 1. Provided provider argument
    /// 2. Global configuration default provider
    /// 3. Error if no provider is configured
    ///
    /// # Arguments
    ///
    /// * `provider_arg` - Optional provider specification (name or URI)
    ///
    /// # Returns
    ///
    /// A boxed provider instance
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No provider is configured
    /// - The specified provider is not found
    pub(crate) fn get_provider(
        &self,
        provider_arg: Option<String>,
    ) -> Result<Box<dyn ProviderTrait>> {
        let provider_spec = if let Some(spec) = provider_arg {
            spec
        } else if let Some(global_config) = &self.global_config {
            if let Some(provider) = &global_config.defaults.provider {
                provider.clone()
            } else {
                return Err(SecretSpecError::NoProviderConfigured);
            }
        } else {
            return Err(SecretSpecError::NoProviderConfigured);
        };

        let provider = Box::<dyn ProviderTrait>::try_from(provider_spec)?;

        Ok(provider)
    }

    /// Sets a secret value in the provider
    ///
    /// If no value is provided, the user will be prompted to enter it securely.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the secret to set
    /// * `value` - Optional value to set (prompts if None)
    /// * `provider_arg` - Optional provider to use
    /// * `profile` - Optional profile to use
    ///
    /// # Returns
    ///
    /// `Ok(())` if the secret was successfully set
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The secret is not defined in the specification
    /// - The provider doesn't support setting values
    /// - The storage operation fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use secretspec::Secrets;
    ///
    /// let spec = Secrets::load().unwrap();
    /// spec.set("DATABASE_URL", Some("postgres://localhost".to_string()), None, None).unwrap();
    /// ```
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

        backend.set(&self.config.project.name, name, &value, profile_name)?;
        println!(
            "{} Secret '{}' saved to {} (profile: {})",
            "✓".green(),
            name,
            backend.name(),
            profile_display
        );

        Ok(())
    }

    /// Retrieves and prints a secret value
    ///
    /// This method retrieves a secret from the storage backend and prints it
    /// to stdout. If the secret is not found but has a default value, the
    /// default is printed.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the secret to retrieve
    /// * `provider_arg` - Optional provider to use
    /// * `profile` - Optional profile to use
    ///
    /// # Returns
    ///
    /// `Ok(())` if the secret was found and printed
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The secret is not defined in the specification
    /// - The secret is not found and has no default value
    pub fn get(
        &self,
        name: &str,
        provider_arg: Option<String>,
        profile: Option<String>,
    ) -> Result<()> {
        let backend = self.get_provider(provider_arg)?;
        let profile_name = self.resolve_profile(profile.as_deref());
        let (_, default) = self
            .resolve_secret_config(name, profile.as_deref())
            .ok_or_else(|| SecretSpecError::SecretNotFound(name.to_string()))?;

        match backend.get(&self.config.project.name, name, profile_name)? {
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

    /// Ensures all required secrets are present, optionally prompting for missing ones
    ///
    /// This method validates all secrets and, in interactive mode, prompts the
    /// user to provide values for any missing required secrets.
    ///
    /// # Arguments
    ///
    /// * `provider_arg` - Optional provider to use
    /// * `profile` - Optional profile to use
    /// * `interactive` - Whether to prompt for missing secrets
    ///
    /// # Returns
    ///
    /// A `ValidatedSecrets` with the final state of all secrets
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Required secrets are missing and interactive mode is disabled
    /// - Storage operations fail
    fn ensure_secrets(
        &self,
        provider_arg: Option<String>,
        profile: Option<String>,
        interactive: bool,
    ) -> Result<ValidatedSecrets> {
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
                                &profile_display,
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

    /// Checks the status of all secrets and prompts for missing required ones
    ///
    /// This method displays the status of all secrets defined in the specification,
    /// showing which are present, missing, or using defaults. It then prompts
    /// the user to provide values for any missing required secrets.
    ///
    /// # Arguments
    ///
    /// * `provider_arg` - Optional provider to use
    /// * `profile` - Optional profile to use
    ///
    /// # Returns
    ///
    /// `Ok(())` if all required secrets are present after prompting
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The provider cannot be initialized
    /// - Storage operations fail
    ///
    /// # Example
    ///
    /// ```no_run
    /// use secretspec::Secrets;
    ///
    /// let spec = Secrets::load().unwrap();
    /// spec.check(None, None).unwrap();
    /// ```
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

    /// Imports secrets from one provider to another
    ///
    /// This method copies all secrets defined in the specification from the
    /// source provider to the default provider configured in the global settings.
    ///
    /// # Arguments
    ///
    /// * `from_provider` - The provider specification to import from
    ///
    /// # Returns
    ///
    /// `Ok(())` if the import completes (even if some secrets were not found)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The source provider cannot be initialized
    /// - The target provider cannot be initialized
    /// - Storage operations fail
    ///
    /// # Example
    ///
    /// ```no_run
    /// use secretspec::Secrets;
    ///
    /// let spec = Secrets::load().unwrap();
    /// spec.import("dotenv://.env.production").unwrap();
    /// ```
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
        let from_provider_instance = Box::<dyn ProviderTrait>::try_from(from_provider.to_string())?;

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
            match from_provider_instance.get(&self.config.project.name, name, profile_display)? {
                Some(value) => {
                    // Secret exists in "from" provider, check if it exists in "to" provider
                    match to_provider.get(&self.config.project.name, name, profile_display)? {
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
                            to_provider.set(
                                &self.config.project.name,
                                name,
                                &value,
                                profile_display,
                            )?;
                            println!("{} {} - {}", "✓".green(), name, config.description);
                            imported += 1;
                        }
                    }
                }
                None => {
                    // Secret doesn't exist in "from" provider
                    // Check if it exists in the "to" provider
                    match to_provider.get(&self.config.project.name, name, profile_display)? {
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

    /// Validates all secrets in the specification
    ///
    /// This method checks all secrets defined in the current profile (and default
    /// profile if different) and returns detailed information about their status.
    ///
    /// # Arguments
    ///
    /// * `provider_arg` - Optional provider to use
    /// * `profile` - Optional profile to use
    ///
    /// # Returns
    ///
    /// A `ValidatedSecrets` containing the status of all secrets
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The provider cannot be initialized
    /// - The specified profile doesn't exist
    /// - Storage operations fail
    ///
    /// # Example
    ///
    /// ```no_run
    /// use secretspec::Secrets;
    ///
    /// let spec = Secrets::load().unwrap();
    /// let result = spec.validate(None, None).unwrap();
    /// if result.is_valid() {
    ///     println!("All required secrets are present!");
    /// }
    /// ```
    pub fn validate(
        &self,
        provider_arg: Option<String>,
        profile: Option<String>,
    ) -> Result<ValidatedSecrets> {
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

            match backend.get(&self.config.project.name, &name, profile_name)? {
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

        Ok(ValidatedSecrets {
            secrets,
            missing_required,
            missing_optional,
            with_defaults,
            provider: backend,
            profile: profile_name.to_string(),
        })
    }

    /// Runs a command with secrets injected as environment variables
    ///
    /// This method validates that all required secrets are present, then runs
    /// the specified command with all secrets injected as environment variables.
    ///
    /// # Arguments
    ///
    /// * `command` - The command and arguments to run
    /// * `provider_arg` - Optional provider to use
    /// * `profile` - Optional profile to use
    ///
    /// # Returns
    ///
    /// This method executes the command and exits with the command's exit code.
    /// It only returns an error if validation fails or the command cannot be started.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No command is specified
    /// - Required secrets are missing
    /// - The command cannot be executed
    ///
    /// # Example
    ///
    /// ```no_run
    /// use secretspec::Secrets;
    ///
    /// let spec = Secrets::load().unwrap();
    /// spec.run(vec!["npm".to_string(), "start".to_string()], None, None).unwrap();
    /// ```
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
