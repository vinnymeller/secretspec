---
title: Adding a New Provider
description: Step-by-step guide for implementing custom provider backends
---

## Provider Trait

```rust
pub trait Provider: Send + Sync {
    fn get(&self, project: &str, key: &str, profile: Option<&str>) -> Result<Option<String>>;
    fn set(&self, project: &str, key: &str, value: &str, profile: Option<&str>) -> Result<()>;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn allows_set(&self) -> bool { true }  // Optional
}
```

## Implementation Steps

1. **Create provider module** in `src/provider/mybackend.rs`
2. **Define config struct** with `Serialize`, `Deserialize`, `Default`, and `from_uri()`
3. **Implement Provider trait** for your provider struct
4. **Export from mod.rs**: Add `pub mod mybackend;` and exports
5. **Register in registry.rs**: Add to `providers()` list and `create_from_string()` match

## Example Implementation

```rust
use super::Provider;
use crate::{Result, SecretSpecError};
use http::Uri;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyBackendConfig {
    pub endpoint: Option<String>,
}

impl Default for MyBackendConfig {
    fn default() -> Self {
        Self { endpoint: None }
    }
}

impl MyBackendConfig {
    pub fn from_uri(uri: &Uri) -> Result<Self> {
        if uri.scheme_str() != Some("mybackend") {
            return Err(SecretSpecError::ProviderOperationFailed(
                "Invalid scheme".to_string()
            ));
        }
        Ok(Self {
            endpoint: uri.host().map(|h| h.to_string()),
        })
    }
}

pub struct MyBackendProvider {
    config: MyBackendConfig,
}

impl MyBackendProvider {
    pub fn new(config: MyBackendConfig) -> Self {
        Self { config }
    }
}

impl Provider for MyBackendProvider {
    fn get(&self, project: &str, key: &str, profile: Option<&str>) -> Result<Option<String>> {
        // Implementation
        Ok(None)
    }

    fn set(&self, project: &str, key: &str, value: &str, profile: Option<&str>) -> Result<()> {
        // Implementation
        Ok(())
    }

    fn name(&self) -> &'static str {
        "mybackend"
    }

    fn description(&self) -> &'static str {
        "My custom backend"
    }
}
```