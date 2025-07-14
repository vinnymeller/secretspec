use super::{Provider, ProviderInfo};
use crate::Result;

/// Internal registration structure used by the macro.
#[doc(hidden)]
pub struct ProviderRegistration {
    pub info: ProviderInfo,
    pub schemes: &'static [&'static str],
    pub factory: fn(&url::Url) -> Result<Box<dyn Provider>>,
}

/// Distributed slice that collects all provider registrations.
#[doc(hidden)]
#[linkme::distributed_slice]
pub static PROVIDER_REGISTRY: [ProviderRegistration];
