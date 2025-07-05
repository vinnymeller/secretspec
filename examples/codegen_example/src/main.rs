// Include the generated code
include!(concat!(env!("OUT_DIR"), "/secrets.rs"));

use secretspec::codegen::{Profile, Provider};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("SecretSpec Code Generation Example\n");

    // Example 2: Load development profile with dotenv provider
    match SecretSpec::load_with(Provider::Dotenv, Profile::Development) {
        Ok(secrets) => {
            println!("   ✓ Loaded development secrets");
            println!(
                "   - Database URL: {} (using dev default)",
                secrets.database_url
            );
            println!("   - API Key: {} (using dev default)", secrets.api_key);
        }
        Err(e) => {
            println!("   ✗ Failed to load development secrets: {}", e);
        }
    }

    println!("\n3. Setting secrets as environment variables:");

    // Create a mock instance
    let mock_secrets = SecretSpec {
        database_url: "postgres://prod-server/myapp".to_string(),
        api_key: "prod-api-key-123".to_string(),
        redis_url: Some("redis://prod-cache:6379".to_string()),
        log_level: Some("error".to_string()),
    };

    // Set as environment variables
    mock_secrets.set_as_env_vars()?;
    println!("   ✓ Set all secrets as environment variables");

    Ok(())
}
