use crate::provider::Provider;
use crate::{Result, SecretSpecError};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::{Command, Stdio};
use url::Url;

/// Represents a Bitwarden item retrieved from the CLI.
#[derive(Debug, Deserialize)]
struct BitwardenItem {
    /// Unique identifier for the item.
    id: String,
    /// The name/title of the item.
    name: String,
    /// Login-specific data (for Login items).
    login: Option<BitwardenLogin>,
    /// Custom fields containing the actual secret data.
    fields: Option<Vec<BitwardenField>>,
}

/// Login data within a Bitwarden item.
#[derive(Debug, Deserialize)]
struct BitwardenLogin {
    /// Collection of URIs associated with the login.
    uris: Option<Vec<BitwardenUri>>,
}

/// URI data within a Bitwarden login.
#[derive(Debug, Deserialize)]
struct BitwardenUri {
    uri: Option<String>,
}

/// Represents a custom field within a Bitwarden item.
#[derive(Debug, Deserialize, Serialize)]
struct BitwardenField {
    /// The name/label of the field.
    name: String,
    /// The value stored in the field.
    value: String,
    /// The type of field (0 = Text, 1 = Hidden, 2 = Boolean).
    #[serde(rename = "type")]
    field_type: u8,
}

/// Configuration for the Bitwarden provider.
///
/// This struct contains the configuration options for interacting with Bitwarden
/// through the `bw` CLI tool.
///
/// # Examples
///
/// ```ignore
/// use secretspec::provider::bitwarden::BitwardenConfig;
///
/// // Create a default configuration
/// let config = BitwardenConfig::default();
///
/// // Create a configuration with a collection
/// let config = BitwardenConfig {
///     collection_name: Some("SecretSpec".to_string()),
///     ..Default::default()
/// };
///
/// // Create a configuration with a folder
/// let config = BitwardenConfig {
///     folder_name: Some("Development".to_string()),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BitwardenConfig {
    /// Optional organization ID for organizational vaults.
    pub organization_id: Option<String>,
    /// Optional collection name for organizing secrets within an organization.
    pub collection_name: Option<String>,
    /// Optional folder name for organizing secrets within personal vaults.
    pub folder_name: Option<String>,
    /// Optional server URL for self-hosted Bitwarden instances.
    pub server_url: Option<String>,
}

impl TryFrom<&Url> for BitwardenConfig {
    type Error = SecretSpecError;

    /// Creates a BitwardenConfig from a URL.
    ///
    /// Parses URLs in the following formats:
    /// - `bitwarden://` - Default personal vault
    /// - `bitwarden://collection-name` - Specific collection
    ///
    /// # Arguments
    ///
    /// * `url` - A URL with the `bitwarden` scheme
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the parsed configuration or an error
    /// if the URL scheme is not `bitwarden`.
    fn try_from(url: &Url) -> std::result::Result<Self, Self::Error> {
        if url.scheme() != "bitwarden" {
            return Err(SecretSpecError::ProviderOperationFailed(format!(
                "Invalid scheme '{}' for bitwarden provider",
                url.scheme()
            )));
        }

        let mut config = Self::default();

        // Parse host for collection name only (no folders or organizations supported)
        // TODO: uri format just isn't well thought out, & I don't have an organization to test with
        if let Some(host) = url.host_str() {
            config.collection_name = Some(host.to_string());
        }

        // Parse query parameters for server URL
        if let Some(server) = url
            .query_pairs()
            .find(|(key, _)| key == "server")
            .map(|(_, value)| value.to_string())
        {
            config.server_url = Some(server);
        }

        Ok(config)
    }
}

/// Bitwarden provider implementation for SecretSpec.
///
/// This provider integrates with Bitwarden password manager through the `bw` CLI tool.
/// It stores secrets as custom fields within Login items, using one item per project/profile
/// combination for efficient organization and minimal API calls.
///
/// # Requirements
///
/// The Bitwarden CLI (`bw`) must be installed and authenticated:
/// - Install: Download from https://bitwarden.com/help/article/cli/
/// - macOS: `brew install bitwarden-cli`
/// - Linux: Download binary or use package manager
/// - NixOS: `nix-env -iA nixpkgs.bitwarden-cli`
///
/// After installation:
/// 1. `bw login` (one-time setup)
/// 2. `bw unlock` to get session token
/// 3. Set `BW_SESSION` environment variable with the session token
///
/// # Storage Structure
///
/// Each project/profile combination creates a Login item with:
/// - Name: "{project}/{profile}" (e.g., "myapp/production")
/// - Custom fields: One field per secret (field name = key, field value = secret)
/// - URI: "secretspec://{project}/{profile}" for identification
///
/// # Examples
///
/// ```ignore
/// use secretspec::provider::bitwarden::{BitwardenProvider, BitwardenConfig};
///
/// let provider = BitwardenProvider::default();
///
/// let config = BitwardenConfig {
///     collection_name: Some("SecretSpec".to_string()),
///     ..Default::default()
/// };
/// let provider = BitwardenProvider::new(config);
/// ```
pub struct BitwardenProvider {
    config: BitwardenConfig,
}

crate::register_provider! {
    struct: BitwardenProvider,
    config: BitwardenConfig,
    name: "bitwarden",
    description: "Bitwarden password manager",
    schemes: ["bitwarden"],
    examples: [
        "bitwarden://",
        "bitwarden://SecretSpec"
    ],
}

impl BitwardenProvider {
    pub fn new(config: BitwardenConfig) -> Self {
        Self { config }
    }

    /// Executes a Bitwarden CLI command and returns its output.
    fn execute_bw_command(&self, args: &[&str]) -> Result<String> {
        let mut cmd = Command::new("bw");
        cmd.args(args);

        // Set server URL if configured
        if let Some(server_url) = &self.config.server_url {
            cmd.env("BW_SERVER", server_url);
        }

        let output = match cmd.output() {
            Ok(output) => output,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(SecretSpecError::ProviderOperationFailed(
                    "Bitwarden CLI (bw) is not installed.\n\nTo install it:\n  - Download from: https://bitwarden.com/help/article/cli/\n  - macOS: brew install bitwarden-cli\n  - Linux: Download the binary or check your package manager\n  - NixOS: nix-env -iA nixpkgs.bitwarden-cli\n\nAfter installation:\n1. Run 'bw login' to authenticate\n2. Run 'bw unlock' to get a session token\n3. Set the BW_SESSION environment variable with the token".to_string(),
                ));
            }
            Err(e) => return Err(e.into()),
        };

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            if error_msg.contains("You are not logged in") {
                return Err(SecretSpecError::ProviderOperationFailed(
                    "Bitwarden authentication required.\n\n1. Run 'bw login' to authenticate\n2. Run 'bw unlock' to get a session token\n3. Set the BW_SESSION environment variable with the token".to_string(),
                ));
            }
            if error_msg.contains("Invalid master password")
                || error_msg.contains("Vault is locked")
            {
                return Err(SecretSpecError::ProviderOperationFailed(
                    "Bitwarden vault is locked. Run 'bw unlock' and set the BW_SESSION environment variable.".to_string(),
                ));
            }
            return Err(SecretSpecError::ProviderOperationFailed(
                error_msg.to_string(),
            ));
        }

        String::from_utf8(output.stdout)
            .map_err(|e| SecretSpecError::ProviderOperationFailed(e.to_string()))
    }

    /// Formats the item name for storage in Bitwarden.
    fn format_item_name(&self, project: &str, profile: &str) -> String {
        format!("{project}/{profile}")
    }

    /// Formats the item URI for identification.
    fn format_item_uri(&self, project: &str, profile: &str) -> String {
        format!("secretspec://{project}/{profile}")
    }

    /// Verifies that the user is authenticated and vault is unlocked.
    fn check_authentication(&self) -> Result<()> {
        let status_output = self.execute_bw_command(&["status"])?;

        if status_output.contains("\"status\":\"unauthenticated\"") {
            return Err(SecretSpecError::ProviderOperationFailed(
                "Bitwarden authentication required. Run 'bw login' first.".to_string(),
            ));
        }

        if status_output.contains("\"status\":\"locked\"") {
            return Err(SecretSpecError::ProviderOperationFailed(
                "Bitwarden vault is locked. Run 'bw unlock' and set the BW_SESSION environment variable.".to_string(),
            ));
        }

        Ok(())
    }

    /// Syncs the vault to ensure fresh data.
    ///
    /// Performs a `bw sync` operation to fetch the latest data from the Bitwarden
    /// servers. This ensures that searches and operations work with up-to-date
    /// information, which is important when multiple clients are modifying the vault.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Sync completed successfully
    /// * `Err(_)` - Sync failed (network issues, authentication problems, etc.)
    fn sync_vault(&self) -> Result<()> {
        self.execute_bw_command(&["sync"])?;
        Ok(())
    }

    /// Gets the collection ID for the configured collection name.
    ///
    /// This method searches for an existing collection with the specified name
    /// in the user's personal vault. Unlike folders, collections cannot be
    /// auto-created through the CLI, so the collection must already exist.
    ///
    /// # Returns
    ///
    /// * `Ok(Some(collection_id))` - Found collection successfully
    /// * `Ok(None)` - No collection name configured
    /// * `Err(_)` - Collection not found or failed to list collections
    ///
    /// # Errors
    ///
    /// Returns errors for:
    /// - Collection not found (user must create it first)
    /// - Failed to list collections
    /// - Invalid JSON responses from Bitwarden CLI
    fn get_collection_id(&self) -> Result<Option<String>> {
        if let Some(collection_name) = &self.config.collection_name {
            let collections_output = self.execute_bw_command(&["list", "collections"])?;

            if collections_output.trim().is_empty() || collections_output.trim() == "[]" {
                return Err(SecretSpecError::ProviderOperationFailed(format!(
                    "Collection '{collection_name}' not found. No collections exist in your vault."
                )));
            }

            let collections: Vec<serde_json::Value> = serde_json::from_str(&collections_output)
                .map_err(|e| {
                    SecretSpecError::ProviderOperationFailed(format!(
                        "Failed to parse collections response: {e}"
                    ))
                })?;

            for collection in collections {
                if let Some(name) = collection["name"].as_str() {
                    if name == collection_name {
                        if let Some(id) = collection["id"].as_str() {
                            return Ok(Some(id.to_string()));
                        }
                    }
                }
            }

            return Err(SecretSpecError::ProviderOperationFailed(format!(
                "Collection '{collection_name}' not found in your Bitwarden vault.\n\nTo create it:\n  - Web: bitwarden.com → Organizations → Collections → New Collection\n  - Mobile: Bitwarden app → Organizations → Collections → Add\n  - Note: Collections are part of organization vaults, not personal vaults"
            )));
        }
        Ok(None)
    }

    /// Finds an existing item by project and profile.
    fn find_item(&self, project: &str, profile: &str) -> Result<Option<BitwardenItem>> {
        self.sync_vault()?;

        let item_name = self.format_item_name(project, profile);

        // Build search command with optional collection filtering
        let mut args = vec!["list", "items", "--search", &item_name];
        let collection_id = self.get_collection_id()?;

        if let Some(collection_id) = &collection_id {
            args.push("--collectionid");
            args.push(collection_id);
        }

        let search_result = self.execute_bw_command(&args)?;

        if search_result.trim().is_empty() || search_result.trim() == "[]" {
            return Ok(None);
        }

        let items: Vec<BitwardenItem> = serde_json::from_str(&search_result).map_err(|e| {
            SecretSpecError::ProviderOperationFailed(format!(
                "Failed to parse Bitwarden response: {e}"
            ))
        })?;

        // Find exact match by name and URI
        let target_uri = self.format_item_uri(project, profile);
        for item in items {
            if item.name == item_name {
                // Check if this item has our URI (for exact identification)
                if let Some(login) = &item.login {
                    if let Some(uris) = &login.uris {
                        if uris
                            .iter()
                            .any(|uri| uri.uri.as_deref() == Some(&target_uri))
                        {
                            return Ok(Some(item));
                        }
                    }
                }
                // If no URI match but name matches, it might be our item
                return Ok(Some(item));
            }
        }

        Ok(None)
    }
}

impl Provider for BitwardenProvider {
    fn name(&self) -> &'static str {
        Self::PROVIDER_NAME
    }

    fn get(&self, project: &str, key: &str, profile: &str) -> Result<Option<String>> {
        self.check_authentication()?;

        match self.find_item(project, profile)? {
            Some(item) => {
                if let Some(fields) = item.fields {
                    for field in fields {
                        if field.name == key {
                            return Ok(Some(field.value));
                        }
                    }
                }
                Ok(None)
            }
            None => Ok(None),
        }
    }

    fn set(&self, project: &str, key: &str, value: &str, profile: &str) -> Result<()> {
        self.check_authentication()?;

        match self.find_item(project, profile)? {
            Some(item) => {
                // Update existing item
                let mut fields = item.fields.unwrap_or_default();

                // Update existing field or add new one
                let mut found = false;
                for field in &mut fields {
                    if field.name == key {
                        field.value = value.to_string();
                        found = true;
                        break;
                    }
                }

                if !found {
                    fields.push(BitwardenField {
                        name: key.to_string(),
                        value: value.to_string(),
                        field_type: 0, // Text field
                    });
                }

                // Get the current item details
                let item_json = self.execute_bw_command(&["get", "item", &item.id])?;
                let mut item_data: serde_json::Value =
                    serde_json::from_str(&item_json).map_err(|e| {
                        SecretSpecError::ProviderOperationFailed(format!(
                            "Failed to parse current item data: {e}"
                        ))
                    })?;

                // Update the fields in the item data
                item_data["fields"] = serde_json::to_value(&fields).map_err(|e| {
                    SecretSpecError::ProviderOperationFailed(format!(
                        "Failed to serialize fields: {e}"
                    ))
                })?;

                let update_json = serde_json::to_string(&item_data).map_err(|e| {
                    SecretSpecError::ProviderOperationFailed(format!(
                        "Failed to serialize update data: {e}"
                    ))
                })?;

                // Encode the JSON using bw encode
                let mut encode_cmd = Command::new("bw");
                encode_cmd.args(["encode"]);

                if let Some(server_url) = &self.config.server_url {
                    encode_cmd.env("BW_SERVER", server_url);
                }

                let mut encode_child = encode_cmd
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()?;

                if let Some(stdin) = encode_child.stdin.as_mut() {
                    stdin.write_all(update_json.as_bytes())?;
                }

                let encode_output = encode_child.wait_with_output()?;
                if !encode_output.status.success() {
                    let error_msg = String::from_utf8_lossy(&encode_output.stderr);
                    return Err(SecretSpecError::ProviderOperationFailed(format!(
                        "Failed to encode update data: {error_msg}"
                    )));
                }

                let encoded_data = String::from_utf8(encode_output.stdout).map_err(|e| {
                    SecretSpecError::ProviderOperationFailed(format!(
                        "Failed to read encoded data: {e}"
                    ))
                })?;

                // Use bw edit item with encoded data
                let mut edit_cmd = Command::new("bw");
                edit_cmd.args(["edit", "item", &item.id]);

                if let Some(server_url) = &self.config.server_url {
                    edit_cmd.env("BW_SERVER", server_url);
                }

                let mut edit_child = edit_cmd
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()?;

                if let Some(stdin) = edit_child.stdin.as_mut() {
                    stdin.write_all(encoded_data.trim().as_bytes())?;
                }

                let edit_output = edit_child.wait_with_output()?;
                if !edit_output.status.success() {
                    let error_msg = String::from_utf8_lossy(&edit_output.stderr);
                    return Err(SecretSpecError::ProviderOperationFailed(format!(
                        "Failed to update Bitwarden item: {error_msg}"
                    )));
                }
            }
            None => {
                // Create new item using the proper workflow: get template -> modify -> encode -> create

                // Step 1: Get the item template
                let template_output = self.execute_bw_command(&["get", "template", "item"])?;
                let mut template: serde_json::Value = serde_json::from_str(&template_output)
                    .map_err(|e| {
                        SecretSpecError::ProviderOperationFailed(format!(
                            "Failed to parse Bitwarden item template: {e}"
                        ))
                    })?;

                // Step 2: Modify the template with our data
                template["name"] =
                    serde_json::Value::String(self.format_item_name(project, profile));
                template["notes"] = serde_json::Value::String(format!(
                    "SecretSpec item for project '{project}', profile '{profile}'"
                ));
                template["type"] = serde_json::Value::Number(serde_json::Number::from(1u8)); // Login item

                // Set up login object with URI
                if template["login"].is_null() {
                    template["login"] = serde_json::json!({});
                }
                template["login"]["uris"] = serde_json::json!([{
                    "uri": self.format_item_uri(project, profile),
                    "match": null
                }]);
                template["login"]["username"] = serde_json::Value::Null;
                template["login"]["password"] = serde_json::Value::Null;

                // Set up custom fields
                template["fields"] = serde_json::json!([{
                    "name": key,
                    "value": value,
                    "type": 0
                }]);

                // Add collection assignment if configured
                let collection_id = self.get_collection_id()?;
                if let Some(collection_id) = collection_id {
                    template["collectionIds"] = serde_json::json!([collection_id]);
                }

                let create_json = serde_json::to_string(&template).map_err(|e| {
                    SecretSpecError::ProviderOperationFailed(format!(
                        "Failed to serialize item template: {e}",
                    ))
                })?;

                // Step 3: Encode the JSON using bw encode
                let mut encode_cmd = Command::new("bw");
                encode_cmd.args(["encode"]);

                if let Some(server_url) = &self.config.server_url {
                    encode_cmd.env("BW_SERVER", server_url);
                }

                let mut encode_child = encode_cmd
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()?;

                if let Some(stdin) = encode_child.stdin.as_mut() {
                    stdin.write_all(create_json.as_bytes())?;
                }

                let encode_output = encode_child.wait_with_output()?;
                if !encode_output.status.success() {
                    let error_msg = String::from_utf8_lossy(&encode_output.stderr);
                    return Err(SecretSpecError::ProviderOperationFailed(format!(
                        "Failed to encode item data: {error_msg}"
                    )));
                }

                let encoded_data = String::from_utf8(encode_output.stdout).map_err(|e| {
                    SecretSpecError::ProviderOperationFailed(format!(
                        "Failed to read encoded data: {e}"
                    ))
                })?;

                // Step 4: Create the item using the encoded data
                let mut create_cmd = Command::new("bw");
                create_cmd.args(["create", "item"]);

                if let Some(server_url) = &self.config.server_url {
                    create_cmd.env("BW_SERVER", server_url);
                }

                let mut create_child = create_cmd
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()?;

                if let Some(stdin) = create_child.stdin.as_mut() {
                    stdin.write_all(encoded_data.trim().as_bytes())?;
                }

                let create_output = create_child.wait_with_output()?;
                if !create_output.status.success() {
                    let error_msg = String::from_utf8_lossy(&create_output.stderr);
                    return Err(SecretSpecError::ProviderOperationFailed(format!(
                        "Failed to create Bitwarden item: {error_msg}"
                    )));
                }
            }
        }

        Ok(())
    }
}

impl Default for BitwardenProvider {
    fn default() -> Self {
        Self::new(BitwardenConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn test_default_config() {
        let config = BitwardenConfig::default();
        assert!(config.organization_id.is_none());
        assert!(config.collection_name.is_none());
        assert!(config.folder_name.is_none());
        assert!(config.server_url.is_none());
    }

    #[test]
    fn test_uri_parsing_basic() {
        let url = Url::parse("bitwarden://").unwrap();
        let config: BitwardenConfig = (&url).try_into().unwrap();

        assert!(config.organization_id.is_none());
        assert!(config.collection_name.is_none());
        assert!(config.folder_name.is_none());
        assert!(config.server_url.is_none());
    }

    #[test]
    fn test_uri_parsing_collection() {
        let url = Url::parse("bitwarden://SecretSpec").unwrap();
        let config: BitwardenConfig = (&url).try_into().unwrap();

        assert!(config.organization_id.is_none());
        assert_eq!(config.collection_name, Some("SecretSpec".to_string()));
        assert!(config.folder_name.is_none());
        assert!(config.server_url.is_none());
    }

    #[test]
    fn test_uri_parsing_with_server() {
        let url = Url::parse("bitwarden://SecretSpec?server=https://vault.example.com").unwrap();
        let config: BitwardenConfig = (&url).try_into().unwrap();

        assert!(config.organization_id.is_none());
        assert_eq!(config.collection_name, Some("SecretSpec".to_string()));
        assert!(config.folder_name.is_none());
        assert_eq!(
            config.server_url,
            Some("https://vault.example.com".to_string())
        );
    }

    #[test]
    fn test_uri_parsing_invalid_scheme() {
        let url = Url::parse("invalid://test").unwrap();
        let result: Result<BitwardenConfig> = (&url).try_into();

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid scheme 'invalid'")
        );
    }

    #[test]
    fn test_format_item_name() {
        let provider = BitwardenProvider::default();
        let name = provider.format_item_name("myapp", "production");
        assert_eq!(name, "myapp/production");
    }

    #[test]
    fn test_format_item_uri() {
        let provider = BitwardenProvider::default();
        let uri = provider.format_item_uri("myapp", "production");
        assert_eq!(uri, "secretspec://myapp/production");
    }

    #[test]
    fn test_provider_name() {
        let provider = BitwardenProvider::default();
        assert_eq!(provider.name(), "bitwarden");
    }
}
