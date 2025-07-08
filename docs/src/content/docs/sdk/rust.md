---
title: Rust SDK
description: Type-safe Rust integration for SecretSpec
---

SecretSpec provides a Rust library with type-safe access to secrets through a derive macro that generates strongly-typed structs from your `secretspec.toml` file at compile time.

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
secretspec = { version = "0.1", features = ["macros"] }
```

Basic example:

```rust
// Generate typed structs from secretspec.toml
secretspec::define_secrets!("secretspec.toml");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load secrets with type-safe struct
    let secrets_wrapper = SecretSpec::load(
        Some(secretspec::Provider::Keyring), 
        None  // Use default profile
    )?;
    
    // Access secrets (field names are lowercased)
    let secrets = &secrets_wrapper.secrets;
    println!("Database: {}", secrets.database_url);  // DATABASE_URL → database_url

    // Optional secrets are Option<String>
    if let Some(redis) = &secrets.redis_url {
        println!("Redis: {}", redis);
    }

    // Set all secrets as environment variables
    secrets.set_as_env_vars();

    Ok(())
}
```

## Key APIs

### Generated Types

The macro generates types based on your `secretspec.toml`:

```rust
// Main struct with all secrets
pub struct SecretSpec {
    pub database_url: Option<String>,  // Optional if ANY profile has default
    pub api_key: String,               // Required if ALL profiles require it
}

// Available profiles
pub enum Profile {
    Default,
    Development,
    Production,
}

// Profile-specific types for compile-time safety
pub enum SecretSpecProfile {
    Development {
        database_url: Option<String>,  // Has default
        api_key: Option<String>,       // Has default
    },
    Production {
        database_url: String,          // Required
        api_key: String,              // Required
    },
}
```

### Loading Secrets

```rust
// Load with environment variables
std::env::set_var("SECRETSPEC_PROVIDER", "keyring");
std::env::set_var("SECRETSPEC_PROFILE", "production");
let secrets = SecretSpec::load(None, None)?;

// Load with specific provider and profile
let secrets = SecretSpec::load(
    Some(secretspec::Provider::Keyring),
    Some(Profile::Production)
)?;

// Load with profile-specific types
let secrets_wrapper = SecretSpec::load_as_profile(
    Some(secretspec::Provider::Keyring),
    Some(Profile::Production)
)?;

match secrets_wrapper.secrets {
    SecretSpecProfile::Production { database_url, api_key, .. } => {
        connect_to_database(&database_url);
        authenticate(&api_key);
    }
    _ => {}
}
```

### Library API (without macro)

```rust
use secretspec::{SecretSpec, ValidationResult};

let spec = SecretSpec::load()?;

// Validate and get secrets
let validation = spec.validate(
    Some("keyring".to_string()),
    Some("production".to_string())
)?;

if validation.is_valid() {
    let db_url = &validation.secrets["DATABASE_URL"];
    println!("Database URL: {}", db_url);
} else {
    eprintln!("Missing: {:?}", validation.missing_required);
}
```

## Integration Example

With Actix Web:

```rust
use actix_web::{web, App, HttpServer};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load secrets at startup
    let secrets = SecretSpec::load(
        Some(secretspec::Provider::Keyring),
        None
    ).expect("Failed to load secrets");

    // Set as environment variables
    secrets.set_as_env_vars();

    // Or pass as app data
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(secrets.clone()))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
```

## Error Handling

SecretSpec provides typed errors for common scenarios:

```rust
use secretspec::{SecretSpec, SecretSpecError};

match spec.validate(None, None) {
    Ok(validation) if !validation.is_valid() => {
        eprintln!("Missing required secrets: {:?}", validation.missing_required);
    }
    Err(SecretSpecError::NoProviderConfigured) => {
        eprintln!("No provider configured. Run 'secretspec config init'");
    }
    Err(SecretSpecError::RequiredSecretMissing(name)) => {
        eprintln!("Required secret '{}' not found", name);
        eprintln!("Run 'secretspec set {}' to configure", name);
    }
    Err(e) => eprintln!("Error: {}", e),
    Ok(_) => println!("All secrets loaded"),
}
```

## Tips

- **Field naming**: Environment variables like `DATABASE_URL` become `database_url` in Rust
- **Type rules**: A field is `Option<String>` if it has a default in any profile
- **Available providers**: `Keyring`, `Dotenv`, `Env`, `OnePassword`, `LastPass`
- **Testing**: Use the `Env` provider with test environment variables for unit tests