# SecretSpec

Declarative secrets manager for development workflows, supporting a variety of storage backends.

See [announcement blog post for motivation](XXX).

## Features

- **Declarative Configuration**: Define your secrets in `secretspec.toml` with descriptions and requirements
- **Multiple Provider Backends**: [Keyring](https://docs.rs/keyring/latest/keyring/) (system credential store), [.env](https://www.dotenv.org/), and environment variable support
- **Type-Safe Rust Library**: Generate strongly-typed structs from your `secretspec.toml` for compile-time safety
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

### Global Configuration

Global configuration is stored at `~/.config/secretspec/config.toml` on Linux/macOS or `%APPDATA%\secretspec\config.toml` on Windows.

Provider is specified in global configuration using `secretspec config init` or via `--provider` on CLI.

```toml
[defaults]
provider = "keyring"  # or "dotenv"
```

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

### Adding a New Provider Backend

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

## Using SecretSpec as a Rust Library

### Basic Usage

```rust
use secretspec::SecretSpec;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let spec = SecretSpec::load()?;
    
    // Get all secrets for the current environment
    let secrets = spec.get_all_secrets(None, None)?;
    
    // Access individual secrets
    let db_url = secrets.get("DATABASE_URL")
        .ok_or("DATABASE_URL not found")?;
    
    println!("Connecting to: {}", db_url);
    Ok(())
}
```

### Type-Safe Code Generation

SecretSpec can generate strongly-typed Rust structs from your `secretspec.toml` file:

1. **Add to your `Cargo.toml`:**
   ```toml
   [dependencies]
   secretspec = "0.1"
   
   [build-dependencies]
   secretspec = { version = "0.1", features = ["codegen"] }
   ```

2. **Create a `build.rs`:**
   ```rust
   fn main() {
       secretspec::codegen::generate_types(
           "secretspec.toml",
           &std::env::var("OUT_DIR").unwrap()
       ).unwrap();
   }
   ```

3. **Use the generated types:**
   ```rust
   // Include the generated code
   include!(concat!(env!("OUT_DIR"), "/secrets.rs"));
   
   fn main() -> Result<(), Box<dyn std::error::Error>> {
       // Load with strongly-typed struct
       let secrets = SecretSpec::load()?;
       
       // Required secrets are guaranteed to exist
       println!("Database: {}", secrets.database_url);
       
       // Optional secrets are Option<String>
       if let Some(redis) = &secrets.redis_url {
           println!("Redis: {}", redis);
       }
       
       // Set all secrets as environment variables
       secrets.set_as_env_vars()?;
       
       Ok(())
   }
   ```

4. **Load with specific provider and profile:**
   ```rust
   use secretspec::codegen::{Provider, Profile};
   
   let secrets = SecretSpec::load_with(
       Provider::Keyring,
       Profile::Production
   )?;
   ```

The generated struct will have:
- Required secrets (with no default) as `String` fields
- Optional secrets (or those with defaults) as `Option<String>` fields
- Compile-time type safety - if it compiles, all required secrets are available

## Profile Support

SecretSpec supports profile-specific configuration overrides:

```toml
[secrets.API_KEY]
description = "API key for external service"
required = true

[secrets.API_KEY.development]
required = false
default = "dev-api-key"

[secrets.API_KEY.production]
required = true  # No default - must be explicitly set
```

Use profiles with any command:
```bash
# Development profile
$ secretspec check --profile development

# Production profile  
$ secretspec run --profile production -- npm start
```

## License

This project is licensed under the Apache License 2.0.
