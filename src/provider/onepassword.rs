use crate::provider::Provider;
use crate::{Result, SecretSpecError};
use http::Uri;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Deserialize)]
struct OnePasswordItem {
    fields: Vec<OnePasswordField>,
}

#[derive(Debug, Deserialize)]
struct OnePasswordField {
    id: String,
    #[serde(rename = "type")]
    field_type: String,
    label: Option<String>,
    value: Option<String>,
}

#[derive(Debug, Serialize)]
struct OnePasswordItemTemplate {
    title: String,
    category: String,
    vault: Option<String>,
    fields: Vec<OnePasswordFieldTemplate>,
    tags: Vec<String>,
}

#[derive(Debug, Serialize)]
struct OnePasswordFieldTemplate {
    label: String,
    #[serde(rename = "type")]
    field_type: String,
    value: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OnePasswordConfig {
    /// Optional account shorthand (for multiple accounts)
    pub account: Option<String>,
    /// Default vault to use when profile is not specified
    pub default_vault: Option<String>,
    /// Service account token (alternative to interactive auth)
    pub service_account_token: Option<String>,
}

impl OnePasswordConfig {
    pub fn from_uri(uri: &Uri) -> Result<Self> {
        let scheme = uri.scheme_str().ok_or_else(|| {
            SecretSpecError::ProviderOperationFailed("URI must have a scheme".to_string())
        })?;

        match scheme {
            "onepassword" => {
                return Err(SecretSpecError::ProviderOperationFailed(
                    "Invalid scheme 'onepassword'. Use '1password' instead (e.g., 1password://vault/path)".to_string()
                ));
            }
            "1password" | "1password+token" => {}
            _ => {
                return Err(SecretSpecError::ProviderOperationFailed(format!(
                    "Invalid scheme '{}' for 1Password provider",
                    scheme
                )));
            }
        }

        let authority = uri.authority().map(|a| a.as_str());
        let mut config = Self::default();

        // Parse authority for account@vault format, ignoring dummy localhost
        if let Some(auth) = authority {
            if auth != "localhost" {
                if let Some(at_pos) = auth.find('@') {
                    let user_info = &auth[..at_pos];
                    let vault = &auth[at_pos + 1..];

                    // Handle user:token format for service account tokens
                    if scheme == "1password+token" {
                        if let Some(colon_pos) = user_info.find(':') {
                            config.service_account_token =
                                Some(user_info[colon_pos + 1..].to_string());
                        } else {
                            config.service_account_token = Some(user_info.to_string());
                        }
                    } else {
                        config.account = Some(user_info.to_string());
                    }

                    config.default_vault = Some(vault.to_string());
                } else {
                    // No @, so the entire authority is the vault
                    config.default_vault = Some(auth.to_string());
                }
            }
        }

        Ok(config)
    }
}

pub struct OnePasswordProvider {
    config: OnePasswordConfig,
}

impl OnePasswordProvider {
    pub fn new(config: OnePasswordConfig) -> Self {
        Self { config }
    }

    pub fn from_uri(uri: &Uri) -> Result<Self> {
        let config = OnePasswordConfig::from_uri(uri)?;
        Ok(Self::new(config))
    }

    fn execute_op_command(&self, args: &[&str]) -> Result<String> {
        let mut cmd = Command::new("op");

        // Set service account token if provided
        if let Some(token) = &self.config.service_account_token {
            cmd.env("OP_SERVICE_ACCOUNT_TOKEN", token);
        }

        // Add account if specified
        if let Some(account) = &self.config.account {
            cmd.arg("--account").arg(account);
        }

        cmd.args(args);

        let output = match cmd.output() {
            Ok(output) => output,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(SecretSpecError::ProviderOperationFailed(
                    "1Password CLI (op) is not installed.\n\nTo install it:\n  - macOS: brew install 1password-cli\n  - Linux: Download from https://1password.com/downloads/command-line/\n  - Windows: Download from https://1password.com/downloads/command-line/\n  - NixOS: nix-env -iA nixpkgs.onepassword\n\nAfter installation, run 'op signin' to authenticate.".to_string(),
                ));
            }
            Err(e) => return Err(e.into()),
        };

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            if error_msg.contains("not currently signed in") {
                return Err(SecretSpecError::ProviderOperationFailed(
                    "1Password authentication required. Please run 'op signin' first.".to_string(),
                ));
            }
            return Err(SecretSpecError::ProviderOperationFailed(
                error_msg.to_string(),
            ));
        }

        String::from_utf8(output.stdout)
            .map_err(|e| SecretSpecError::ProviderOperationFailed(e.to_string()))
    }

    fn get_vault_name(&self, profile: Option<&str>) -> String {
        profile
            .map(|p| p.to_string())
            .or_else(|| self.config.default_vault.clone())
            .unwrap_or_else(|| "Private".to_string())
    }

    fn format_item_name(&self, project: &str, key: &str) -> String {
        format!("{}/{}", project, key)
    }

    fn create_item_template(
        &self,
        project: &str,
        key: &str,
        value: &str,
        vault: &str,
    ) -> OnePasswordItemTemplate {
        OnePasswordItemTemplate {
            title: self.format_item_name(project, key),
            category: "SECURE_NOTE".to_string(),
            vault: Some(vault.to_string()),
            fields: vec![
                OnePasswordFieldTemplate {
                    label: "project".to_string(),
                    field_type: "STRING".to_string(),
                    value: project.to_string(),
                },
                OnePasswordFieldTemplate {
                    label: "key".to_string(),
                    field_type: "STRING".to_string(),
                    value: key.to_string(),
                },
                OnePasswordFieldTemplate {
                    label: "value".to_string(),
                    field_type: "STRING".to_string(),
                    value: value.to_string(),
                },
            ],
            tags: vec!["automated".to_string(), project.to_string()],
        }
    }
}

impl Provider for OnePasswordProvider {
    fn get(&self, project: &str, key: &str, profile: Option<&str>) -> Result<Option<String>> {
        let vault = self.get_vault_name(profile);
        let item_name = self.format_item_name(project, key);

        // Try to get the item by title
        let args = vec![
            "item", "get", &item_name, "--vault", &vault, "--format", "json",
        ];

        match self.execute_op_command(&args) {
            Ok(output) => {
                let item: OnePasswordItem = serde_json::from_str(&output)?;

                // Look for the "value" field
                for field in &item.fields {
                    if field.label.as_deref() == Some("value") {
                        return Ok(field.value.clone());
                    }
                }

                // Fallback: look for password field or first concealed field
                for field in &item.fields {
                    if field.field_type == "CONCEALED" || field.id == "password" {
                        return Ok(field.value.clone());
                    }
                }

                Ok(None)
            }
            Err(SecretSpecError::ProviderOperationFailed(msg)) if msg.contains("isn't an item") => {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    fn set(&self, project: &str, key: &str, value: &str, profile: Option<&str>) -> Result<()> {
        let vault = self.get_vault_name(profile);
        let item_name = self.format_item_name(project, key);

        // First, try to update existing item
        if let Ok(Some(_)) = self.get(project, key, profile) {
            // Item exists, update it
            let field_assignment = format!("value={}", value);
            let args = vec![
                "item",
                "edit",
                &item_name,
                "--vault",
                &vault,
                &field_assignment,
            ];

            self.execute_op_command(&args)?;
        } else {
            // Item doesn't exist, create it
            let template = self.create_item_template(project, key, value, &vault);
            let template_json = serde_json::to_string(&template)?;

            // Write template to temp file
            use std::io::Write;
            let mut temp_file = tempfile::NamedTempFile::new()?;
            temp_file.write_all(template_json.as_bytes())?;
            temp_file.flush()?;

            let args = vec![
                "item",
                "create",
                "--template",
                temp_file.path().to_str().ok_or_else(|| {
                    SecretSpecError::ProviderOperationFailed(
                        "Invalid UTF-8 in temporary file path".to_string(),
                    )
                })?,
            ];

            self.execute_op_command(&args)?;
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "1password"
    }

    fn description(&self) -> &'static str {
        "1Password password manager"
    }
}

impl Default for OnePasswordProvider {
    fn default() -> Self {
        Self::new(OnePasswordConfig::default())
    }
}
