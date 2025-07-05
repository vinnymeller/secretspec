// Include the generated code from build.rs
mod basic {
    include!(concat!(env!("OUT_DIR"), "/test_configs/basic_secrets.rs"));
    
    #[test]
    fn test_basic_struct_fields() {
        // Create a mock instance to verify field types
        let test_secrets = SecretSpec {
            database_url: "postgres://localhost".to_string(),
            api_key: "test-key".to_string(),
            optional_secret: None,
            with_default: Some("custom-value".to_string()),
        };
        
        // Verify we can access fields
        assert_eq!(test_secrets.database_url, "postgres://localhost");
        assert_eq!(test_secrets.api_key, "test-key");
        assert!(test_secrets.optional_secret.is_none());
        assert_eq!(test_secrets.with_default, Some("custom-value".to_string()));
    }
    
    #[test] 
    fn test_load_method_exists() {
        // Verify the load() method exists and returns the correct type
        fn _test_compile() {
            let _result: Result<SecretSpec, secretspec::SecretSpecError> = SecretSpec::load();
        }
    }
    
    #[test]
    fn test_load_with_method_exists() {
        use secretspec::codegen::{Provider, Profile};
        
        // Verify load_with() method signature
        fn _test_compile() {
            let _result: Result<SecretSpec, secretspec::SecretSpecError> = 
                SecretSpec::load_with(Provider::Keyring, Profile::Development);
        }
    }
    
    #[test]
    fn test_set_as_env_vars_method() {
        let test_secrets = SecretSpec {
            database_url: "postgres://test".to_string(),
            api_key: "key123".to_string(),
            optional_secret: Some("optional".to_string()),
            with_default: None,
        };
        
        // Test that the method compiles and returns the correct type
        let result = test_secrets.set_as_env_vars();
        assert!(result.is_ok());
        
        // Verify env vars were set
        unsafe {
            assert_eq!(std::env::var("DATABASE_URL").unwrap(), "postgres://test");
            assert_eq!(std::env::var("API_KEY").unwrap(), "key123");
            assert_eq!(std::env::var("OPTIONAL_SECRET").unwrap(), "optional");
            
            // Clean up
            std::env::remove_var("DATABASE_URL");
            std::env::remove_var("API_KEY");
            std::env::remove_var("OPTIONAL_SECRET");
        }
    }
}

mod env_specific {
    include!(concat!(env!("OUT_DIR"), "/test_configs/env_secrets.rs"));
    
    #[test]
    fn test_env_specific_struct() {
        let test_secrets = SecretSpec {
            database_url: "postgres://prod".to_string(),
            api_endpoint: Some("https://api.example.com".to_string()),
        };
        
        assert_eq!(test_secrets.database_url, "postgres://prod");
        assert_eq!(test_secrets.api_endpoint, Some("https://api.example.com".to_string()));
    }
    
    #[test]
    fn test_profile_handling() {
        use secretspec::codegen::{Provider, Profile};
        
        // Verify that the generated code can handle different environments
        fn _test_compile() {
            let _dev: Result<SecretSpec, secretspec::SecretSpecError> = 
                SecretSpec::load_with(Provider::Dotenv, Profile::Development);
            
            let _prod: Result<SecretSpec, secretspec::SecretSpecError> = 
                SecretSpec::load_with(Provider::Keyring, Profile::Production);
            
            let _test: Result<SecretSpec, secretspec::SecretSpecError> = 
                SecretSpec::load_with(Provider::Env, Profile::Test);
        }
    }
}

#[test]
fn test_provider_enum() {
    use secretspec::codegen::Provider;
    
    // Test all provider variants
    match Provider::Keyring {
        Provider::Keyring => {},
        Provider::Dotenv => panic!("Wrong variant"),
        Provider::Env => panic!("Wrong variant"),
    }
}

#[test]
fn test_profile_enum() {
    use secretspec::codegen::Profile;
    
    // Test all profile variants
    match Profile::Development {
        Profile::Development => {},
        Profile::Production => panic!("Wrong variant"),
        Profile::Staging => panic!("Wrong variant"),
        Profile::Test => panic!("Wrong variant"),
    }
}