use super::Provider;
use crate::Result;
use keyring::Entry;

pub struct KeyringProvider;

impl Provider for KeyringProvider {
    fn get(&self, project: &str, key: &str, profile: Option<&str>) -> Result<Option<String>> {
        let service = format!("secretspec/{}", project);
        let username = if let Some(prof) = profile {
            format!("{}:{}", prof, key)
        } else {
            key.to_string()
        };
        
        let entry = Entry::new(&service, &username)?;
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn set(&self, project: &str, key: &str, value: &str, profile: Option<&str>) -> Result<()> {
        let service = format!("secretspec/{}", project);
        let username = if let Some(prof) = profile {
            format!("{}:{}", prof, key)
        } else {
            key.to_string()
        };
        
        let entry = Entry::new(&service, &username)?;
        entry.set_password(value)?;
        Ok(())
    }
}
