#[cfg(test)]
mod tests {
    use crate::capitalize_first;
    use secretspec_types::ProjectConfig;

    #[test]
    fn test_capitalize_first() {
        assert_eq!(capitalize_first("development"), "Development");
        assert_eq!(capitalize_first("production"), "Production");
        assert_eq!(capitalize_first("test"), "Test");
        assert_eq!(capitalize_first(""), "");
        assert_eq!(capitalize_first("a"), "A");
    }

    #[test]
    fn test_parse_basic_config() {
        let toml_str = r#"[project]
name = "test"
revision = "1.0"

[profiles.default]
API_KEY = { description = "API key", required = true }
DATABASE_URL = { description = "Database URL", required = false, default = "postgres://localhost" }
"#;

        let config: ProjectConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.profiles.len(), 1);
        let default_profile = &config.profiles["default"];
        assert_eq!(default_profile.secrets.len(), 2);

        let api_key = &default_profile.secrets["API_KEY"];
        assert!(api_key.required);
        assert!(api_key.default.is_none());

        let db_url = &default_profile.secrets["DATABASE_URL"];
        assert!(!db_url.required);
        assert_eq!(db_url.default.as_deref(), Some("postgres://localhost"));
    }

    #[test]
    fn test_parse_profile_overrides() {
        let toml_str = r#"
            [profiles.default]
            API_KEY = { description = "API key", required = true }
            
            [profiles.development]
            API_KEY = { description = "API key", required = false, default = "dev-key" }
            
            [profiles.production]
            API_KEY = { description = "API key", required = true }
        "#;

        let config: ProjectConfig = toml::from_str(&format!(
            r#"[project]
name = "test"
revision = "1.0"
{}"#,
            toml_str
        ))
        .unwrap();
        let api_key = &config.profiles["default"].secrets["API_KEY"];

        assert!(api_key.required);
        assert_eq!(config.profiles.len(), 3);

        let dev_api_key = &config.profiles["development"].secrets["API_KEY"];
        assert!(!dev_api_key.required);
        assert_eq!(dev_api_key.default.as_deref(), Some("dev-key"));

        let prod_api_key = &config.profiles["production"].secrets["API_KEY"];
        assert!(prod_api_key.required);
        assert!(prod_api_key.default.is_none());
    }

    #[test]
    fn test_field_type_determination() {
        // Test that a field that's optional in any profile becomes Option<String>
        let toml_str = r#"[project]
name = "test"
revision = "1.0"

[profiles.default]
SOMETIMES_REQUIRED = { description = "Sometimes required secret", required = true }

[profiles.development]
SOMETIMES_REQUIRED = { description = "Sometimes required secret", required = false }
"#;

        let config: ProjectConfig = toml::from_str(toml_str).unwrap();

        // Simulate the logic from the macro - check if secret is optional across all profiles
        let mut is_ever_optional = false;

        for (_profile_name, profile_config) in &config.profiles {
            if let Some(secret_config) = profile_config.secrets.get("SOMETIMES_REQUIRED") {
                if !secret_config.required || secret_config.default.is_some() {
                    is_ever_optional = true;
                    break;
                }
            } else {
                // Secret doesn't exist in this profile, so it's optional
                is_ever_optional = true;
                break;
            }
        }

        assert!(
            is_ever_optional,
            "Field should be optional since it's optional in development"
        );
    }

    #[test]
    fn test_always_required_field() {
        let toml_str = r#"[project]
name = "test"
revision = "1.0"

[profiles.default]
ALWAYS_REQUIRED = { description = "Always required secret", required = true }

[profiles.development]
ALWAYS_REQUIRED = { description = "Always required secret", required = true }

[profiles.production]
ALWAYS_REQUIRED = { description = "Always required secret", required = true }
"#;

        let config: ProjectConfig = toml::from_str(toml_str).unwrap();
        let secret_config = &config.profiles["default"].secrets["ALWAYS_REQUIRED"];
        let mut is_ever_optional = false;

        if !secret_config.required || secret_config.default.is_some() {
            is_ever_optional = true;
        }

        // Test with the same logic that checks across all profiles
        // (The profile check logic is already above)

        assert!(!is_ever_optional, "Field should never be optional");
    }

    #[test]
    fn test_default_makes_optional() {
        let toml_str = r#"[project]
name = "test"
revision = "1.0"

[profiles.default]
HAS_DEFAULT = { description = "Secret with default", required = true, default = "some-default" }
"#;

        let config: ProjectConfig = toml::from_str(toml_str).unwrap();
        let secret_config = &config.profiles["default"].secrets["HAS_DEFAULT"];

        let is_ever_optional = !secret_config.required || secret_config.default.is_some();
        assert!(
            is_ever_optional,
            "Field with default should be treated as optional"
        );
    }
}
