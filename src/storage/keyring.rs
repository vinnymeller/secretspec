use super::StorageBackend;
use crate::Result;
use keyring::Entry;

pub struct KeyringStorage;

impl StorageBackend for KeyringStorage {
    fn get(&self, project: &str, key: &str) -> Result<Option<String>> {
        let entry = Entry::new(&format!("secretspec/{}", project), key)?;
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn set(&self, project: &str, key: &str, value: &str) -> Result<()> {
        let entry = Entry::new(&format!("secretspec/{}", project), key)?;
        entry.set_password(value)?;
        Ok(())
    }
}
