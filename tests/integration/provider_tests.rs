use secretspec::provider::ProviderRegistry;

fn generate_test_project_name() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros();
    let suffix = timestamp % 100000;
    format!("secretspec_test_{}", suffix)
}

fn get_test_providers() -> Vec<String> {
    std::env::var("SECRETSPEC_TEST_PROVIDERS")
        .unwrap_or_else(|_| String::new())
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().to_string())
        .collect()
}

#[cfg(test)]
mod generic_provider_tests {
    use super::*;
    use tempfile::TempDir;

    fn create_provider_with_temp_path(
        provider_name: &str,
    ) -> (Box<dyn secretspec::provider::Provider>, Option<TempDir>) {
        match provider_name {
            "dotenv" => {
                let temp_dir = TempDir::new().expect("Create temp directory");
                let dotenv_path = temp_dir.path().join(".env");
                let provider_spec = format!("dotenv:{}", dotenv_path.to_str().unwrap());
                let provider = ProviderRegistry::create_from_string(&provider_spec)
                    .expect("Should create dotenv provider with path");
                (provider, Some(temp_dir))
            }
            _ => {
                let provider = ProviderRegistry::create_from_string(provider_name)
                    .expect(&format!("{} provider should exist", provider_name));
                (provider, None)
            }
        }
    }

    #[test]
    fn test_basic_workflow() {
        let providers = get_test_providers();
        if providers.is_empty() {
            eprintln!("No providers specified. Set SECRETSPEC_TEST_PROVIDERS=keyring,dotenv,env");
            return;
        }

        for provider_name in providers {
            println!("Testing provider: {}", provider_name);

            let (provider, _temp_dir) = create_provider_with_temp_path(&provider_name);
            let project_name = generate_test_project_name();

            // Test 1: Get non-existent secret
            let result = provider.get(&project_name, "TEST_PASSWORD", "default");
            match result {
                Ok(None) => {
                    // Expected: key doesn't exist
                }
                Ok(Some(_)) => {
                    panic!("[{}] Should not find non-existent secret", provider_name);
                }
                Err(_) => {
                    // Some providers may return error instead of None
                }
            }

            // Test 2: Try to set a secret (may fail for read-only providers)
            let test_value = format!("test_password_{}", provider_name);
            match provider.set(&project_name, "TEST_PASSWORD", &test_value, "default") {
                Ok(_) => {
                    // If set succeeded, verify we can retrieve it
                    let retrieved = provider
                        .get(&project_name, "TEST_PASSWORD", "default")
                        .expect(&format!(
                            "[{}] Should not error when getting after set",
                            provider_name
                        ));

                    match retrieved {
                        Some(value) => {
                            assert_eq!(
                                value, test_value,
                                "[{}] Retrieved value should match set value",
                                provider_name
                            );
                        }
                        None => {
                            panic!("[{}] Should find secret after setting it", provider_name);
                        }
                    }
                }
                Err(e) => {
                    // Provider is read-only or doesn't support set, that's ok
                    println!(
                        "[{}] Provider doesn't support set operation: {:?}",
                        provider_name, e
                    );
                }
            }
        }
    }

    #[test]
    fn test_multiple_secrets() {
        let providers = get_test_providers();
        if providers.is_empty() {
            eprintln!("No providers specified. Set SECRETSPEC_TEST_PROVIDERS=keyring,dotenv,env");
            return;
        }

        for provider_name in providers {
            println!("Testing multiple secrets for provider: {}", provider_name);

            let (provider, _temp_dir) = create_provider_with_temp_path(&provider_name);
            let project_name = generate_test_project_name();

            let secrets = vec![
                ("DATABASE_URL", "postgres://localhost/test"),
                ("API_KEY", "sk_test_123456"),
                ("JWT_SECRET", "super_secret_jwt_key"),
            ];

            let mut set_keys = Vec::new();

            // Try to set multiple secrets
            for (key, value) in &secrets {
                match provider.set(&project_name, key, value, "default") {
                    Ok(_) => {
                        set_keys.push((*key, *value));
                    }
                    Err(e) => {
                        // Provider doesn't support set, skip rest of test
                        println!(
                            "[{}] Provider doesn't support set operation: {:?}",
                            provider_name, e
                        );
                        return;
                    }
                }
            }

            // Retrieve and verify all set secrets
            for (key, expected_value) in &set_keys {
                let result = provider.get(&project_name, key, "default").expect(&format!(
                    "[{}] Should not error when getting {}",
                    provider_name, key
                ));

                match result {
                    Some(value) => {
                        assert_eq!(
                            &value, expected_value,
                            "[{}] Value mismatch for {}",
                            provider_name, key
                        );
                    }
                    None => {
                        panic!("[{}] Should find {} after setting it", provider_name, key);
                    }
                }
            }
        }
    }

    #[test]
    fn test_special_characters() {
        let providers = get_test_providers();
        if providers.is_empty() {
            eprintln!("No providers specified. Set SECRETSPEC_TEST_PROVIDERS=keyring,dotenv,env");
            return;
        }

        for provider_name in providers {
            println!("Testing special characters for provider: {}", provider_name);

            let (provider, _temp_dir) = create_provider_with_temp_path(&provider_name);
            let project_name = generate_test_project_name();

            let test_cases = vec![
                ("SPACED_VALUE", "value with spaces"),
                ("NEWLINE_VALUE", "value\nwith\nnewlines"),
                ("SPECIAL_CHARS", "!@#%^&*()_+-=[]{}|;',./<>?"),
                ("UNICODE_VALUE", "🔐 Secret with émojis and ñ"),
            ];

            let mut set_keys = Vec::new();

            for (key, value) in &test_cases {
                match provider.set(&project_name, key, value, "default") {
                    Ok(_) => {
                        set_keys.push((*key, *value));

                        let result = provider.get(&project_name, key, "default").expect(&format!(
                            "[{}] Should not error when getting {}",
                            provider_name, key
                        ));

                        match result {
                            Some(retrieved) => {
                                assert_eq!(
                                    &retrieved, value,
                                    "[{}] Special characters should be preserved for {}",
                                    provider_name, key
                                );
                            }
                            None => {
                                panic!("[{}] Should find {} after setting it", provider_name, key);
                            }
                        }
                    }
                    Err(e) => {
                        // Provider doesn't support set, skip rest
                        println!(
                            "[{}] Provider doesn't support set operation: {:?}",
                            provider_name, e
                        );
                        break;
                    }
                }
            }
        }
    }

    #[test]
    fn test_profile_support() {
        let providers = get_test_providers();
        if providers.is_empty() {
            eprintln!("No providers specified. Set SECRETSPEC_TEST_PROVIDERS=keyring,dotenv,env");
            return;
        }

        for provider_name in providers {
            println!("Testing profile support for provider: {}", provider_name);

            let (provider, _temp_dir) = create_provider_with_temp_path(&provider_name);
            let project_name = generate_test_project_name();

            // Try to set secrets with different profiles
            let profiles = vec!["dev", "staging", "prod"];
            let test_key = "API_KEY";

            for profile in &profiles {
                let value = format!("key_for_{}", profile);
                match provider.set(&project_name, test_key, &value, profile) {
                    Ok(_) => {
                        // Verify we can retrieve with the same profile
                        let result =
                            provider
                                .get(&project_name, test_key, profile)
                                .expect(&format!(
                                    "[{}] Should not error when getting with profile",
                                    provider_name
                                ));

                        match result {
                            Some(retrieved) => {
                                assert_eq!(
                                    retrieved, value,
                                    "[{}] Profile-specific value should match",
                                    provider_name
                                );
                            }
                            None => {
                                panic!(
                                    "[{}] Should find secret for profile {}",
                                    provider_name, profile
                                );
                            }
                        }
                    }
                    Err(_) => {
                        println!("[{}] Provider doesn't support set operation", provider_name);
                        break;
                    }
                }
            }
        }
    }

    #[test]
    fn test_error_handling() {
        let providers = get_test_providers();
        if providers.is_empty() {
            eprintln!("No providers specified. Set SECRETSPEC_TEST_PROVIDERS=keyring,dotenv,env");
            return;
        }

        for provider_name in providers {
            println!("Testing error handling for provider: {}", provider_name);

            let (provider, _temp_dir) = create_provider_with_temp_path(&provider_name);
            let project_name = generate_test_project_name();

            // Test empty key (most providers should handle this gracefully)
            let _ = provider.get(&project_name, "", "default");

            // Test set with empty value (should succeed if provider supports set)
            match provider.set(&project_name, "EMPTY_VALUE", "", "default") {
                Ok(_) => {
                    // Verify empty value can be retrieved
                    let result = provider
                        .get(&project_name, "EMPTY_VALUE", "default")
                        .expect(&format!(
                            "[{}] Should not error when getting empty value",
                            provider_name
                        ));

                    match result {
                        Some(value) => {
                            assert_eq!(
                                value, "",
                                "[{}] Empty value should be preserved",
                                provider_name
                            );
                        }
                        None => {
                            // Some providers might not store empty values
                            println!("[{}] Provider doesn't store empty values", provider_name);
                        }
                    }
                }
                Err(e) => {
                    // Provider doesn't support set, that's ok
                    println!("[{}] Provider doesn't support set: {:?}", provider_name, e);
                }
            }
        }
    }
}
