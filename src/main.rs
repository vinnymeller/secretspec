use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use secretspec::{DefaultConfig, GlobalConfig, ProjectConfig, Result, SecretSpec};
use std::collections::HashMap;
use std::fs;
use std::io;
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
        /// Storage backend to use
        #[arg(short, long)]
        storage: Option<String>,
        /// Environment to use
        #[arg(short, long)]
        env: Option<String>,
    },
    /// Get a secret value
    Get {
        /// Name of the secret
        name: String,
        /// Storage backend to use
        #[arg(short, long)]
        storage: Option<String>,
        /// Environment to use
        #[arg(short, long)]
        env: Option<String>,
    },
    /// Run a command with secrets injected
    Run {
        /// Storage backend to use
        #[arg(short, long)]
        storage: Option<String>,
        /// Environment to use
        #[arg(short, long)]
        env: Option<String>,
        /// Command and arguments to run
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
    /// Check if all required secrets are available
    Check {
        /// Storage backend to use
        #[arg(short, long)]
        storage: Option<String>,
        /// Environment to use
        #[arg(short, long)]
        env: Option<String>,
    },
    /// Configure user settings
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Initialize user configuration
    Init,
    /// Show current configuration
    Show,
}

fn get_config_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("", "", "secretspec").ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "Could not find config directory")
    })?;
    Ok(dirs.config_dir().join("config.toml"))
}

fn load_project_config() -> Result<ProjectConfig> {
    let content = fs::read_to_string("secretspec.toml")
        .map_err(|_| secretspec::SecretSpecError::NoManifest)?;
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

fn save_global_config(config: &GlobalConfig) -> Result<()> {
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
            let project_config = ProjectConfig::from_path(&from)?;
            let mut content = toml::to_string_pretty(&project_config)?;

            // Append comprehensive example
            content.push_str(ProjectConfig::get_example_toml());

            fs::write("secretspec.toml", content)?;

            println!(
                "✓ Created secretspec.toml with {} secrets",
                project_config.secrets.len()
            );

            if from.exists() {
                println!(
                    "\n! Remove {} after migrating secrets with:",
                    from.display()
                );
                println!("  secretspec set <SECRET_NAME>");
            }

            println!("\nNext steps:");
            println!("  1. secretspec config init    # Set up user configuration");
            println!("  2. secretspec set API_KEY    # Store your secrets");
            println!("  3. secretspec check          # Verify all secrets are set");
            println!("  4. secretspec run -- your-command  # Run with secrets");

            Ok(())
        }
        Commands::Config { action } => match action {
            ConfigAction::Init => {
                let config = GlobalConfig {
                    defaults: DefaultConfig {
                        storage: "keyring".to_string(),
                    },
                    projects: HashMap::new(),
                };

                save_global_config(&config)?;
                println!("✓ Created config at {}", get_config_path()?.display());
                Ok(())
            }
            ConfigAction::Show => {
                match load_global_config()? {
                    Some(config) => {
                        println!("Configuration file: {}\n", get_config_path()?.display());
                        println!("{}", toml::to_string_pretty(&config)?);
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
            storage,
            env,
        } => {
            let app = SecretSpec::load()?;
            app.set(&name, value, storage, env)
        }
        Commands::Get { name, storage, env } => {
            let app = SecretSpec::load()?;
            app.get(&name, storage, env)
        }
        Commands::Run {
            command,
            storage,
            env,
        } => {
            let app = SecretSpec::load()?;
            app.run(command, storage, env)
        }
        Commands::Check { storage, env } => {
            let app = SecretSpec::load()?;
            app.check(storage, env)
        }
    }
}
