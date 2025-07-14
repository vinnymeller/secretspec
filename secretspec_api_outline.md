# SecretSpec Crate API Structure

## Overview
- **Crate**: secretspec v0.1.0
- **Total Items**: 580 (103 public)
- **Main Module**: `src/lib.rs`

## Module Structure

### Root Module (`secretspec`)

#### Core Types

1. **`SecretSpec`** (struct)
   - Main entry point for the library
   - Manages loading, validation, and retrieval of secrets
   - Methods:
     - `load()` - Loads using default configuration paths
     - `new()` - Creates instance with given configurations
     - `builder()` - Creates a `SecretSpecBuilder`
     - `check()` - Checks status of all secrets and prompts for missing ones
     - `validate()` - Validates all secrets in the specification
     - `validate_single()` - Validates and retrieves a single secret
     - `get()` - Retrieves and prints a secret value
     - `set()` - Sets a secret value in the storage backend
     - `run()` - Runs a command with secrets injected as environment variables
     - `import()` - Imports secrets from one provider to another
     - `write()` - Writes a new `secretspec.toml` based on a dotenv file

2. **`SecretSpecBuilder`** (struct)
   - Builder for creating `SecretSpec` instances with custom configuration

3. **`SecretSpecError`** (enum)
   - Main error type for secretspec operations
   - 16 variants for different error conditions

4. **`SecretSpecSecret`** (struct)
   - Represents a single secret configuration

5. **`ValidationResult`** (struct)
   - Result of secret validation operations

6. **`GetOptions`** (struct)
   - Options for retrieving secrets

#### Traits

1. **`SecretSpecSecretsExt`** (trait)
   - Extension trait for secret operations

#### Type Aliases

1. **`Result<T>`** - Alias for `std::result::Result<T, SecretSpecError>`

#### Public Functions

1. **`project_config_from_path()`** - Creates a ProjectConfig from a dotenv file
2. **`parse_spec()`** - Parses a spec from configuration
3. **`parse_spec_from_str()`** - Parses a spec from a string
4. **`get_example_toml()`** - Returns example TOML configuration
5. **`generate_toml_with_comments()`** - Generates TOML with helpful comments

### Provider Module (`secretspec::provider`)

The provider system implements a trait-based plugin architecture for managing secrets across different storage backends.

#### Core Trait

**`Provider`** (trait)
- Defines the interface for all secret storage providers
- Requirements: `Send + Sync`
- Methods:
  - `get()` - Retrieves a secret value
  - `set()` - Stores a secret value
  - `allows_set()` - Returns whether provider supports writing
  - `name()` - Returns provider name for display
  - `description()` - Returns brief description

#### Provider Implementations

1. **`KeyringProvider`** (struct)
   - System keyring integration (default provider)
   - Configuration: `KeyringConfig`

2. **`DotEnvProvider`** (struct)
   - Manages secrets in .env files
   - URI: `dotenv://.env.production`

3. **`EnvProvider`** (struct)
   - Environment variables (read-only)
   - URI: `env://`

4. **`OnePasswordProvider`** (struct)
   - 1Password integration
   - Configuration: `OnePasswordConfig`
   - URI: `1password://vault/items`

5. **`LastPassProvider`** (struct)
   - LastPass integration
   - Configuration: `LastPassConfig`
   - URI: `lastpass://folder`

#### Provider Management

1. **`ProviderRegistry`** (struct)
   - Registry for managing secret storage providers
   - Factory for creating providers from URIs

2. **`ProviderInfo`** (struct)
   - Information about a secret storage provider

## Re-exported Types

The crate re-exports types from `secretspec_types`:
- Core configuration types (ProjectConfig, SecretConfig)
- TOML parsing and serialization utilities
- Provider enum definitions

## Key Features

1. **Profile Support**
   - Different configurations for development, staging, production
   - Profile-aware storage paths: `{provider}/{project}/{profile}/{key}`

2. **URI-Based Provider Configuration**
   - Flexible provider instantiation via URI strings
   - Examples: `keyring://`, `dotenv://.env.production`

3. **Validation System**
   - Ensures all required secrets are present
   - Profile-based fallback to "default" profile
   - Support for optional secrets and defaults

4. **Type Safety**
   - Optional compile-time code generation via `secretspec-derive`
   - Strongly-typed access to secrets

5. **Command Execution**
   - Run commands with secrets injected as environment variables
   - Automatic secret validation before execution

## Usage Patterns

1. **Basic Usage**
   ```rust
   let spec = SecretSpec::load()?;
   spec.check(None, None)?;
   spec.run(vec!["npm".to_string(), "start".to_string()], None, None)?;
   ```

2. **Provider Creation**
   ```rust
   let provider = ProviderRegistry::create_from_string("keyring://")?;
   provider.set("myproject", "API_KEY", "secret123", "production")?;
   ```

3. **Custom Configuration**
   ```rust
   let spec = SecretSpec::builder()
       .default_provider("dotenv://.env.local")
       .build()?;
   ```