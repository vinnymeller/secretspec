#[cfg(feature = "codegen")]
mod codegen_integration_tests {
    use secretspec::codegen::{generate_types, Provider, Profile};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_generated_struct_syntax() {
        let temp_dir = TempDir::new().unwrap();
        let toml_path = temp_dir.path().join("secretspec.toml");
        
        let toml_content = r#"
[project]
name = "test-app"

[secrets.DATABASE_URL]
description = "Database connection string"
required = true

[secrets.API_KEY]
description = "API key"
required = true

[secrets.OPTIONAL_KEY]
description = "Optional key"
required = false
"#;
        
        fs::write(&toml_path, toml_content).unwrap();
        
        // Generate types
        generate_types(&toml_path, &temp_dir.path().to_path_buf()).unwrap();
        
        let generated_path = temp_dir.path().join("secrets.rs");
        let generated = fs::read_to_string(&generated_path).unwrap();
        
        // Verify the generated code structure
        assert!(generated.contains("pub struct SecretSpec"));
        assert!(generated.contains("pub database_url : String"));
        assert!(generated.contains("pub api_key : String"));
        assert!(generated.contains("pub optional_key : Option < String >"));
        
        // Verify methods are generated
        assert!(generated.contains("impl SecretSpec"));
        assert!(generated.contains("pub fn load ()"));
        assert!(generated.contains("pub fn load_with ("));
        assert!(generated.contains("pub fn set_as_env_vars ("));
    }

    #[test]
    fn test_generated_struct_with_profiles() {
        let temp_dir = TempDir::new().unwrap();
        let toml_path = temp_dir.path().join("secretspec.toml");
        
        let toml_content = r#"
[project]
name = "test-app"

[secrets.DATABASE_URL]
description = "Database connection string"
required = true

[secrets.DATABASE_URL.development]
required = false
default = "sqlite://./dev.db"

[secrets.DATABASE_URL.production]
required = true

[secrets.API_KEY]
description = "API key"
required = true
default = "prod-key"

[secrets.API_KEY.development]
default = "dev-key"
"#;
        
        fs::write(&toml_path, toml_content).unwrap();
        
        // Generate types
        generate_types(&toml_path, &temp_dir.path().to_path_buf()).unwrap();
        
        // Verify the generated file handles environments correctly
        let generated_path = temp_dir.path().join("secrets.rs");
        let generated = fs::read_to_string(&generated_path).unwrap();
        
        // Should have load_with method for profile support
        assert!(generated.contains("pub fn load_with"));
        assert!(generated.contains("secretspec :: codegen :: Profile :: Development"));
        assert!(generated.contains("secretspec :: codegen :: Profile :: Production"));
    }

    #[test]
    fn test_generated_struct_field_types() {
        let temp_dir = TempDir::new().unwrap();
        let toml_path = temp_dir.path().join("secretspec.toml");
        
        let toml_content = r#"
[project]
name = "test-app"

[secrets.REQUIRED_NO_DEFAULT]
description = "Required with no default"
required = true

[secrets.REQUIRED_WITH_DEFAULT]
description = "Required with default"
required = true
default = "default-value"

[secrets.OPTIONAL_NO_DEFAULT]
description = "Optional with no default"
required = false

[secrets.OPTIONAL_WITH_DEFAULT]
description = "Optional with default"
required = false
default = "optional-default"
"#;
        
        fs::write(&toml_path, toml_content).unwrap();
        
        // Generate types
        generate_types(&toml_path, &temp_dir.path().to_path_buf()).unwrap();
        
        let generated_path = temp_dir.path().join("secrets.rs");
        let generated = fs::read_to_string(&generated_path).unwrap();
        
        // Verify field types
        assert!(generated.contains("pub required_no_default : String"));
        assert!(generated.contains("pub required_with_default : Option < String >"));
        assert!(generated.contains("pub optional_no_default : Option < String >"));
        assert!(generated.contains("pub optional_with_default : Option < String >"));
        
        // Verify error handling for required fields
        assert!(generated.contains("RequiredSecretMissing"));
    }

    #[test]
    fn test_mock_secretspec_loading() {
        // This test simulates how the generated code would work with SecretSpec
        let temp_dir = TempDir::new().unwrap();
        let toml_path = temp_dir.path().join("secretspec.toml");
        
        let toml_content = r#"
[project]
name = "test-app"

[secrets.DATABASE_URL]
description = "Database URL"
required = true

[secrets.CACHE_URL]
description = "Cache URL"
required = false
default = "redis://localhost:6379"
"#;
        
        fs::write(&toml_path, toml_content).unwrap();
        
        // Change to temp directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();
        
        // Create a mock .env file
        fs::write(".env", "DATABASE_URL=postgres://test\n").unwrap();
        
        // Load secrets using the actual SecretSpec
        let spec = secretspec::SecretSpec::load().unwrap();
        let secrets = spec.get_all_secrets(Some("dotenv".to_string()), None).unwrap();
        
        // Verify the mock loading behavior
        assert_eq!(secrets.get("DATABASE_URL").unwrap(), "postgres://test");
        assert_eq!(secrets.get("CACHE_URL").unwrap(), "redis://localhost:6379");
        
        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_set_as_env_vars() {
        let temp_dir = TempDir::new().unwrap();
        let toml_path = temp_dir.path().join("secretspec.toml");
        
        let toml_content = r#"
[project]
name = "test-app"

[secrets.TEST_VAR_1]
description = "Test variable 1"
required = true

[secrets.TEST_VAR_2]
description = "Test variable 2"
required = false
"#;
        
        fs::write(&toml_path, toml_content).unwrap();
        
        // Generate types
        generate_types(&toml_path, &temp_dir.path().to_path_buf()).unwrap();
        
        let generated_path = temp_dir.path().join("secrets.rs");
        let generated = fs::read_to_string(&generated_path).unwrap();
        
        // Verify set_as_env_vars method is generated
        assert!(generated.contains("pub fn set_as_env_vars"));
        assert!(generated.contains("std :: env :: set_var"));
        
        // Verify unsafe blocks for set_var
        assert!(generated.contains("unsafe {"));
    }

    #[test]
    fn test_provider_and_profile_enums() {
        // Test that the Provider and Profile enums are accessible
        let _provider = Provider::Keyring;
        let _profile = Profile::Development;
        
        // Test enum matching
        match Provider::Dotenv {
            Provider::Keyring => panic!("Wrong provider"),
            Provider::Dotenv => {},
            Provider::Env => panic!("Wrong provider"),
        }
        
        match Profile::Production {
            Profile::Development => panic!("Wrong profile"),
            Profile::Production => {},
            Profile::Staging => panic!("Wrong profile"),
            Profile::Test => panic!("Wrong profile"),
        }
    }
}