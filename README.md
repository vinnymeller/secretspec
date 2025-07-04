# SecretSpec

Declarative secrets manager for development workflows, supporting a variety of storage backends.

See [announcement blog post for motivation](XXX).

## Features

- **Declarative Configuration**: Define your secrets in `secretspec.toml` with descriptions and requirements
- **Multiple Storage Backends**: [Keyring](https://docs.rs/keyring/latest/keyring/) (system credential store), [.env](https://www.dotenv.org/), and environment variable support
- **Simple Migration**: `secretspec init` to migrate from existing `.env` files

## Quick Start

1. **Initialize `secretspec.toml` (automatically detects .env)**
   ```bash
   $ secretspec init
   ```

2. **Set up storage backend:**
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
   $ secretspec run --environment production -- npm start
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

# Environment-specific overrides
[secrets.DATABASE_URL.development]
default = "sqlite://./dev.db"
required = false

[secrets.DATABASE_URL.production]
required = true  # no default - must be set
```

### Global Configuration

Global configuration is stored at `~/.config/secretspec/config.toml` on Linux/macOS or `%APPDATA%\secretspec\config.toml` on Windows.

Storage is specified in global configuration using `secretspec config init` or via `--storage` on CLI.

```toml
[defaults]
storage = "keyring"  # or "dotenv"
```

## Storage Backends

SecretSpec includes three built-in storage backends:

- **keyring** - Secure system credential store integration
- **dotenv** - Local .env file storage
- **env** - Read-only environment variable access

*Additional storage backends are welcome!**

### Keyring Storage (Recommended)

Stores secrets securely in your system's credential store (Keychain on macOS, Credential Manager on Windows, Secret Service on Linux).

```bash
# Use keyring for this command
$ secretspec check --storage keyring

# Set as default in global config
$ secretspec config init  # sets keyring as default
```

### .env File Storage

Stores secrets in a local `.env` file. Useful for development environments.

```bash
# Use .env file for this command
$ secretspec set DATABASE_URL --storage dotenv

# Set as default for this project
# Edit ~/.config/secretspec/config.toml to set project-specific storage
```

### Environment Variable Storage

⚠️ **Read-only backend for CI/CD compatibility**

Reads secrets directly from process environment variables. **Not encrypted** - primarily for backwards compatibility in CI/CD pipelines where secrets are already set as environment variables.

```bash
export DATABASE_URL="your-connection-string"

# Use environment variables (read-only)
$ secretspec get DATABASE_URL --storage env
your-connection-string

$ secretspec check --storage env
```

### Adding a New Storage Backend

To implement a new storage backend in this repository:

1. **Create a new backend module** in `src/storage/your_backend.rs`:
   ```rust
   use crate::Result;
   use super::StorageBackend;

   pub struct YourBackendStorage {
       // Your backend-specific configuration
   }

   impl StorageBackend for YourBackendStorage {
       fn get(&self, project: &str, key: &str) -> Result<Option<String>> {
           // Implementation
       }

       fn set(&self, project: &str, key: &str, value: &str) -> Result<()> {
           // Implementation
       }
   }
   ```

2. **Register your backend** in `src/storage/mod.rs`:
   ```rust
   // Add to module exports
   pub mod your_backend;
   pub use your_backend::YourBackendStorage;

   // Add to StorageRegistry::new()
   backends.insert(
       "your_backend".to_string(),
       Box::new(YourBackendStorage::new()) as Box<dyn StorageBackend>,
   );
   ```

3. **Use your new backend**:
   ```bash
   $ secretspec set SECRET_NAME --storage your_backend
   ```

## License

This project is licensed under the Apache License 2.0.
