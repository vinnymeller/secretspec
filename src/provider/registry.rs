use crate::provider::{
    DotEnvConfig, DotEnvProvider, EnvConfig, EnvProvider, KeyringConfig, KeyringProvider,
    LastPassConfig, LastPassProvider, OnePasswordConfig, OnePasswordProvider, Provider,
};
use crate::{Result, SecretSpecError};
use http::Uri;

#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub examples: Vec<&'static str>,
}

impl ProviderInfo {
    pub fn display_with_examples(&self) -> String {
        if self.examples.is_empty() {
            format!("{}: {}", self.name, self.description)
        } else {
            format!(
                "{}: {} (e.g., {})",
                self.name,
                self.description,
                self.examples.join(", ")
            )
        }
    }
}

pub struct ProviderRegistry;

impl ProviderRegistry {
    pub fn providers() -> Vec<ProviderInfo> {
        vec![
            ProviderInfo {
                name: "keyring",
                description: "Uses system keychain (Recommended)",
                examples: vec![],
            },
            ProviderInfo {
                name: "1password",
                description: "1Password password manager",
                examples: vec!["1password://vault", "1password://work@Production"],
            },
            ProviderInfo {
                name: "dotenv",
                description: "Traditional .env files",
                examples: vec!["dotenv:/path/to/.env"],
            },
            ProviderInfo {
                name: "env",
                description: "Read-only environment variables",
                examples: vec![],
            },
            ProviderInfo {
                name: "lastpass",
                description: "LastPass password manager",
                examples: vec!["lastpass://folder"],
            },
        ]
    }

    pub fn get_info(name: &str) -> Option<ProviderInfo> {
        Self::providers().into_iter().find(|p| p.name == name)
    }

    pub fn create_from_string(s: &str) -> Result<Box<dyn Provider>> {
        // Special handling for dotenv with paths
        if s.starts_with("dotenv:") && !s.contains("://") {
            let path = &s[7..]; // Remove "dotenv:" prefix
            let config = if path.is_empty() {
                DotEnvConfig::default()
            } else {
                DotEnvConfig::from_path_string(path)
            };
            return Ok(Box::new(DotEnvProvider::new(config)));
        }

        // Normalize the input to ensure it's a valid URI
        let normalized = if s.contains("://") {
            // Already has scheme separator
            s.to_string()
        } else if s.ends_with(':') {
            // Has colon but no slashes, add dummy authority
            format!("{}//localhost", s)
        } else if s.contains(':') && s.contains('/') {
            // Has colon and slashes but not "://", probably like "dotenv:/path"
            // Insert authority after the colon
            let colon_pos = s.find(':').unwrap();
            format!("{}://localhost{}", &s[..colon_pos], &s[colon_pos + 1..])
        } else {
            // Just a provider name, make it a proper URI
            format!("{}://localhost", s)
        };

        // Parse the normalized URI
        let uri = normalized.parse::<Uri>().map_err(|e| {
            SecretSpecError::ProviderOperationFailed(format!(
                "Invalid provider specification '{}': {}",
                s, e
            ))
        })?;

        let scheme = uri.scheme_str().ok_or_else(|| {
            SecretSpecError::ProviderOperationFailed("URI must have a scheme".to_string())
        })?;

        match scheme {
            "1password" | "1password+token" => {
                let config = OnePasswordConfig::from_uri(&uri)?;
                Ok(Box::new(OnePasswordProvider::new(config)))
            }
            "keyring" => {
                let config = KeyringConfig::from_uri(&uri)?;
                Ok(Box::new(KeyringProvider::new(config)))
            }
            "dotenv" => {
                let config = DotEnvConfig::from_uri(&uri)?;
                Ok(Box::new(DotEnvProvider::new(config)))
            }
            "env" => {
                let config = EnvConfig::from_uri(&uri)?;
                Ok(Box::new(EnvProvider::new(config)))
            }
            "lastpass" => {
                let config = LastPassConfig::from_uri(&uri)?;
                Ok(Box::new(LastPassProvider::new(config)))
            }
            "onepassword" => Err(SecretSpecError::ProviderOperationFailed(
                "Invalid scheme 'onepassword'. Use '1password' instead (e.g., 1password://vault/path)".to_string()
            )),
            _ => {
                // Check if it's a known provider name to give a better error
                if Self::providers().iter().any(|p| p.name == scheme) {
                    Err(SecretSpecError::ProviderOperationFailed(
                        format!("Provider '{}' exists but URI parsing failed", scheme)
                    ))
                } else {
                    Err(SecretSpecError::ProviderNotFound(scheme.to_string()))
                }
            }
        }
    }
}
