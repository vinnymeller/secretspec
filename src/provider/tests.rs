use crate::Result;
use crate::provider::{Provider, ProviderRegistry};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Mock provider for testing
pub struct MockProvider {
    storage: Arc<Mutex<HashMap<String, String>>>,
}

impl MockProvider {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Provider for MockProvider {
    fn get(&self, project: &str, key: &str, profile: &str) -> Result<Option<String>> {
        let storage = self.storage.lock().unwrap();
        let full_key = format!("{}/{}/{}", project, profile, key);
        Ok(storage.get(&full_key).cloned())
    }

    fn set(&self, project: &str, key: &str, value: &str, profile: &str) -> Result<()> {
        let mut storage = self.storage.lock().unwrap();
        let full_key = format!("{}/{}/{}", project, profile, key);
        storage.insert(full_key, value.to_string());
        Ok(())
    }

    fn name(&self) -> &'static str {
        "mock"
    }
}

#[test]
fn test_create_from_string_with_full_uris() {
    // Test basic onepassword URI
    let provider = ProviderRegistry::create_from_string("onepassword://Private").unwrap();
    assert_eq!(provider.name(), "onepassword");

    // Test onepassword with account
    let provider = ProviderRegistry::create_from_string("onepassword://work@Production").unwrap();
    assert_eq!(provider.name(), "onepassword");

    // Test onepassword with token
    let provider =
        ProviderRegistry::create_from_string("onepassword+token://:ops_abc123@Private").unwrap();
    assert_eq!(provider.name(), "onepassword");
}

#[test]
fn test_create_from_string_with_plain_names() {
    // Test plain provider names
    let provider = ProviderRegistry::create_from_string("env").unwrap();
    assert_eq!(provider.name(), "env");

    let provider = ProviderRegistry::create_from_string("keyring").unwrap();
    assert_eq!(provider.name(), "keyring");

    let provider = ProviderRegistry::create_from_string("dotenv").unwrap();
    assert_eq!(provider.name(), "dotenv");

    // Test onepassword separately to debug the issue
    match ProviderRegistry::create_from_string("onepassword") {
        Ok(provider) => assert_eq!(provider.name(), "onepassword"),
        Err(e) => panic!("Failed to create onepassword provider: {}", e),
    }

    let provider = ProviderRegistry::create_from_string("lastpass").unwrap();
    assert_eq!(provider.name(), "lastpass");
}

#[test]
fn test_create_from_string_with_colon() {
    // Test provider names with colon
    let provider = ProviderRegistry::create_from_string("env:").unwrap();
    assert_eq!(provider.name(), "env");

    let provider = ProviderRegistry::create_from_string("keyring:").unwrap();
    assert_eq!(provider.name(), "keyring");
}

#[test]
fn test_invalid_onepassword_scheme() {
    // Test that '1password' scheme gives proper error suggesting 'onepassword'
    let result = ProviderRegistry::create_from_string("1password");
    match result {
        Err(err) => assert!(err.to_string().contains("Use 'onepassword' instead")),
        Ok(_) => panic!("Expected error for '1password' scheme"),
    }

    let result = ProviderRegistry::create_from_string("1password:");
    match result {
        Err(err) => assert!(err.to_string().contains("Use 'onepassword' instead")),
        Ok(_) => panic!("Expected error for '1password:' scheme"),
    }

    let result = ProviderRegistry::create_from_string("1password://Private");
    match result {
        Err(err) => assert!(err.to_string().contains("Use 'onepassword' instead")),
        Ok(_) => panic!("Expected error for '1password://' scheme"),
    }
}

#[test]
fn test_dotenv_with_custom_path() {
    // Test dotenv provider with relative path - host part becomes first folder
    let provider = ProviderRegistry::create_from_string("dotenv://custom/path/to/.env").unwrap();
    assert_eq!(provider.name(), "dotenv");

    // Test with absolute path format
    let provider = ProviderRegistry::create_from_string("dotenv:///custom/path/.env").unwrap();
    assert_eq!(provider.name(), "dotenv");
}

#[test]
fn test_unknown_provider() {
    let result = ProviderRegistry::create_from_string("unknown");
    assert!(result.is_err());
    match result {
        Err(crate::SecretSpecError::ProviderNotFound(scheme)) => {
            assert_eq!(scheme, "unknown");
        }
        _ => panic!("Expected ProviderNotFound error"),
    }
}

#[test]
fn test_dotenv_shorthand_from_docs() {
    // Test the example from line 187 of registry.rs
    let provider = ProviderRegistry::create_from_string("dotenv:.env.production").unwrap();
    assert_eq!(provider.name(), "dotenv");
}

#[test]
fn test_documentation_examples() {
    // Test examples from the documentation

    // From line 102: onepassword://work@Production
    let provider = ProviderRegistry::create_from_string("onepassword://work@Production").unwrap();
    assert_eq!(provider.name(), "onepassword");

    // From line 107: dotenv:/path/to/.env
    let provider = ProviderRegistry::create_from_string("dotenv:/path/to/.env").unwrap();
    assert_eq!(provider.name(), "dotenv");

    // From line 115: lastpass://folder
    let provider = ProviderRegistry::create_from_string("lastpass://folder").unwrap();
    assert_eq!(provider.name(), "lastpass");

    // Test dotenv examples from provider list
    let provider = ProviderRegistry::create_from_string("dotenv://path").unwrap();
    assert_eq!(provider.name(), "dotenv");
}

#[test]
fn test_edge_cases_and_normalization() {
    // Test scheme-only format (mentioned in docs line 151)
    let provider = ProviderRegistry::create_from_string("keyring:").unwrap();
    assert_eq!(provider.name(), "keyring");

    // Test dotenv special case without authority (line 152-153)
    let provider = ProviderRegistry::create_from_string("dotenv:/absolute/path").unwrap();
    assert_eq!(provider.name(), "dotenv");

    // Test normalized URIs with localhost (line 154)
    let provider = ProviderRegistry::create_from_string("env://localhost").unwrap();
    assert_eq!(provider.name(), "env");
}

#[test]
fn test_documentation_example_line_184() {
    // Test the exact example from line 184 of registry.rs
    let provider = ProviderRegistry::create_from_string("onepassword://vault/Production").unwrap();
    assert_eq!(provider.name(), "onepassword");
}

#[test]
fn test_url_parsing_behavior() {
    use url::Url;

    // Test how URLs are actually parsed
    let url = "onepassword://vault/Production".parse::<Url>().unwrap();
    assert_eq!(url.scheme(), "onepassword");
    assert_eq!(url.host_str(), Some("vault"));
    assert_eq!(url.path(), "/Production");

    // Test dotenv URL parsing - host part becomes part of the path
    let url = "dotenv://path/to/.env".parse::<Url>().unwrap();
    assert_eq!(url.scheme(), "dotenv");
    assert_eq!(url.host_str(), Some("path"));
    assert_eq!(url.path(), "/to/.env");
}
