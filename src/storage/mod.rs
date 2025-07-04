use crate::Result;
use std::collections::HashMap;
use std::path::PathBuf;

pub mod dotenv;
pub mod env;
pub mod keyring;

pub use dotenv::DotEnvStorage;
pub use env::EnvStorage;
pub use keyring::KeyringStorage;

pub trait StorageBackend: Send + Sync {
    fn get(&self, project: &str, key: &str) -> Result<Option<String>>;
    fn set(&self, project: &str, key: &str, value: &str) -> Result<()>;
}

pub struct StorageRegistry {
    backends: HashMap<String, Box<dyn StorageBackend>>,
}

impl StorageRegistry {
    pub fn new() -> Self {
        let mut backends = HashMap::new();
        backends.insert(
            "keyring".to_string(),
            Box::new(KeyringStorage) as Box<dyn StorageBackend>,
        );
        backends.insert(
            "dotenv".to_string(),
            Box::new(DotEnvStorage::new(PathBuf::from(".env"))) as Box<dyn StorageBackend>,
        );
        backends.insert(
            "env".to_string(),
            Box::new(EnvStorage::new()) as Box<dyn StorageBackend>,
        );
        Self { backends }
    }

    pub fn get(&self, name: &str) -> Option<&Box<dyn StorageBackend>> {
        self.backends.get(name)
    }
}
