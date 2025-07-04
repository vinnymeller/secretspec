# SecretSpec

Declarative secrets manager for development workflows, supporting a variety of storage backends.

## Features

- **Declarative Configuration**: Define your secrets in `secretspec.toml` with descriptions and requirements
- **Multiple Storage Backends**: [Keyring](https://docs.rs/keyring/latest/keyring/) (system credential store), [.env](https://www.dotenv.org/), and environment variable support
- **Simple Migration**: `secretspec init` to migrate from existing `.env` files

## Installation

```bash
$ cargo install secretspec
```

(open pull requests once these hit distributions)

## Quick Start

1. **Initialize `secretspec.toml` from an existing .env file:**
   ```bash
   $ secretspec init
   ```

2. **Set up global configuration:**
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
   ```

## Configuration

### Project Configuration (`secretspec.toml`)

Each project has a `secretspec.toml` file that declares the required secrets:

```toml
[project]
name = "my-app"  # if not specified, inferred from current directory name

[secrets.DATABASE_URL]
description = "PostgreSQL connection string"
required = true

[secrets.API_KEY]
description = "External service API key"
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

[secrets.API_KEY.production]
required = true  # override: required in production
```

### Global Configuration

Global settings are stored in your system config directory:

```toml
[defaults]
storage = "keyring"  # or "dotenv"
```

## Storage Backends

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

### Available Storage Backends

SecretSpec includes three built-in storage backends:

- **keyring** - Secure system credential store integration
- **dotenv** - Local .env file storage 
- **env** - Read-only environment variable access

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

## Commands

### Project Management

```bash
# Initialize project from .env file
$ secretspec init

# Check if all required secrets are set
$ secretspec check

# Check secrets for specific environment
$ secretspec check --env production

# Run command with secrets injected
$ secretspec run -- your-command

# Run with environment-specific configuration
$ secretspec run --env production -- your-command
```

### Secret Management

```bash
# Set a secret (will prompt for value)
$ secretspec set SECRET_NAME

# Set a secret with value
$ secretspec set SECRET_NAME "secret-value"

# Set secret for specific environment
$ secretspec set SECRET_NAME --env production

# Get a secret value
$ secretspec get SECRET_NAME

# Get secret with environment-specific defaults
$ secretspec get SECRET_NAME --env development

# Use specific storage backend
$ secretspec set SECRET_NAME --storage keyring
```

### Configuration

```bash
# Initialize global configuration
$ secretspec config init

# Show current configuration
$ secretspec config show
```

## Security Best Practices

1. **Use keyring storage** for production and sensitive environments
2. **Add `.env` to `.gitignore`** if using env storage
3. **Never commit `secretspec.toml` with actual secret values** - it should only contain metadata
4. **Use required: true** for critical secrets
5. **Provide meaningful descriptions** for all secrets

## Migration from .env

If you're already using `.env` files:

1. Run `secretspec init --from .env` to create `secretspec.toml`
2. Set your secrets with `secretspec set SECRET_NAME`
3. Remove the original `.env` file
4. Use `secretspec run -- your-command` instead of loading `.env` manually

## Examples

### Basic Web Application

```toml
[project]
name = "web-app"

[secrets.DATABASE_URL]
description = "PostgreSQL connection string"
required = true

[secrets.SESSION_SECRET]
description = "Secret key for session encryption"
required = true

[secrets.SMTP_PASSWORD]
description = "Password for email service"
required = false
```

### Development vs Production

Use different storage backends and environment-specific configurations:

```bash
# Development: use .env file with development defaults
$ secretspec set DATABASE_URL --storage dotenv --env development

# Production: use keyring with production requirements
$ secretspec set DATABASE_URL --storage keyring --env production

# Check different environments
$ secretspec check --env development
$ secretspec check --env production
```

## Troubleshooting

### "No secretspec.toml found"
Run `secretspec init` in your project directory first.

### "No storage backend configured"
Run `secretspec config init` to set up global defaults, or use `--storage` flag.

### Keyring access issues
Ensure your system's credential store is unlocked and accessible.

## License

This project is licensed under the Apache License 2.0.
