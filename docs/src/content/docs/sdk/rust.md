---
title: Rust SDK
description: Type-safe Rust integration for SecretSpec
---

SecretSpec provides a Rust library with type-safe access to secrets through a derive macro that generates strongly-typed structs from your `secretspec.toml` file at compile time.

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
secretspec = { version = "0.1" }
```

Basic example:

```rust
// Generate typed structs from secretspec.toml
secretspec::define_secrets!("secretspec.toml");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load secrets using the builder pattern
    let secretspec = Secrets::builder()
        .with_provider("keyring")  // Can use provider name or URI like "dotenv:/path/to/.env"
        .with_profile("development")  // Can use string or Profile enum
        .load()?;  // All conversions and errors are handled here

    // Access secrets (field names are lowercased)
    println!("Database: {}", secretspec.secrets.database_url);  // DATABASE_URL → database_url

    // Optional secrets are Option<String>
    if let Some(redis) = &secretspec.secrets.redis_url {
        println!("Redis: {}", redis);
    }

    // Access profile and provider information
    println!("Using profile: {}", secretspec.profile);
    println!("Using provider: {}", secretspec.provider);

    // Set all secrets as environment variables
    secretspec.secrets.set_as_env_vars();

    Ok(())
}
```

## Loading with Profile-Specific Types

The `load_profile()` method on the builder provides profile-specific types for your secrets:

```rust
secretspec::define_secrets!("secretspec.toml");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load secrets with profile-specific types
    let secretspec = Secrets::builder()
        .with_provider("keyring")
        .with_profile(Profile::Production)
        .load_profile()?;
    
    // Access profile and provider information
    println!("Loaded profile: {}", secretspec.profile);
    println!("Using provider: {}", secretspec.provider);
    
    // Access secrets through profile-specific enum
    match secretspec.secrets {
        SecretSpecProfile::Production { database_url, api_key, .. } => {
            // In production: these are String (required)
            println!("Database: {}", database_url);
            println!("API Key: {}", api_key);
        }
        SecretSpecProfile::Development { database_url, api_key, .. } => {
            // In development: these might be Option<String> if they have defaults
            if let Some(db) = database_url {
                println!("Database: {}", db);
            }
        }
        _ => {}
    }
    
    Ok(())
}
```
