#[cfg(test)]
mod tests {
    use crate::capitalize_first;
    use secretspec_types::{ProfileOverride, ProjectConfig, SecretConfig};
    use std::collections::HashMap;

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
        let toml_str = r#"
            [secrets.API_KEY]
            required = true
            
            [secrets.DATABASE_URL]
            required = false
            default = "postgres://localhost"
        "#;

        let config: ProjectConfig = toml::from_str(&format!(
            r#"[project]
name = "test"
{}"#,
            toml_str
        ))
        .unwrap();
        assert_eq!(config.secrets.len(), 2);

        let api_key = &config.secrets["API_KEY"];
        assert!(api_key.required);
        assert!(api_key.default.is_none());

        let db_url = &config.secrets["DATABASE_URL"];
        assert!(!db_url.required);
        assert_eq!(db_url.default.as_deref(), Some("postgres://localhost"));
    }

    #[test]
    fn test_parse_profile_overrides() {
        let toml_str = r#"
            [secrets.API_KEY]
            required = true
            
            [secrets.API_KEY.development]
            required = false
            default = "dev-key"
            
            [secrets.API_KEY.production]
            required = true
        "#;

        let config: ProjectConfig = toml::from_str(&format!(
            r#"[project]
name = "test"
{}"#,
            toml_str
        ))
        .unwrap();
        let api_key = &config.secrets["API_KEY"];

        assert!(api_key.required);
        assert_eq!(api_key.profiles.len(), 2);

        let dev_override = &api_key.profiles["development"];
        assert_eq!(dev_override.required, Some(false));
        assert_eq!(dev_override.default.as_deref(), Some("dev-key"));

        let prod_override = &api_key.profiles["production"];
        assert_eq!(prod_override.required, Some(true));
        assert!(prod_override.default.is_none());
    }

    #[test]
    fn test_field_type_determination() {
        // Test that a field that's optional in any profile becomes Option<String>
        let toml_str = r#"
            [secrets.SOMETIMES_REQUIRED]
            required = true
            
            [secrets.SOMETIMES_REQUIRED.development]
            required = false
        "#;

        let config: ProjectConfig = toml::from_str(&format!(
            r#"[project]
name = "test"
{}"#,
            toml_str
        ))
        .unwrap();

        // Simulate the logic from the macro
        let secret_config = &config.secrets["SOMETIMES_REQUIRED"];
        let mut is_ever_optional = false;

        if !secret_config.required || secret_config.default.is_some() {
            is_ever_optional = true;
        }

        for (_profile_name, profile_override) in &secret_config.profiles {
            let profile_required = profile_override.required.unwrap_or(secret_config.required);
            let has_default = profile_override.default.is_some() || secret_config.default.is_some();

            if !profile_required || has_default {
                is_ever_optional = true;
            }
        }

        assert!(
            is_ever_optional,
            "Field should be optional since it's optional in development"
        );
    }

    #[test]
    fn test_always_required_field() {
        let toml_str = r#"
            [secrets.ALWAYS_REQUIRED]
            required = true
            
            [secrets.ALWAYS_REQUIRED.development]
            required = true
            
            [secrets.ALWAYS_REQUIRED.production]
            required = true
        "#;

        let config: ProjectConfig = toml::from_str(&format!(
            r#"[project]
name = "test"
{}"#,
            toml_str
        ))
        .unwrap();
        let secret_config = &config.secrets["ALWAYS_REQUIRED"];
        let mut is_ever_optional = false;

        if !secret_config.required || secret_config.default.is_some() {
            is_ever_optional = true;
        }

        for (_profile_name, profile_override) in &secret_config.profiles {
            let profile_required = profile_override.required.unwrap_or(secret_config.required);
            let has_default = profile_override.default.is_some() || secret_config.default.is_some();

            if !profile_required || has_default {
                is_ever_optional = true;
            }
        }

        assert!(!is_ever_optional, "Field should never be optional");
    }

    #[test]
    fn test_default_makes_optional() {
        let toml_str = r#"
            [secrets.HAS_DEFAULT]
            required = true
            default = "some-default"
        "#;

        let config: ProjectConfig = toml::from_str(&format!(
            r#"[project]
name = "test"
{}"#,
            toml_str
        ))
        .unwrap();
        let secret_config = &config.secrets["HAS_DEFAULT"];

        let is_ever_optional = !secret_config.required || secret_config.default.is_some();
        assert!(
            is_ever_optional,
            "Field with default should be treated as optional"
        );
    }
}
