use crate::provider::Provider;
use crate::{Result, SecretSpecError};
use http::Uri;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastPassConfig {
    /// Default folder prefix
    pub folder_prefix: Option<String>,
}

impl Default for LastPassConfig {
    fn default() -> Self {
        Self {
            folder_prefix: None,
        }
    }
}

impl LastPassConfig {
    pub fn from_uri(uri: &Uri) -> Result<Self> {
        let scheme = uri.scheme_str().ok_or_else(|| {
            SecretSpecError::ProviderOperationFailed("URI must have a scheme".to_string())
        })?;

        if scheme != "lastpass" {
            return Err(SecretSpecError::ProviderOperationFailed(format!(
                "Invalid scheme '{}' for lastpass provider",
                scheme
            )));
        }

        let mut config = Self::default();

        // Parse folder from authority or path, ignoring the dummy localhost
        if let Some(auth) = uri.authority() {
            let auth_str = auth.as_str();
            if auth_str != "localhost" {
                config.folder_prefix = Some(auth_str.to_string());
            }
        }

        if config.folder_prefix.is_none() && !uri.path().is_empty() && uri.path() != "/" {
            config.folder_prefix = Some(uri.path().trim_start_matches('/').to_string());
        }

        Ok(config)
    }
}

pub struct LastPassProvider {
    _config: LastPassConfig,
}

impl LastPassProvider {
    pub fn new(config: LastPassConfig) -> Self {
        Self { _config: config }
    }

    pub fn from_uri(uri: &Uri) -> Result<Self> {
        let config = LastPassConfig::from_uri(uri)?;
        Ok(Self::new(config))
    }

    fn execute_lpass_command(&self, args: &[&str]) -> Result<String> {
        let mut cmd = Command::new("lpass");
        cmd.args(args);

        let output = match cmd.output() {
            Ok(output) => output,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(SecretSpecError::ProviderOperationFailed(
                    "LastPass CLI (lpass) is not installed.\n\nTo install it:\n  - macOS: brew install lastpass-cli\n  - Linux: Check your package manager (apt install lastpass-cli, yum install lastpass-cli, etc.)\n  - NixOS: nix-env -iA nixpkgs.lastpass-cli\n\nAfter installation, run 'lpass login <your-email>' to authenticate.".to_string(),
                ));
            }
            Err(e) => return Err(e.into()),
        };

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            if error_msg.contains("Could not find decryption key")
                || error_msg.contains("Not logged in")
            {
                return Err(SecretSpecError::ProviderOperationFailed(
                    "LastPass authentication required. Please run 'lpass login' first.".to_string(),
                ));
            }
            return Err(SecretSpecError::ProviderOperationFailed(
                error_msg.to_string(),
            ));
        }

        String::from_utf8(output.stdout)
            .map_err(|e| SecretSpecError::ProviderOperationFailed(e.to_string()))
    }

    fn format_item_name(&self, project: &str, key: &str, _profile: Option<&str>) -> String {
        format!("secretspec/{}/{}", project, key)
    }

    fn check_if_logged_in(&self) -> Result<()> {
        // Check if we're logged in first
        if !self.check_login_status()? {
            return Err(SecretSpecError::ProviderOperationFailed(
                "LastPass authentication required. Please run 'lpass login <your-email>' first."
                    .to_string(),
            ));
        }
        Ok(())
    }

    fn check_login_status(&self) -> Result<bool> {
        match self.execute_lpass_command(&["status"]) {
            Ok(output) => Ok(!output.contains("Not logged in")),
            Err(SecretSpecError::ProviderOperationFailed(msg))
                if msg.contains("Not logged in")
                    || msg.contains("LastPass authentication required") =>
            {
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }
}

impl Provider for LastPassProvider {
    fn get(&self, project: &str, key: &str, profile: Option<&str>) -> Result<Option<String>> {
        self.check_if_logged_in()?;

        let item_name = self.format_item_name(project, key, profile);

        match self.execute_lpass_command(&["show", "--sync=now", "--password", &item_name]) {
            Ok(output) => {
                let password = output.trim();
                if password.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(password.to_string()))
                }
            }
            Err(SecretSpecError::ProviderOperationFailed(msg))
                if msg.contains("Could not find specified account") =>
            {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    fn set(&self, project: &str, key: &str, value: &str, profile: Option<&str>) -> Result<()> {
        self.check_if_logged_in()?;

        let item_name = self.format_item_name(project, key, profile);

        // Check if item exists
        if self.get(project, key, profile)?.is_some() {
            // Update existing item
            let args = vec![
                "edit",
                "--sync=now",
                &item_name,
                "--password",
                "--non-interactive",
            ];

            let mut cmd = Command::new("lpass");
            cmd.args(&args);
            cmd.env("LPASS_DISABLE_PINENTRY", "1");

            let mut child = cmd
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;

            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(value.as_bytes())?;
            }

            let output = child.wait_with_output()?;
            if !output.status.success() {
                let error_msg = String::from_utf8_lossy(&output.stderr);
                return Err(SecretSpecError::ProviderOperationFailed(
                    error_msg.to_string(),
                ));
            }
        } else {
            // Create new item using lpass set
            let args = vec![
                "set",
                "--sync=now",
                &item_name,
                "--password",
                "--non-interactive",
            ];

            let mut cmd = Command::new("lpass");
            cmd.args(&args);
            cmd.env("LPASS_DISABLE_PINENTRY", "1");

            let mut child = cmd
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;

            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(value.as_bytes())?;
            }

            let output = child.wait_with_output()?;
            if !output.status.success() {
                let error_msg = String::from_utf8_lossy(&output.stderr);
                return Err(SecretSpecError::ProviderOperationFailed(
                    error_msg.to_string(),
                ));
            }
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "lastpass"
    }

    fn description(&self) -> &'static str {
        "LastPass password manager"
    }
}

impl Default for LastPassProvider {
    fn default() -> Self {
        Self::new(LastPassConfig::default())
    }
}
