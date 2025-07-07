// Use the proc macro to generate typed structs from secretspec.toml
// This generates: SecretSpec, SecretSpecProfile, Profile, and Provider types
secretspec::define_secrets!("secretspec.toml");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("SecretSpec Code Generation Example\n");

    // Create a .env file for testing
    std::fs::write(
        ".env",
        "DATABASE_URL=postgres://localhost/testdb\nAPI_KEY=test-key-123\n",
    )?;

    // Example 1: Load with union types (safe for any profile)
    println!("1. Loading secrets with union types:");
    match SecretSpec::load(Some(Provider::Dotenv), None) {
        Ok(secrets) => {
            println!("   ✓ Loaded successfully");
            if let Some(database_url) = &secrets.database_url {
                println!("   - Database URL: {}", database_url);
            }
            if let Some(api_key) = &secrets.api_key {
                println!("   - API Key: {} (found)", api_key);
            } else {
                println!("   - API Key: None");
            }
            if let Some(redis_url) = &secrets.redis_url {
                println!("   - Redis URL: {}", redis_url);
            }
            if let Some(log_level) = &secrets.log_level {
                println!("   - Log Level: {}", log_level);
            }
        }
        Err(e) => {
            println!("   ✗ Failed to load secrets: {}", e);
        }
    }

    // Example 2: Load development profile with exact types
    println!("\n2. Loading development profile:");
    match SecretSpec::load_as_profile(Some(Provider::Dotenv), Some(Profile::Development)) {
        Ok(SecretSpecProfile::Development {
            database_url,
            api_key,
            redis_url,
            log_level,
        }) => {
            println!("   ✓ Loaded development profile");
            // In development profile, both database_url and api_key have defaults
            if let Some(url) = database_url {
                println!("   - Database URL: {}", url);
            }
            if let Some(key) = api_key {
                println!("   - API Key: {}", key);
            }
            if let Some(url) = redis_url {
                println!("   - Redis URL: {}", url);
            }
            if let Some(level) = log_level {
                println!("   - Log Level: {}", level);
            }
        }
        Err(e) => {
            println!("   ✗ Failed to load development profile: {}", e);
        }
    }

    println!("\n3. Setting secrets as environment variables:");
    if let Ok(secrets) = SecretSpec::load(Some(Provider::Dotenv), None) {
        secrets.set_as_env_vars();
        println!("   ✓ Set all secrets as environment variables");

        // Verify they were set
        println!(
            "   - DATABASE_URL env: {:?}",
            std::env::var("DATABASE_URL").ok()
        );
        println!("   - API_KEY env: {:?}", std::env::var("API_KEY").ok());
    }

    // Clean up
    std::fs::remove_file(".env").ok();

    Ok(())
}
