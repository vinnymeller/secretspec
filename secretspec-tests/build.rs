fn main() {
    use std::fs;
    use std::path::Path;
    
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let test_configs_dir = Path::new(&out_dir).join("test_configs");
    fs::create_dir_all(&test_configs_dir).unwrap();
    
    // Test config 1: Basic types
    let basic_config = r#"
[project]
name = "test-basic"

[secrets.DATABASE_URL]
description = "Database connection string"
required = true

[secrets.API_KEY]
description = "API key"
required = true

[secrets.OPTIONAL_SECRET]
description = "Optional secret"
required = false

[secrets.WITH_DEFAULT]
description = "Secret with default"
required = false
default = "default-value"
"#;
    
    let basic_toml_path = test_configs_dir.join("basic.toml");
    fs::write(&basic_toml_path, basic_config).unwrap();
    
    // Generate types for basic config
    secretspec::codegen::generate_types(
        &basic_toml_path,
        &test_configs_dir
    ).unwrap();
    
    // Rename the generated file to avoid conflicts
    fs::rename(
        test_configs_dir.join("secrets.rs"),
        test_configs_dir.join("basic_secrets.rs")
    ).unwrap();
    
    // Test config 2: Profile-specific
    let env_config = r#"
[project]
name = "test-env"

[secrets.DATABASE_URL]
description = "Database URL"
required = true

[secrets.DATABASE_URL.development]
required = false
default = "sqlite://./dev.db"

[secrets.DATABASE_URL.production]
required = true

[secrets.API_ENDPOINT]
description = "API endpoint"
required = true
default = "https://api.prod.com"

[secrets.API_ENDPOINT.development]
default = "http://localhost:8080"
"#;
    
    let env_toml_path = test_configs_dir.join("env.toml");
    fs::write(&env_toml_path, env_config).unwrap();
    
    // Generate types for env config
    secretspec::codegen::generate_types(
        &env_toml_path,
        &test_configs_dir
    ).unwrap();
    
    // Rename the generated file
    fs::rename(
        test_configs_dir.join("secrets.rs"),
        test_configs_dir.join("env_secrets.rs")
    ).unwrap();
    
    println!("cargo:rerun-if-changed=build.rs");
}