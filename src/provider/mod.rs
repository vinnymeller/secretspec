use crate::Result;

pub mod dotenv;
pub mod env;
pub mod keyring;
pub mod lastpass;
pub mod onepassword;
pub mod registry;

#[cfg(test)]
pub(crate) mod tests;

pub use dotenv::{DotEnvConfig, DotEnvProvider};
pub use env::{EnvConfig, EnvProvider};
pub use keyring::{KeyringConfig, KeyringProvider};
pub use lastpass::{LastPassConfig, LastPassProvider};
pub use onepassword::{OnePasswordConfig, OnePasswordProvider};
pub use registry::{ProviderInfo, ProviderRegistry};

pub trait Provider: Send + Sync {
    fn get(&self, project: &str, key: &str, profile: &str) -> Result<Option<String>>;
    fn set(&self, project: &str, key: &str, value: &str, profile: &str) -> Result<()>;

    /// Returns whether this provider supports setting values.
    /// Defaults to true, but can be overridden by read-only providers.
    fn allows_set(&self) -> bool {
        true
    }

    /// Returns the name of this provider for display purposes
    fn name(&self) -> &'static str;

    /// Returns a brief description of this provider
    fn description(&self) -> &'static str;
}
