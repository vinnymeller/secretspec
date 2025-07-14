use clap::{Parser, Subcommand};
use color_eyre::eyre::{Result, WrapErr};
use directories::ProjectDirs;
use secretspec::{Config, GlobalConfig, GlobalDefaults, Secrets};
use std::collections::HashMap;
use std::fs;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

/// Main CLI structure for the secretspec application.
///
/// This is the entry point for the command-line interface, parsing user commands
/// and delegating to the appropriate subcommands for secrets management.
#[derive(Parser)]
#[command(name = "secretspec")]
#[command(about = "Secure environment variable manager", long_about = None)]
struct Cli {
    /// The subcommand to execute
    #[command(subcommand)]
    command: Commands,
}

/// Available commands for the secretspec CLI.
///
/// This enum defines all the subcommands that can be executed, including
/// initialization, secret management, configuration, and import operations.
#[derive(Subcommand)]
enum Commands {
    /// Initialize a new secretspec.toml from a provider
    Init {
        /// Provider URL to import from (e.g., dotenv://.env, dotenv://.env.production)
        /// Currently only dotenv provider is supported.
        #[arg(short, long, default_value = "dotenv://.env")]
        from: String,
    },
    /// Set a secret value
    Set {
        /// Name of the secret
        name: String,
        /// Value of the secret (will prompt if not provided)
        value: Option<String>,
        /// Provider backend to use
        #[arg(short, long, env = "SECRETSPEC_PROVIDER")]
        provider: Option<String>,
        /// Profile to use
        #[arg(short = 'P', long, env = "SECRETSPEC_PROFILE")]
        profile: Option<String>,
    },
    /// Get a secret value
    Get {
        /// Name of the secret
        name: String,
        /// Provider backend to use
        #[arg(short, long, env = "SECRETSPEC_PROVIDER")]
        provider: Option<String>,
        /// Profile to use
        #[arg(short = 'P', long, env = "SECRETSPEC_PROFILE")]
        profile: Option<String>,
    },
    /// Run a command with secrets injected
    Run {
        /// Provider backend to use
        #[arg(short, long, env = "SECRETSPEC_PROVIDER")]
        provider: Option<String>,
        /// Profile to use
        #[arg(short = 'P', long, env = "SECRETSPEC_PROFILE")]
        profile: Option<String>,
        /// Command and arguments to run
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
    /// Check if all required secrets are available
    Check {
        /// Provider backend to use
        #[arg(short, long, env = "SECRETSPEC_PROVIDER")]
        provider: Option<String>,
        /// Profile to use
        #[arg(short = 'P', long, env = "SECRETSPEC_PROFILE")]
        profile: Option<String>,
    },
    /// Configure user settings
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Import secrets from one provider to the default provider
    Import {
        /// Provider backend to import from (secrets will be imported to the default provider)
        from_provider: String,
    },
}

/// Configuration-related subcommands.
///
/// These actions handle the user's global configuration settings,
/// including initialization and viewing current settings.
#[derive(Subcommand)]
enum ConfigAction {
    /// Initialize user configuration
    Init,
    /// Show current configuration
    Show,
}

/// Get the path to the user's configuration file.
///
/// Returns the platform-specific configuration directory path for secretspec,
/// typically ~/.config/secretspec/config.toml on Unix systems.
///
/// # Returns
///
/// * `Ok(PathBuf)` - The path to the configuration file
/// * `Err` - If the configuration directory cannot be determined
///
/// # Example
///
/// ```no_run
/// let config_path = get_config_path()?;
/// println!("Config file: {}", config_path.display());
/// ```
fn get_config_path() -> secretspec::Result<PathBuf> {
    let dirs = ProjectDirs::from("", "", "secretspec").ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "Could not find config directory")
    })?;
    Ok(dirs.config_dir().join("config.toml"))
}

/// Load the global configuration from disk.
///
/// Reads and parses the user's global configuration file if it exists.
/// Returns `None` if the configuration file doesn't exist.
///
/// # Returns
///
/// * `Ok(Some(GlobalConfig))` - The parsed configuration
/// * `Ok(None)` - If the configuration file doesn't exist
/// * `Err` - If reading or parsing the configuration fails
fn load_global_config() -> secretspec::Result<Option<GlobalConfig>> {
    let config_path = get_config_path()?;
    if !config_path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&config_path)?;
    Ok(Some(toml::from_str(&content)?))
}

/// Save the global configuration to disk.
///
/// Writes the provided configuration to the user's configuration file,
/// creating parent directories if necessary.
///
/// # Arguments
///
/// * `config` - The configuration to save
///
/// # Returns
///
/// * `Ok(())` - If the configuration was saved successfully
/// * `Err` - If creating directories or writing the file fails
fn save_global_config(config: &GlobalConfig) -> secretspec::Result<()> {
    let config_path = get_config_path()?;
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(config)?;
    fs::write(&config_path, content)?;
    Ok(())
}

/// Returns an example TOML configuration string
///
/// This function provides a template for creating new `secretspec.toml` files,
/// showing the recommended structure and commenting conventions.
///
/// # Returns
///
/// A static string containing an example TOML configuration
fn get_example_toml() -> &'static str {
    r#"# DATABASE_URL = { description = "Database connection string", required = true }

[profiles.development]
# Development profile inherits all secrets from default profile
# Only define secrets here that need different values or settings than default
# DATABASE_URL = { default = "sqlite:///dev.db" }
#
# New secrets
# REDIS_URL = { description = "Redis connection URL for caching", required = false, default = "redis://localhost:6379" }
"#
}

/// Generates a TOML string from a ProjectConfig with helpful comments
///
/// This function serializes a `ProjectConfig` to TOML format while adding
/// instructional comments to help users understand the configuration options.
///
/// # Arguments
///
/// * `config` - The project configuration to serialize
///
/// # Returns
///
/// A TOML string with the configuration and helpful comments
///
/// # Errors
///
/// Returns an error if the configuration cannot be serialized
fn generate_toml_with_comments(config: &Config) -> secretspec::Result<String> {
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

/// Main entry point for the secretspec CLI application.
///
/// Parses command-line arguments and executes the appropriate command.
/// All commands are delegated to the SecretSpec library for processing.
///
/// # Returns
///
/// * `Ok(())` - If the command executed successfully
/// * `Err` - If any error occurred during execution
fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        // Initialize a new secretspec.toml configuration file
        Commands::Init { from } => {
            // Check if secretspec.toml already exists
            if PathBuf::from("secretspec.toml").exists() {
                use inquire::Confirm;
                let overwrite = Confirm::new("secretspec.toml already exists. Overwrite?")
                    .with_default(false)
                    .prompt()?;

                if !overwrite {
                    println!("Cancelled.");
                    return Ok(());
                }
            }

            // Parse the provider URL
            let uri = from
                .parse::<url::Url>()
                .map_err(|e| color_eyre::eyre::eyre!("Invalid provider URL '{}': {}", from, e))?;

            // Extract scheme from URI to validate provider
            let scheme = uri.scheme();

            // Currently only support dotenv provider
            if scheme != "dotenv" {
                return Err(color_eyre::eyre::eyre!(
                    "Only 'dotenv://' provider URLs are currently supported for init --from. Got: {}",
                    from
                ));
            }

            // Create dotenv provider and reflect secrets
            let dotenv_config = (&uri).try_into()?;
            let dotenv_provider = secretspec::provider::dotenv::DotEnvProvider::new(dotenv_config);
            let secrets = dotenv_provider.reflect()?;

            // Create a new project config
            let mut profiles = HashMap::new();
            profiles.insert("default".to_string(), secretspec::Profile { secrets });

            let project_config = secretspec::Config {
                project: secretspec::Project {
                    name: std::env::current_dir()?
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    revision: "1.0".to_string(),
                    extends: None,
                },
                profiles,
            };
            let mut content = generate_toml_with_comments(&project_config)?;

            // Append comprehensive example
            content.push_str(get_example_toml());

            fs::write("secretspec.toml", content)?;

            // Set file permissions to 600 (owner read/write only) on Unix systems
            #[cfg(unix)]
            {
                let metadata = fs::metadata("secretspec.toml")?;
                let mut permissions = metadata.permissions();
                permissions.set_mode(0o600);
                fs::set_permissions("secretspec.toml", permissions)?;
            }

            let secret_count = project_config
                .profiles
                .values()
                .map(|p| p.secrets.len())
                .sum::<usize>();
            println!("✓ Created secretspec.toml with {} secrets", secret_count);

            println!("\nNext steps:");
            println!("  1. secretspec config init    # Set up user configuration");
            println!("  2. secretspec check          # Verify all secrets and set them");
            println!("  3. secretspec run -- your-command  # Run with secrets");

            Ok(())
        }
        // Handle configuration management commands
        Commands::Config { action } => match action {
            // Initialize user configuration with interactive prompts
            ConfigAction::Init => {
                use inquire::Select;
                use secretspec::provider;

                // Get provider choices from the centralized registry
                let provider_choices: Vec<String> = provider::providers()
                    .into_iter()
                    .map(|info| info.display_with_examples())
                    .collect();

                let selected_choice =
                    Select::new("Select your preferred provider backend:", provider_choices)
                        .prompt()?;

                // Extract provider name from the selected choice
                let provider = selected_choice.split(':').next().unwrap_or("keyring");

                let profiles = vec!["development", "default", "none"];
                let profile_choice = Select::new("Select your default profile:", profiles)
                    .with_help_message(
                        "'development' is recommended for local development environments",
                    )
                    .prompt()?;

                let profile = if profile_choice == "none" {
                    None
                } else {
                    Some(profile_choice.to_string())
                };

                let config = GlobalConfig {
                    defaults: GlobalDefaults {
                        provider: Some(provider.to_string()),
                        profile,
                    },
                };

                save_global_config(&config)?;
                println!(
                    "\n✓ Configuration saved to {}",
                    get_config_path()?.display()
                );
                Ok(())
            }
            // Display current user configuration
            ConfigAction::Show => {
                match load_global_config()? {
                    Some(config) => {
                        println!("Configuration file: {}\n", get_config_path()?.display());
                        match config.defaults.provider {
                            Some(provider) => println!("Provider: {}", provider),
                            None => println!("Provider: (none)"),
                        }
                        match config.defaults.profile {
                            Some(profile) => println!("Profile:  {}", profile),
                            None => println!("Profile:  (none)"),
                        }
                    }
                    None => {
                        println!(
                            "No configuration found. Run 'secretspec config init' to create one."
                        );
                    }
                }
                Ok(())
            }
        },
        // Set a secret value in the specified provider
        Commands::Set {
            name,
            value,
            provider,
            profile,
        } => {
            let app = Secrets::load().wrap_err("Failed to load secretspec configuration")?;
            app.set(&name, value, provider, profile)
                .wrap_err("Failed to set secret")?;
            Ok(())
        }
        // Retrieve and display a secret value
        Commands::Get {
            name,
            provider,
            profile,
        } => {
            let app = Secrets::load().wrap_err("Failed to load secretspec configuration")?;
            app.get(&name, provider, profile)
                .wrap_err("Failed to get secret")?;
            Ok(())
        }
        // Execute a command with secrets injected as environment variables
        Commands::Run {
            command,
            provider,
            profile,
        } => {
            let app = Secrets::load().wrap_err("Failed to load secretspec configuration")?;
            app.run(command, provider, profile)
                .wrap_err("Failed to run command")?;
            Ok(())
        }
        // Verify all required secrets are available
        Commands::Check { provider, profile } => {
            let app = Secrets::load().wrap_err("Failed to load secretspec configuration")?;
            app.check(provider, profile)
                .wrap_err("Failed to check secrets")?;
            Ok(())
        }
        // Import secrets from one provider to another
        Commands::Import { from_provider } => {
            let app = Secrets::load().wrap_err("Failed to load secretspec configuration")?;
            app.import(&from_provider)
                .wrap_err("Failed to import secrets")?;
            Ok(())
        }
    }
}
