use crate::Result;
use std::collections::HashMap;
use std::path::PathBuf;

pub mod dotenv;
pub mod env;
pub mod keyring;

pub use dotenv::DotEnvProvider;
pub use env::EnvProvider;
pub use keyring::KeyringProvider;

pub trait Provider: Send + Sync {
    fn get(&self, project: &str, key: &str, profile: Option<&str>) -> Result<Option<String>>;
    fn set(&self, project: &str, key: &str, value: &str, profile: Option<&str>) -> Result<()>;
    
    /// Returns whether this provider supports setting values.
    /// Defaults to true, but can be overridden by read-only providers.
    fn allows_set(&self) -> bool {
        true
    }
}

pub struct ProviderRegistry {
    backends: HashMap<String, Box<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let mut backends = HashMap::new();
        backends.insert(
            "keyring".to_string(),
            Box::new(KeyringProvider) as Box<dyn Provider>,
        );
        backends.insert(
            "dotenv".to_string(),
            Box::new(DotEnvProvider::new(PathBuf::from(".env"))) as Box<dyn Provider>,
        );
        backends.insert(
            "env".to_string(),
            Box::new(EnvProvider::new()) as Box<dyn Provider>,
        );
        Self { backends }
    }

    pub fn get(&self, name: &str) -> Option<&Box<dyn Provider>> {
        self.backends.get(name)
    }
}
