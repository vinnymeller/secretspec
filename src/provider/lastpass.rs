use crate::provider::Provider;
use crate::{Result, SecretSpecError};
use std::io::Write;
use std::process::{Command, Stdio};

pub struct LastPassProvider {
    /// Whether to sync before each operation
    sync_on_access: bool,
    /// Default folder prefix
    folder_prefix: Option<String>,
}

impl LastPassProvider {
    pub fn new() -> Self {
        Self {
            sync_on_access: true,
            folder_prefix: None,
        }
    }

    fn execute_lpass_command(&self, args: &[&str]) -> Result<String> {
        let mut cmd = Command::new("lpass");
        cmd.args(args);

        let output = cmd.output()?;

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

    fn get_folder_name(&self, profile: Option<&str>) -> String {
        let base = profile.unwrap_or("default");
        match &self.folder_prefix {
            Some(prefix) => format!("{}/{}", prefix, base),
            None => base.to_string(),
        }
    }

    fn format_item_name(&self, project: &str, key: &str, profile: Option<&str>) -> String {
        let folder = self.get_folder_name(profile);
        format!("{}/{}/{}", folder, project, key)
    }

    fn sync_if_needed(&self) -> Result<()> {
        if self.sync_on_access {
            self.execute_lpass_command(&["sync"])?;
        }
        Ok(())
    }
}

impl Provider for LastPassProvider {
    fn get(&self, project: &str, key: &str, profile: Option<&str>) -> Result<Option<String>> {
        self.sync_if_needed()?;

        let item_name = self.format_item_name(project, key, profile);

        match self.execute_lpass_command(&["show", "--password", &item_name]) {
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
        self.sync_if_needed()?;

        let item_name = self.format_item_name(project, key, profile);

        // Check if item exists
        if self.get(project, key, profile)?.is_some() {
            // Update existing item
            let args = vec!["edit", &item_name, "--password", "--non-interactive"];

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
            // Create new item
            let folder = self.get_folder_name(profile);

            // Create folder if it doesn't exist
            let _ = self.execute_lpass_command(&["mkdir", &folder]);

            let url = format!("project://{}/{}", project, key);
            let username = format!("{}:{}", project, key);

            let note_content = format!(
                "URL: {}\nUsername: {}\nPassword: {}\nNotes: Managed by secrets provider\nProject: {}\nKey: {}",
                url, username, value, project, key
            );

            // Use lpass add with piped input
            let args = vec!["add", &item_name, "--non-interactive", "--note"];

            let mut cmd = Command::new("lpass");
            cmd.args(&args);
            cmd.env("LPASS_DISABLE_PINENTRY", "1");

            let mut child = cmd
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;

            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(note_content.as_bytes())?;
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
}

impl Default for LastPassProvider {
    fn default() -> Self {
        Self::new()
    }
}
