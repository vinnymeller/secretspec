# SecretSpec Code Generation Example

This example demonstrates how to use SecretSpec's code generation feature to create strongly-typed secret structs.

## How it works

1. The `build.rs` file runs during compilation and generates a Rust struct from `secretspec.toml`
2. The generated struct has:
   - Required secrets as `String` fields
   - Optional secrets as `Option<String>` fields
   - Methods for loading from different providers and environments

## Running the example

```bash
# From this directory
cargo run

# Or from the workspace root
cargo run -p codegen-example
```

## Generated Code

The build script generates a struct like this:

```rust
pub struct SecretSpec {
    pub database_url: String,        // Required
    pub api_key: String,            // Required
    pub redis_url: Option<String>,  // Optional with default
    pub log_level: Option<String>,  // Optional with default
}

impl SecretSpec {
    pub fn load() -> Result<Self, secretspec::SecretSpecError> { ... }
    pub fn load_with(provider: Provider, environment: Environment) -> Result<Self, secretspec::SecretSpecError> { ... }
    pub fn set_as_env_vars(&self) -> Result<(), std::io::Error> { ... }
}
```

## Benefits

- **Type Safety**: Required secrets are guaranteed at compile time
- **IDE Support**: Auto-completion for all secret fields
- **No Runtime Surprises**: If it compiles, all required secrets are available
- **Profile Support**: Different requirements per profile