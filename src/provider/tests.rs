#[cfg(test)]
mod tests {
    use crate::provider::ProviderRegistry;

    #[test]
    fn test_create_from_string_with_full_uris() {
        // Test basic 1password URI
        let provider = ProviderRegistry::create_from_string("1password://Private").unwrap();
        assert_eq!(provider.name(), "1password");

        // Test 1password with account
        let provider = ProviderRegistry::create_from_string("1password://work@Production").unwrap();
        assert_eq!(provider.name(), "1password");

        // Test 1password with token
        let provider =
            ProviderRegistry::create_from_string("1password+token://:ops_abc123@Private").unwrap();
        assert_eq!(provider.name(), "1password");
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

        let provider = ProviderRegistry::create_from_string("1password").unwrap();
        assert_eq!(provider.name(), "1password");

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
        // Test that 'onepassword' scheme gives proper error
        let result = ProviderRegistry::create_from_string("onepassword");
        match result {
            Err(err) => assert!(err.to_string().contains("Use '1password' instead")),
            Ok(_) => panic!("Expected error for 'onepassword' scheme"),
        }

        let result = ProviderRegistry::create_from_string("onepassword:");
        match result {
            Err(err) => assert!(err.to_string().contains("Use '1password' instead")),
            Ok(_) => panic!("Expected error for 'onepassword:' scheme"),
        }

        let result = ProviderRegistry::create_from_string("onepassword://Private");
        match result {
            Err(err) => assert!(err.to_string().contains("Use '1password' instead")),
            Ok(_) => panic!("Expected error for 'onepassword://' scheme"),
        }
    }

    #[test]
    fn test_dotenv_with_custom_path() {
        // Test dotenv provider with custom path
        let provider =
            ProviderRegistry::create_from_string("dotenv://localhost/custom/path/.env").unwrap();
        assert_eq!(provider.name(), "dotenv");

        // Test with the simplified format
        let provider = ProviderRegistry::create_from_string("dotenv:/custom/path/.env").unwrap();
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
}
