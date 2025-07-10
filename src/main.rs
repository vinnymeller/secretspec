use clap::{Parser, Subcommand};
use color_eyre::eyre::{Result, WrapErr};
use directories::ProjectDirs;
use secretspec::{DefaultConfig, GlobalConfig, SecretSpec};
use std::fs;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "secretspec")]
#[command(about = "Secure environment variable manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new secretspec.toml from existing .env file
    Init {
        /// Path to .env file to import from
        #[arg(short, long, default_value = ".env")]
        from: PathBuf,
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
    /// Import secrets from one provider to another
    Import {
        /// Provider backend to import from
        from_provider: String,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Initialize user configuration
    Init,
    /// Show current configuration
    Show,
}

fn get_config_path() -> secretspec::Result<PathBuf> {
    let dirs = ProjectDirs::from("", "", "secretspec").ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "Could not find config directory")
    })?;
    Ok(dirs.config_dir().join("config.toml"))
}

fn load_global_config() -> secretspec::Result<Option<GlobalConfig>> {
    let config_path = get_config_path()?;
    if !config_path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&config_path)?;
    Ok(Some(toml::from_str(&content)?))
}

fn save_global_config(config: &GlobalConfig) -> secretspec::Result<()> {
    let config_path = get_config_path()?;
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(config)?;
    fs::write(&config_path, content)?;
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
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

            let project_config = secretspec::project_config_from_path(&from)?;
            let mut content = secretspec::generate_toml_with_comments(&project_config)?;

            // Append comprehensive example
            content.push_str(secretspec::get_example_toml());

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

            if from.exists() {
                println!(
                    "\n! Remove {} after migrating secrets with:",
                    from.display()
                );
                println!("  secretspec set <SECRET_NAME>");
            }

            println!("\nNext steps:");
            println!("  1. secretspec config init    # Set up user configuration");
            println!("  2. secretspec check          # Verify all secrets and set them");
            println!("  3. secretspec run -- your-command  # Run with secrets");

            Ok(())
        }
        Commands::Config { action } => match action {
            ConfigAction::Init => {
                use inquire::Select;
                use secretspec::provider::ProviderRegistry;

                // Get provider choices from the centralized registry
                let provider_choices: Vec<String> = ProviderRegistry::providers()
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
                    defaults: DefaultConfig {
                        provider: provider.to_string(),
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
            ConfigAction::Show => {
                match load_global_config()? {
                    Some(config) => {
                        println!("Configuration file: {}\n", get_config_path()?.display());
                        println!("Provider: {}", config.defaults.provider);
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
        Commands::Set {
            name,
            value,
            provider,
            profile,
        } => {
            let app = SecretSpec::load().wrap_err("Failed to load secretspec configuration")?;
            app.set(&name, value, provider, profile)
                .wrap_err("Failed to set secret")?;
            Ok(())
        }
        Commands::Get {
            name,
            provider,
            profile,
        } => {
            let app = SecretSpec::load().wrap_err("Failed to load secretspec configuration")?;
            app.get(&name, provider, profile)
                .wrap_err("Failed to get secret")?;
            Ok(())
        }
        Commands::Run {
            command,
            provider,
            profile,
        } => {
            let app = SecretSpec::load().wrap_err("Failed to load secretspec configuration")?;
            app.run(command, provider, profile)
                .wrap_err("Failed to run command")?;
            Ok(())
        }
        Commands::Check { provider, profile } => {
            let app = SecretSpec::load().wrap_err("Failed to load secretspec configuration")?;
            app.check(provider, profile)
                .wrap_err("Failed to check secrets")?;
            Ok(())
        }
        Commands::Import { from_provider } => {
            let app = SecretSpec::load().wrap_err("Failed to load secretspec configuration")?;
            app.import(&from_provider)
                .wrap_err("Failed to import secrets")?;
            Ok(())
        }
    }
}
