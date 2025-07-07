# SecretSpec

[![CI](https://github.com/cachix/secretspec/actions/workflows/ci.yml/badge.svg)](https://github.com/cachix/secretspec/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/secretspec)](https://crates.io/crates/secretspec)
[![docs.rs](https://docs.rs/secretspec/badge.svg)](https://docs.rs/secretspec)
[![Discord channel](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fdiscord.com%2Fapi%2Finvites%2FnaMgvexb6q%3Fwith_counts%3Dtrue&query=%24.approximate_member_count&logo=discord&logoColor=white&label=Discord%20users&color=green&style=flat)](https://discord.gg/naMgvexb6q)
![License: Apache 2.0](https://img.shields.io/github/license/cachix/secretspec)

Declarative secrets manager for development workflows, supporting a variety of storage backends.

See [announcement blog post for motivation](XXX).

## Features

- **Declarative Configuration**: Define your secrets in `secretspec.toml` with descriptions and requirements
- **Multiple Provider Backends**: [Keyring](https://docs.rs/keyring/latest/keyring/) (system credential store), [.env](https://www.dotenv.org/), and environment variable support
- **Type-Safe Rust SDK**: Generate strongly-typed structs from your `secretspec.toml` for compile-time safety
- **Profile Support**: Override secret requirements and defaults per profile (development, production, etc.)
- **Simple Migration**: `secretspec init` to migrate from existing `.env` files

## Quick Start

1. **Initialize `secretspec.toml` (automatically detects .env)**
   ```bash
   $ secretspec init
   ```

2. **Set up provider backend:**
   ```bash
   $ secretspec config init
   ```

3. **Set your secrets:**
   ```bash
   $ secretspec set DATABASE_URL
   $ secretspec set API_KEY
   ```

4. **Check that all secrets are configured:**
   ```bash
   $ secretspec check
   ```

5. **Run your application with secrets:**
   ```bash
   $ secretspec run -- npm start
   
   # Or with a specific profile
   $ secretspec run --profile production -- npm start
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

[secrets.DATABASE_URL]
description = "PostgreSQL connection string"
required = true

[secrets.REDIS_URL]
description = "Redis connection string"
required = false
default = "redis://localhost:6379"

# Profile-specific overrides
[secrets.DATABASE_URL.development]
default = "sqlite://./dev.db"
required = false

[secrets.DATABASE_URL.production]
required = true  # no default - must be set
```

### Provider Configuration

SecretSpec provider can be configured through three methods (in order of precedence):

1. **User config file** (preferred): Set via `secretspec config init`. Stored at `~/.config/secretspec/config.toml` on Linux/macOS or `%APPDATA%\secretspec\config.toml` on Windows
2. **Environment variable**: `SECRETSPEC_PROVIDER`
3. **CLI arguments**: `--provider` flag on any command

## Provider Backends

SecretSpec includes three built-in provider backends:

- **keyring** - Secure system credential store integration
- **dotenv** - Local .env file storage
- **env** - Read-only environment variable access

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


## Rust SDK

SecretSpec provides a proc macro that generates strongly-typed Rust structs from your `secretspec.toml` file at compile time.

### Add to your `Cargo.toml`:
```toml
[dependencies]
secretspec = { version = "0.1", features = ["codegen"] }
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
