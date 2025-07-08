[![Build Status](https://img.shields.io/github/check-runs/cachix/secretspec/main)](https://github.com/cachix/secretspec/actions)
[![Crates.io](https://img.shields.io/crates/v/secretspec)](https://crates.io/crates/secretspec)
[![docs.rs](https://docs.rs/secretspec/badge.svg)](https://docs.rs/secretspec)
[![Discord channel](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fdiscord.com%2Fapi%2Finvites%2FnaMgvexb6q%3Fwith_counts%3Dtrue&query=%24.approximate_member_count&logo=discord&logoColor=white&label=Discord%20users&color=green&style=flat)](https://discord.gg/naMgvexb6q)
![License: Apache 2.0](https://img.shields.io/github/license/cachix/secretspec)

# SecretSpec

Declarative secrets for development workflows, supporting a variety of storage backends.

See [announcement blog post for motivation](XXX).

## Introduction

Modern applications require secrets - API keys, database credentials, service tokens. Yet we lack a standard way to declare these requirements. Applications either hard-code retrieval mechanisms or fail at runtime with missing environment variables.

### The Problem: Conflating What, How, and Where

Current secret management approaches force applications to simultaneously answer three distinct questions:

- **WHAT** - Which secrets does the application need? (DATABASE_URL, API_KEY)
- **HOW** - What are the requirements? (required vs optional, defaults, validation, environment)
- **WHERE** - Where are these secrets stored? (environment variables, Vault, AWS Secrets Manager)

This coupling creates several problems:

1. **Lack of Portability**: Applications become tightly coupled to specific storage backends, making it difficult to switch providers or adapt to different environments
2. **Runtime Failures**: Missing secrets are only discovered when the application attempts to use them, leading to crashes in production
3. **Poor Developer Experience**: Each developer must understand the specific storage mechanism and manually configure their environment
4. **Inconsistent Practices**: Every application implements its own ad-hoc solution, leading to a fragmented ecosystem

### The Solution: Declarative Secret Requirements

SecretSpec introduces a declarative approach that separates the "what" and "how" from the "where":

- **WHAT** secrets are needed is declared in `secretspec.toml`
- **HOW** requirements vary by environment is managed through `profile`
- **WHERE** secrets are stored depends on where the application runs, configured via `provider`

Applications declare their secret requirements in a `secretspec.toml` file, while the runtime environment determines the storage backend through `provider` configuration and context via `profile` selection.

This separation enables:
- **Portable Applications**: The same application works across different secret storage backends without code changes
- **Early Validation**: Check that all required secrets are available before starting the application
- **Better Tooling**: Standardized format enables ecosystem-wide tooling for secret management
- **Type Safety**: Generate strongly-typed code from declarations for compile-time guarantees

SecretSpec is a declarative secrets specification for development workflows, supporting a variety of storage backends including system keyrings, .env files, environment variables, and password managers.

See [announcement blog post for motivation](XXX).

## Features

- **Declarative Configuration**: Define your secrets in `secretspec.toml` with descriptions and requirements
- **Multiple Provider Backends**: [Keyring](https://docs.rs/keyring/latest/keyring/) (system credential store), [.env](https://www.dotenv.org/), and environment variable support
- **Type-Safe Rust SDK**: Generate strongly-typed structs from your `secretspec.toml` for compile-time safety
- **Profile Support**: Override secret requirements and defaults per profile (development, production, etc.)
- **Configuration Inheritance**: Extend and override shared configurations using the `extends` feature
- **Discovery**: `secretspec init` to discover secrets from existing `.env` files

## Quick Start

1. **Initialize `secretspec.toml` (discovers secrets from .env)**
   ```bash
   $ secretspec init
   ✓ Created secretspec.toml with 0 secrets

   Next steps:
     1. secretspec config init    # Set up user configuration
     2. secretspec check          # Verify all secrets are set
     3. secretspec run -- your-command  # Run with secrets
   ```

2. **Set up provider backend:**
   ```bash
   $ secretspec config init
   ? Select your preferred provider backend:
   > 1password: 1Password password manager
     dotenv: Traditional .env files
     env: Read-only environment variables
     keyring: Uses system keychain (Recommended)
     lastpass: LastPass password manager
   ? Select your default profile:
   > development
     default
     none
   ✓ Configuration saved to /home/user/.config/secretspec/config.toml
   ```
3. **Check that all secrets are configured and configure them**
   ```bash
   $ secretspec check
   ```

4. **Run your application with secrets:**
   ```bash
   $ secretspec run -- npm start

   # Or with a specific profile and provider
   $ secretspec run --profile production --provider dotenv -- npm start
   ```

## Installation

### Static binary

```bash
$ curl -sSL https://secretspec.dev/install | sh
```

### Devenv.sh

See the [devenv integration guide](https://secretspec.dev/docs/devenv) for setup instructions.

### Nix

```bash
$ nix-env -iA secretspec -f https://github.com/NixOS/nixpkgs/tarball/nixpkgs-unstable
```

*Please, open pull requests once these hit your favorite distribution.*

## Configuration

### Project Configuration (`secretspec.toml`)

Each project has a `secretspec.toml` file that declares the required secrets:

```toml
[project]
name = "my-app"  # Inferred from current directory name when using `secretspec init`
revision = "1.0"
# Optional: extend other configuration files
extends = ["../shared/common", "../shared/auth"]

[profiles.default]
DATABASE_URL = { description = "PostgreSQL connection string", required = true }
REDIS_URL = { description = "Redis connection string", required = false, default = "redis://localhost:6379" }

# Profile-specific configurations
[profiles.development]
DATABASE_URL = { description = "PostgreSQL connection string", required = false, default = "sqlite://./dev.db" }
REDIS_URL = { description = "Redis connection string", required = false, default = "redis://localhost:6379" }

[profiles.production]
DATABASE_URL = { description = "PostgreSQL connection string", required = true }
REDIS_URL = { description = "Redis connection string", required = true }
```

## Profiles

Profiles define **HOW** your secret requirements vary across different environments. They allow you to specify different requirements, defaults, and validation rules for development, staging, production, or any custom environment.

### Understanding Profiles

Each profile can override the base secret definitions with environment-specific settings:

- **required**: Whether the secret must be provided (no default value)
- **default**: A default value to use if the secret isn't set
- **description**: Human-readable explanation of the secret's purpose

### Default Profile

The `default` profile serves as the base configuration. Other profiles inherit from it and can override specific settings:

```toml
[profiles.default]
DATABASE_URL = { description = "PostgreSQL connection string", required = true }
API_KEY = { description = "External API key", required = true }
CACHE_TTL = { description = "Cache time-to-live in seconds", required = false, default = "3600" }
```

### Environment-Specific Profiles

Define profiles for each environment where your application runs:

```toml
# Development: Use local defaults, make most secrets optional
[profiles.development]
DATABASE_URL = { description = "PostgreSQL connection string", required = false, default = "postgresql://localhost/myapp_dev" }
API_KEY = { description = "External API key", required = false, default = "dev-key-12345" }
DEBUG = { description = "Enable debug mode", required = false, default = "true" }

# Staging: Similar to production but with some relaxed requirements
[profiles.staging]
DATABASE_URL = { description = "PostgreSQL connection string", required = true }
API_KEY = { description = "External API key", required = true }
DEBUG = { description = "Enable debug mode", required = false, default = "false" }

# Production: All secrets required, no defaults
[profiles.production]
DATABASE_URL = { description = "PostgreSQL connection string", required = true }
API_KEY = { description = "External API key", required = true }
DEBUG = { description = "Enable debug mode", required = false, default = "false" }
SENTRY_DSN = { description = "Error tracking endpoint", required = true }
```

### Using Profiles

Select a profile when running commands:

```bash
# Use development profile
$ secretspec check --profile development
$ secretspec run --profile development -- npm start

# Use production profile
$ secretspec check --profile production
$ secretspec run --profile production -- npm start

# Set default profile in config
$ secretspec config init
? Select your default profile:
> development
  staging
  production
  default
```

### Profile Selection Priority

Profiles are selected in the following order:

1. **CLI argument**: `--profile` flag
2. **Environment variable**: `SECRETSPEC_PROFILE`
3. **User config**: Default profile in `~/.config/secretspec/config.toml`
4. **Fallback**: `default` profile

### Best Practices for Profiles

1. **Development Profile**: Use safe defaults that work locally
   - Make most secrets optional with local defaults
   - Use development-specific values (localhost URLs, test keys)
   - Enable debug/development features

2. **Production Profile**: Enforce strict requirements
   - Make all critical secrets required
   - Never include default values for sensitive data
   - Add production-only secrets (monitoring, analytics)

3. **Staging Profile**: Mirror production closely
   - Same requirements as production
   - May have different secret values
   - Useful for testing deployment workflows

4. **Custom Profiles**: Create profiles for specific needs
   - CI/CD environments
   - Testing configurations
   - Regional deployments

### Provider Configuration

The **WHERE** of secret storage depends on where your application runs. SecretSpec uses providers to abstract different storage backends, allowing the same `secretspec.toml` to work across development machines, CI/CD pipelines, and production environments.

SecretSpec provider can be configured through three methods (in order of precedence):

1. **User config file** (preferred): Set via `secretspec config init`. Stored at `~/.config/secretspec/config.toml` on Linux/macOS or `%APPDATA%\secretspec\config.toml` on Windows
2. **Environment variable**: `SECRETSPEC_PROVIDER`
3. **CLI arguments**: `--provider` flag on any command

## Provider Backends

SecretSpec includes five built-in provider backends:

- **keyring** - Secure system credential store integration
- **dotenv** - Local .env file storage
- **env** - Read-only environment variable access
- **lastpass** - LastPass password manager integration
- **1password** - 1Password secrets management

*Additional provider backends are welcome!**

### Keyring Provider (Recommended)

Stores secrets securely in your system's credential store (Keychain on macOS, Credential Manager on Windows, Secret Service on Linux).

```bash
# Use keyring for this command
$ secretspec check --provider keyring

# Set as default in global config
$ secretspec config init  # sets keyring as default
```

### .env File Provider

Stores secrets in a local `.env` file. Useful for development environments.

```bash
# Use .env file for this command
$ secretspec set DATABASE_URL --provider dotenv

# Set as default for this project
# Edit ~/.config/secretspec/config.toml to set project-specific provider
```

### Environment Variable Provider

⚠️ **Read-only backend for CI/CD compatibility**

Reads secrets directly from process environment variables. **Not encrypted** - primarily for backwards compatibility in CI/CD pipelines where secrets are already set as environment variables.

```bash
export DATABASE_URL="your-connection-string"

# Use environment variables (read-only)
$ secretspec get DATABASE_URL --provider env
your-connection-string

$ secretspec check --provider env
```

### LastPass Provider

Integrates with LastPass password manager for secure cloud-based secret storage.

```bash
# Use LastPass for this command
$ secretspec set DATABASE_URL --provider lastpass

# Check secrets from LastPass
$ secretspec check --provider lastpass
```

### 1Password Provider

Integrates with 1Password for team-based secret management.

```bash
# Use 1Password for this command
$ secretspec set DATABASE_URL --provider 1password

# Run with 1Password secrets
$ secretspec run --provider 1password -- npm start
```


## Configuration Inheritance

SecretSpec supports configuration inheritance through the `extends` field in the `[project]` section. This allows you to:

- Share common secrets across multiple projects
- Build layered configurations (base → shared → project-specific)
- Maintain DRY principles in your secret management

### Example: Shared Configuration

**shared/common/secretspec.toml:**
```toml
[project]
name = "common"
revision = "1.0"

[profiles.default]
DATABASE_URL = { description = "Main database", required = true }
REDIS_URL = { description = "Cache server", required = false, default = "redis://localhost:6379" }
```

**myapp/secretspec.toml:**
```toml
[project]
name = "myapp"
revision = "1.0"
extends = ["../shared/common"]

[profiles.default]
# Override DATABASE_URL description
DATABASE_URL = { description = "MyApp database", required = true }
# Add new app-specific secret
API_KEY = { description = "External API key", required = true }
```

### Inheritance Rules

- Multiple configs can be extended: `extends = ["../common", "../auth"]`
- Paths are relative to the extending file's directory
- The extending config takes precedence over extended configs
- Secrets are merged at the profile level
- Circular dependencies are detected and prevented

## Rust SDK

SecretSpec provides a proc macro that generates strongly-typed Rust structs from your `secretspec.toml` file at compile time.

### Add to your `Cargo.toml`:
```toml
[dependencies]
secretspec = { version = "0.1" }
```

### Basic Usage

```rust
// Generate typed structs from secretspec.toml
secretspec::define_secrets!("secretspec.toml");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load secrets with type-safe struct
    let secrets = SecretSpec::load(Provider::Keyring)?;

    // Field names are lowercased versions of secret names
    println!("Database: {}", secrets.database_url);  // DATABASE_URL -> database_url

    // Optional secrets are Option<String>
    if let Some(redis) = &secrets.redis_url {
        println!("Redis: {}", redis);
    }

    // Set all secrets as environment variables
    secrets.set_as_env_vars();

    Ok(())
}
```

### Profile-Specific Types

The macro generates exact types for each profile, ensuring compile-time safety:

```rust
// Load with profile-specific types for maximum type safety
match SecretSpec::load_profile(Provider::Keyring, Profile::Production)? {
    SecretSpecProfile::Production { api_key, database_url, redis_url, .. } => {
        // In production: api_key is String (required)
        // database_url is String (required)
        // redis_url might be String or Option<String> based on config
        println!("Production API key: {}", api_key);
    }
    SecretSpecProfile::Development { api_key, database_url, .. } => {
        // In development: api_key is Option<String> (has default)
        // database_url is Option<String> (has default)
        if let Some(key) = api_key {
            println!("Dev API key: {}", key);
        }
    }
    _ => {}
}
```

### Generated Types

The macro generates several types based on your `secretspec.toml`:

- **`SecretSpec`** - Main struct with union types (fields are `Option<String>` if optional in *any* profile)
- **`SecretSpecProfile`** - Enum with profile-specific variants containing exact types
- **`Profile`** - Enum of all profiles from your config (e.g., `Development`, `Production`)
- **`Provider`** - Type-safe provider selection (`Keyring`, `Dotenv`, `Env`)

### Type Rules

- Secret fields are named as lowercase versions of the environment variable (e.g., `DATABASE_URL` → `database_url`)
- A field is `String` if it's required and has no default in ALL profiles
- A field is `Option<String>` if it's optional or has a default in ANY profile
- Profile-specific types reflect the exact requirements for that profile

## Adding a New Provider Backend

To implement a new provider backend in this repository:

1. **Create a new backend module** in `src/provider/your_backend.rs`:
   ```rust
   use crate::Result;
   use super::Provider;

   pub struct YourBackendProvider {
       // Your backend-specific configuration
   }

   impl Provider for YourBackendProvider {
       fn get(&self, project: &str, key: &str, profile: Option<&str>) -> Result<Option<String>> {
           // Implementation
       }

       fn set(&self, project: &str, key: &str, value: &str, profile: Option<&str>) -> Result<()> {
           // Implementation
       }
   }
   ```

2. **Register your backend** in `src/provider/mod.rs`:
   ```rust
   // Add to module exports
   pub mod your_backend;
   pub use your_backend::YourBackendProvider;

   // Add to ProviderRegistry::new()
   backends.insert(
       "your_backend".to_string(),
       Box::new(YourBackendProvider::new()) as Box<dyn Provider>,
   );
   ```

3. **Use your new backend**:
   ```bash
   $ secretspec set SECRET_NAME --provider your_backend
   ```

## License

This project is licensed under the Apache License 2.0.
