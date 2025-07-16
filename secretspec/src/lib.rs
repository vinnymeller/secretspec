//! SecretSpec - A declarative secrets manager for development workflows
//!
//! This library provides a type-safe, declarative way to manage secrets and environment
//! variables across different environments and storage backends.
//!
//! # Features
//!
//! - **Declarative Configuration**: Define secrets in `secretspec.toml`
//! - **Multiple Providers**: Keyring, dotenv, environment variables, OnePassword, LastPass
//! - **Profile Support**: Different configurations for development, staging, production
//! - **Type Safety**: Optional compile-time code generation for strongly-typed access
//! - **Validation**: Ensure all required secrets are present before running applications
//!
//! # Example
//!
//! ```no_run
//! use secretspec::{Secrets, Result};
//!
//! fn main() -> Result<()> {
//!     // Load the secret specification
//!     let spec = Secrets::load()?;
//!
//!     // Validate all secrets are present
//!     spec.check(None, None)?;
//!
//!     // Run a command with secrets injected
//!     spec.run(vec!["npm".to_string(), "start".to_string()], None, None)?;
//!
//!     Ok(())
//! }
//! ```

// Internal modules
mod config;
mod error;
mod secrets;
mod validation;

pub(crate) mod provider;

// CLI module (feature-gated)
#[cfg(feature = "cli")]
pub mod cli;

// Re-export only the types needed by users and generated code
pub use config::Resolved;

// Re-export config types for CLI usage only - these are marked #[doc(hidden)]
#[doc(hidden)]
pub use config::{Config, GlobalConfig, GlobalDefaults, Profile, Project};

// Re-export Secret for secretspec-derive
#[doc(hidden)]
pub use config::Secret;

// Public API exports
pub use error::{Result, SecretSpecError};
pub use secrets::Secrets;
pub use validation::ValidatedSecrets;

#[cfg(test)]
mod tests;
