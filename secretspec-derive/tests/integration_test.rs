// Integration tests that verify the complete macro output

use secretspec_derive::declare_secrets;

mod basic_generation {
    use super::*;

    declare_secrets!("tests/fixtures/basic.toml");

    #[test]
    fn test_struct_fields_exist() {
        // This verifies that the struct has the expected fields
        fn _test_field_types(s: SecretSpec) {
            let _: String = s.api_key;
            let _: String = s.database_url;
            let _: Option<String> = s.optional_secret;
        }
    }
}

mod profile_generation {
    use super::*;

    declare_secrets!("tests/fixtures/profiles.toml");

    #[test]
    fn test_profile_enum_variants() {
        // Verify Profile enum has the expected variants
        let _dev = Profile::Development;
        let _staging = Profile::Staging;
        let _prod = Profile::Production;
    }

    #[test]
    fn test_profile_specific_types() {
        // This verifies the profile-specific enum variants have correct field types
        fn _test_development(profile: SecretSpecProfile) {
            match profile {
                SecretSpecProfile::Development {
                    api_key,
                    database_url,
                    redis_url,
                } => {
                    let _: Option<String> = api_key; // Optional in dev
                    let _: Option<String> = database_url; // Required but has default
                    let _: Option<String> = redis_url; // Optional
                }
                _ => panic!("Expected Development variant"),
            }
        }

        fn _test_production(profile: SecretSpecProfile) {
            match profile {
                SecretSpecProfile::Production {
                    api_key,
                    database_url,
                    redis_url,
                } => {
                    let _: String = api_key; // Required in prod
                    let _: String = database_url; // Required in prod
                    let _: String = redis_url; // Required in prod
                }
                _ => panic!("Expected Production variant"),
            }
        }
    }

    #[test]
    fn test_union_type_fields() {
        // Verify the union struct has Option for fields that are optional in any profile
        fn _test_field_types(s: SecretSpec) {
            let _: Option<String> = s.api_key; // Optional in development
            let _: Option<String> = s.database_url; // Has default in dev, so optional in union type
            let _: Option<String> = s.redis_url; // Optional by default
        }
    }
}

mod complex_generation {
    use super::*;

    declare_secrets!("tests/fixtures/complex.toml");

    #[test]
    fn test_complex_field_types() {
        fn _test_field_types(s: SecretSpec) {
            let _: String = s.always_required;
            let _: Option<String> = s.required_with_default; // Has default
            let _: Option<String> = s.always_optional;
            let _: Option<String> = s.complex_secret; // Optional in dev and test
            let _: Option<String> = s.multi_profile; // Optional in base
        }
    }

    #[test]
    fn test_all_profiles_generated() {
        // Verify all profiles from the TOML are generated
        let _dev = Profile::Development;
        let _staging = Profile::Staging;
        let _prod = Profile::Production;
        let _test = Profile::Test;
    }
}

mod empty_generation {
    use super::*;

    declare_secrets!("tests/fixtures/empty.toml");

    #[test]
    fn test_empty_struct() {
        // Verify the struct is generated even with no secrets
        let _size = std::mem::size_of::<SecretSpec>();

        // The struct should have no fields
        fn _test_no_fields(_s: SecretSpec) {
            // Empty struct
        }
    }
}

mod json_serialization {
    use super::*;
    use serde_json;

    declare_secrets!("tests/fixtures/basic.toml");

    #[test]
    fn test_secret_spec_secrets_json_serialization() {
        use secretspec::Resolved;

        // Create a mock SecretSpec instance
        let spec = SecretSpec {
            api_key: "test_key".to_string(),
            database_url: "postgres://localhost/db".to_string(),
            optional_secret: Some("optional".to_string()),
        };

        let secrets_wrapper = Resolved::new(spec, "dotenv".to_string(), "production".to_string());

        // Test serialization to JSON
        let json = serde_json::to_string(&secrets_wrapper).expect("Failed to serialize Resolved");

        // Verify JSON contains expected fields
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("Failed to parse JSON");
        assert_eq!(parsed["provider"], "dotenv");
        assert_eq!(parsed["profile"], "production");
        assert_eq!(parsed["secrets"]["api_key"], "test_key");

        // Test round-trip deserialization
        let deserialized: Resolved<SecretSpec> =
            serde_json::from_str(&json).expect("Failed to deserialize Resolved");
        assert_eq!(deserialized.provider, "dotenv");
        assert_eq!(deserialized.profile, "production");
        assert_eq!(deserialized.secrets.api_key, "test_key");
    }
}
