//! Validation results for secret checking

use crate::config::Resolved;
use std::collections::HashMap;

/// Container for validated secrets with metadata
///
/// This struct contains the validated secrets along with information about
/// which secrets are present, missing, or using default values.
pub struct ValidatedSecrets {
    /// Resolved secrets with provider and profile information
    pub resolved: Resolved<HashMap<String, String>>,
    /// List of required secrets that are missing
    pub missing_required: Vec<String>,
    /// List of optional secrets that are missing
    pub missing_optional: Vec<String>,
    /// List of secrets using their default values (name, default_value)
    pub with_defaults: Vec<(String, String)>,
}

impl ValidatedSecrets {
    /// Checks if the validation result represents a valid state
    ///
    /// A validation result is considered valid if there are no missing required secrets.
    ///
    /// # Returns
    ///
    /// `true` if all required secrets are present, `false` otherwise
    pub fn is_valid(&self) -> bool {
        self.missing_required.is_empty()
    }
}
