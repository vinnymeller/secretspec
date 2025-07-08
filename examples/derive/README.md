# SecretSpec Code Generation Example

This example demonstrates how to use SecretSpec's proc macro to create strongly-typed secret structs.

## How it works

1. The `secretspec::define_secrets!()` macro generates Rust structs from `secretspec.toml` at compile time
2. The generated types include:
   - `SecretSpec` struct with union types (safe for any profile)
   - `SecretSpecProfile` enum with profile-specific field types
   - `Profile` enum with all profiles from your TOML
   - Methods for loading from different providers and profiles

## Running the example

```bash
# From this directory
cargo run

# Or from the workspace root
cargo run -p codegen-example
```

## Generated Code

The proc macro generates types like this:

```rust
// Union type struct (safe for any profile)
pub struct SecretSpec {
    pub database_url: Option<String>,  // Optional because it has default in dev
    pub api_key: Option<String>,       // Optional because it has default in dev
    pub redis_url: Option<String>,     // Optional with default
    pub log_level: Option<String>,     // Optional with default
}

// Profile-specific enum
pub enum SecretSpecProfile {
    Development {
        database_url: Option<String>,  // Has default in dev profile
        api_key: Option<String>,       // Has default in dev profile
        redis_url: Option<String>,     // Optional with default
        log_level: Option<String>,     // Optional with default
    },
    Production {
        database_url: String,          // Required in production
        api_key: String,               // Required in production
        redis_url: Option<String>,     // Optional with default
        log_level: Option<String>,     // Optional with default
    }
}

impl SecretSpec {
    pub fn load(provider: Provider) -> Result<Self, SecretSpecError> { ... }
    pub fn load_profile(provider: Provider, profile: Profile) -> Result<SecretSpecProfile, SecretSpecError> { ... }
    pub fn set_as_env_vars(&self) { ... }
}
```
