use crate::provider::Provider;
use crate::{Result, SecretSpecError};
use http::Uri;
use serde::{Deserialize, Serialize};
use std::process::Command;

/// Represents a 1Password item retrieved from the CLI.
///
/// This struct deserializes the JSON output from the `op item get` command
/// and contains an array of fields that hold the actual secret data.
#[derive(Debug, Deserialize)]
struct OnePasswordItem {
    /// Collection of fields within the 1Password item.
    /// Each field represents a piece of data stored in the item.
    fields: Vec<OnePasswordField>,
}

/// Represents a single field within a 1Password item.
///
/// Fields can contain various types of data such as passwords, strings,
/// or concealed values. The field's label is used to identify specific
/// data within an item.
#[derive(Debug, Deserialize)]
struct OnePasswordField {
    /// Unique identifier for the field within the item.
    id: String,
    /// The type of field (e.g., "STRING", "CONCEALED", "PASSWORD").
    #[serde(rename = "type")]
    field_type: String,
    /// Optional human-readable label for the field.
    /// Used to identify fields like "value", "password", etc.
    label: Option<String>,
    /// The actual value stored in the field.
    /// May be None for certain field types.
    value: Option<String>,
}

/// Template for creating new 1Password items via the CLI.
///
/// This struct is serialized to JSON and passed to the `op item create` command
/// using the `--template` flag. It defines the structure and metadata for
/// new secure note items that store secrets.
#[derive(Debug, Serialize)]
struct OnePasswordItemTemplate {
    /// The title of the item, formatted as "secretspec/{project}/{profile}/{key}".
    title: String,
    /// The category of the item. Always "SECURE_NOTE" for secretspec items.
    category: String,
    /// The vault where the item should be created.
    /// If None, 1Password will use the default vault.
    vault: Option<String>,
    /// Collection of fields to include in the item.
    /// Contains project, key, and value fields.
    fields: Vec<OnePasswordFieldTemplate>,
    /// Tags to help organize and identify secretspec items.
    /// Includes "automated" and the project name.
    tags: Vec<String>,
}

/// Template for individual fields when creating 1Password items.
///
/// Each field represents a piece of data to store in the item.
/// Used within OnePasswordItemTemplate to define the item's content.
#[derive(Debug, Serialize)]
struct OnePasswordFieldTemplate {
    /// Human-readable label for the field (e.g., "project", "key", "value").
    label: String,
    /// The type of field. Always "STRING" for secretspec fields.
    #[serde(rename = "type")]
    field_type: String,
    /// The actual value to store in the field.
    value: String,
}

/// Configuration for the 1Password provider.
///
/// This struct contains all the necessary configuration options for
/// interacting with 1Password CLI. It supports both interactive authentication
/// and service account tokens for automated workflows.
///
/// # Examples
///
/// ```
/// // Using default configuration (interactive auth)
/// let config = OnePasswordConfig::default();
///
/// // With a specific vault
/// let config = OnePasswordConfig {
///     default_vault: Some("Development".to_string()),
///     ..Default::default()
/// };
///
/// // With service account token for CI/CD
/// let config = OnePasswordConfig {
///     service_account_token: Some("ops_eyJzaWduSW...".to_string()),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OnePasswordConfig {
    /// Optional account shorthand (for multiple accounts).
    ///
    /// Used with the `--account` flag when you have multiple 1Password
    /// accounts configured. This should match the shorthand shown in
    /// `op account list`.
    pub account: Option<String>,
    /// Default vault to use when profile is "default".
    ///
    /// If not set, defaults to "Private" for the default profile.
    /// For non-default profiles, the profile name is used as the vault name.
    pub default_vault: Option<String>,
    /// Service account token (alternative to interactive auth).
    ///
    /// When set, this token is passed via the OP_SERVICE_ACCOUNT_TOKEN
    /// environment variable to authenticate without user interaction.
    /// Ideal for CI/CD environments.
    pub service_account_token: Option<String>,
}

impl OnePasswordConfig {
    /// Creates a OnePasswordConfig from a URI.
    ///
    /// Supports the following URI formats:
    /// - `1password://` - Basic 1Password with interactive auth
    /// - `1password://vault` - Specify default vault
    /// - `1password://account@vault` - Specify account and vault
    /// - `1password+token://token@vault` - Use service account token
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI to parse
    ///
    /// # Returns
    ///
    /// * `Result<Self>` - The parsed configuration or an error
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The URI scheme is not "1password" or "1password+token"
    /// - The URI uses the incorrect "onepassword" scheme
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

/// Provider implementation for 1Password password manager.
///
/// This provider integrates with 1Password CLI (`op`) to store and retrieve
/// secrets. It organizes secrets in a hierarchical structure within 1Password
/// items using the format: `secretspec/{project}/{profile}/{key}`.
///
/// # Authentication
///
/// The provider supports two authentication methods:
///
/// 1. **Interactive Authentication**: Users run `op signin` before using secretspec
/// 2. **Service Account Tokens**: For CI/CD, configure a token in the config
///
/// # Storage Structure
///
/// Secrets are stored as Secure Note items in 1Password with:
/// - Title: `secretspec/{project}/{profile}/{key}`
/// - Category: SECURE_NOTE
/// - Fields: project, key, value
/// - Tags: "automated", {project}
///
/// # Example Usage
///
/// ```bash
/// # Interactive auth
/// op signin
/// secretspec set MY_SECRET --provider 1password://Development
///
/// # Service account token
/// export OP_SERVICE_ACCOUNT_TOKEN="ops_eyJzaWduSW..."
/// secretspec get MY_SECRET --provider 1password+token://Development
/// ```
pub struct OnePasswordProvider {
    /// Configuration for the provider including auth settings and default vault.
    config: OnePasswordConfig,
}

impl OnePasswordProvider {
    /// Creates a new OnePasswordProvider with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration for the provider
    pub fn new(config: OnePasswordConfig) -> Self {
        Self { config }
    }

    /// Creates a new OnePasswordProvider from a URI.
    ///
    /// This is a convenience method that parses the URI and creates
    /// the appropriate configuration.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI to parse (e.g., "1password://vault")
    ///
    /// # Returns
    ///
    /// * `Result<Self>` - The configured provider or an error
    pub fn from_uri(uri: &Uri) -> Result<Self> {
        let config = OnePasswordConfig::from_uri(uri)?;
        Ok(Self::new(config))
    }

    /// Executes a 1Password CLI command with proper error handling.
    ///
    /// This method handles:
    /// - Setting up authentication (account, service token)
    /// - Executing the command
    /// - Parsing error messages for common issues
    /// - Providing helpful error messages for missing CLI
    ///
    /// # Arguments
    ///
    /// * `args` - The command arguments to pass to `op`
    ///
    /// # Returns
    ///
    /// * `Result<String>` - The command output or an error
    ///
    /// # Errors
    ///
    /// Returns specific errors for:
    /// - Missing 1Password CLI installation
    /// - Authentication required
    /// - Command execution failures
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

    /// Checks if the user is authenticated with 1Password.
    ///
    /// Uses the `op whoami` command to verify authentication status.
    /// This is non-intrusive and doesn't require any permissions.
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - User is authenticated
    /// * `Ok(false)` - User is not authenticated
    /// * `Err(_)` - Command execution failed
    fn whoami(&self) -> Result<bool> {
        match self.execute_op_command(&["whoami"]) {
            Ok(_) => Ok(true),
            Err(SecretSpecError::ProviderOperationFailed(msg))
                if msg.contains("not currently signed in") || msg.contains("no account found") =>
            {
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }

    /// Determines the vault name based on the profile.
    ///
    /// # Arguments
    ///
    /// * `profile` - The profile name
    ///
    /// # Returns
    ///
    /// The vault name to use:
    /// - For "default" profile: uses configured default_vault or "Private"
    /// - For other profiles: uses the profile name as the vault name
    fn get_vault_name(&self, profile: &str) -> String {
        if profile == "default" {
            self.config
                .default_vault
                .clone()
                .unwrap_or_else(|| "Private".to_string())
        } else {
            profile.to_string()
        }
    }

    /// Formats the item name for storage in 1Password.
    ///
    /// Creates a hierarchical name that includes project, profile, and key
    /// to ensure uniqueness and organization.
    ///
    /// # Arguments
    ///
    /// * `project` - The project name
    /// * `key` - The secret key
    /// * `profile` - The profile name
    ///
    /// # Returns
    ///
    /// A formatted string like "secretspec/myproject/production/DATABASE_URL"
    fn format_item_name(&self, project: &str, key: &str, profile: &str) -> String {
        format!("secretspec/{}/{}/{}", project, profile, key)
    }

    /// Creates a template for a new 1Password item.
    ///
    /// This template is serialized to JSON and used with `op item create`.
    /// The item is created as a Secure Note with structured fields.
    ///
    /// # Arguments
    ///
    /// * `project` - The project name
    /// * `key` - The secret key
    /// * `value` - The secret value
    /// * `vault` - The vault to create the item in
    /// * `profile` - The profile name
    ///
    /// # Returns
    ///
    /// A OnePasswordItemTemplate ready for serialization
    fn create_item_template(
        &self,
        project: &str,
        key: &str,
        value: &str,
        vault: &str,
        profile: &str,
    ) -> OnePasswordItemTemplate {
        OnePasswordItemTemplate {
            title: self.format_item_name(project, key, profile),
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
    /// Retrieves a secret from 1Password.
    ///
    /// Searches for an item with the title "secretspec/{project}/{profile}/{key}"
    /// in the appropriate vault. The method looks for a field labeled "value"
    /// first, then falls back to password or concealed fields.
    ///
    /// # Arguments
    ///
    /// * `project` - The project name
    /// * `key` - The secret key to retrieve
    /// * `profile` - The profile to use for vault selection
    ///
    /// # Returns
    ///
    /// * `Ok(Some(value))` - The secret value if found
    /// * `Ok(None)` - No secret found with the given key
    /// * `Err(_)` - Authentication or retrieval error
    ///
    /// # Errors
    ///
    /// - Authentication required if not signed in
    /// - Item retrieval failures
    /// - JSON parsing errors
    fn get(&self, project: &str, key: &str, profile: &str) -> Result<Option<String>> {
        // Check authentication status first
        if !self.whoami()? {
            return Err(SecretSpecError::ProviderOperationFailed(
                "1Password authentication required. Please run 'op signin' first.".to_string(),
            ));
        }

        let vault = self.get_vault_name(profile);
        let item_name = self.format_item_name(project, key, profile);

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

    /// Stores or updates a secret in 1Password.
    ///
    /// If an item with the same title exists, it updates the "value" field.
    /// Otherwise, it creates a new Secure Note item with the secret data.
    ///
    /// # Arguments
    ///
    /// * `project` - The project name
    /// * `key` - The secret key
    /// * `value` - The secret value to store
    /// * `profile` - The profile to use for vault selection
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Secret stored successfully
    /// * `Err(_)` - Storage or authentication error
    ///
    /// # Errors
    ///
    /// - Authentication required if not signed in
    /// - Item creation/update failures
    /// - Temporary file creation errors
    fn set(&self, project: &str, key: &str, value: &str, profile: &str) -> Result<()> {
        // Check authentication status first
        if !self.whoami()? {
            return Err(SecretSpecError::ProviderOperationFailed(
                "1Password authentication required. Please run 'op signin' first.".to_string(),
            ));
        }

        let vault = self.get_vault_name(profile);
        let item_name = self.format_item_name(project, key, profile);

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
            let template = self.create_item_template(project, key, value, &vault, profile);
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

    /// Returns the name of this provider.
    ///
    /// Used for provider identification and selection.
    fn name(&self) -> &'static str {
        "1password"
    }

    /// Returns a human-readable description of this provider.
    ///
    /// Used in help text and provider listings.
    fn description(&self) -> &'static str {
        "1Password password manager"
    }
}

impl Default for OnePasswordProvider {
    /// Creates a OnePasswordProvider with default configuration.
    ///
    /// Uses interactive authentication and the "Private" vault by default.
    fn default() -> Self {
        Self::new(OnePasswordConfig::default())
    }
}
