#[cfg(feature = "codegen")]
mod codegen_tests {
    use secretspec::codegen::generate_types;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_generate_types_basic() {
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
required = false
default = "default-key"

[secrets.REDIS_URL]
description = "Redis URL"
required = false
"#;
        
        fs::write(&toml_path, toml_content).unwrap();
        
        // Generate types
        generate_types(&toml_path, &temp_dir.path().to_path_buf()).unwrap();
        
        // Check that the file was created
        let generated_path = temp_dir.path().join("secrets.rs");
        assert!(generated_path.exists());
        
        // Read and verify generated content
        let generated = fs::read_to_string(&generated_path).unwrap();
        
        // Debug print
        println!("Generated code:\n{}", generated);
        
        // Check for struct definition
        assert!(generated.contains("pub struct SecretSpec"));
        
        // Check for required field as String (no spaces in generated output)
        assert!(generated.contains("pub database_url : String"));
        
        // Check for optional fields as Option<String> (no spaces in generated output)
        assert!(generated.contains("pub api_key : Option < String >"));
        assert!(generated.contains("pub redis_url : Option < String >"));
        
        // Check for methods
        assert!(generated.contains("pub fn load ()"));
        assert!(generated.contains("pub fn load_with ("));
        assert!(generated.contains("pub fn set_as_env_vars ("));
    }

    #[test]
    fn test_generate_types_with_profiles() {
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
"#;
        
        fs::write(&toml_path, toml_content).unwrap();
        
        // Generate types
        generate_types(&toml_path, &temp_dir.path().to_path_buf()).unwrap();
        
        let generated_path = temp_dir.path().join("secrets.rs");
        let generated = fs::read_to_string(&generated_path).unwrap();
        
        // Even with profile overrides, the base type should be determined
        // by the root configuration
        assert!(generated.contains("pub database_url : String"));
    }

    #[test]
    fn test_generate_types_all_optional() {
        let temp_dir = TempDir::new().unwrap();
        let toml_path = temp_dir.path().join("secretspec.toml");
        
        let toml_content = r#"
[project]
name = "test-app"

[secrets.OPTIONAL_1]
description = "Optional secret 1"
required = false

[secrets.OPTIONAL_2]
description = "Optional secret 2"
required = false
default = "default-value"
"#;
        
        fs::write(&toml_path, toml_content).unwrap();
        
        generate_types(&toml_path, &temp_dir.path().to_path_buf()).unwrap();
        
        let generated_path = temp_dir.path().join("secrets.rs");
        let generated = fs::read_to_string(&generated_path).unwrap();
        
        // All fields should be optional
        assert!(generated.contains("pub optional_1 : Option < String >"));
        assert!(generated.contains("pub optional_2 : Option < String >"));
    }

    #[test]
    fn test_field_name_conversion() {
        let temp_dir = TempDir::new().unwrap();
        let toml_path = temp_dir.path().join("secretspec.toml");
        
        let toml_content = r#"
[project]
name = "test-app"

[secrets.UPPER_CASE_NAME]
description = "Test uppercase conversion"
required = true

[secrets.MixedCaseName]
description = "Test mixed case"
required = true
"#;
        
        fs::write(&toml_path, toml_content).unwrap();
        
        generate_types(&toml_path, &temp_dir.path().to_path_buf()).unwrap();
        
        let generated_path = temp_dir.path().join("secrets.rs");
        let generated = fs::read_to_string(&generated_path).unwrap();
        
        // Field names should be lowercase
        assert!(generated.contains("pub upper_case_name : String"));
        assert!(generated.contains("pub mixedcasename : String"));
    }

    #[test]
    fn test_required_secret_handling() {
        let temp_dir = TempDir::new().unwrap();
        let toml_path = temp_dir.path().join("secretspec.toml");
        
        let toml_content = r#"
[project]
name = "test-app"

[secrets.REQUIRED_SECRET]
description = "This is required"
required = true

[secrets.REQUIRED_WITH_DEFAULT]
description = "Required but has default"
required = true
default = "default-value"
"#;
        
        fs::write(&toml_path, toml_content).unwrap();
        
        generate_types(&toml_path, &temp_dir.path().to_path_buf()).unwrap();
        
        let generated_path = temp_dir.path().join("secrets.rs");
        let generated = fs::read_to_string(&generated_path).unwrap();
        
        // Required without default should be String
        assert!(generated.contains("pub required_secret : String"));
        
        // Required with default should be Option<String>
        assert!(generated.contains("pub required_with_default : Option < String >"));
    }
}