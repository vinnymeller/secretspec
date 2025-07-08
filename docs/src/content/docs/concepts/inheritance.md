---
title: Configuration Inheritance
description: Building modular secret configurations with inheritance
---

SecretSpec supports configuration inheritance through the `extends` field, allowing you to share common secrets across projects and build layered configurations.

## What is Inheritance?

Inheritance lets you reuse secret definitions from other configuration files. This helps avoid duplication and maintain consistency across projects that share common infrastructure.

## Basic Syntax

Create a shared configuration:

```toml
# shared/common/secretspec.toml
[project]
name = "common"

[profiles.default]
DATABASE_URL = { description = "Main database", required = true }
LOG_LEVEL = { description = "Log verbosity", required = false, default = "info" }
```

Extend it in your project:

```toml
# myapp/secretspec.toml
[project]
name = "myapp"
extends = ["../shared/common"]  # Inherit from common config

[profiles.default]
DATABASE_URL = { description = "MyApp database", required = true }  # Override
API_KEY = { description = "External API key", required = true }     # Add new
```

## Key Rules

1. **Child wins**: When a secret exists in both parent and child, the child's definition completely replaces the parent's
2. **Profile isolation**: Each profile is merged independently - no cross-profile inheritance
3. **Multiple sources**: You can extend from multiple configs using `extends = ["../config1", "../config2"]`
4. **Path resolution**: Paths in `extends` are relative to the file containing them

## Practical Example

Here's a typical monorepo setup:

```
monorepo/
├── shared/
│   ├── base/secretspec.toml      # Common secrets
│   └── database/secretspec.toml  # DB-specific secrets (extends base)
└── services/
    ├── api/secretspec.toml       # extends database config
    └── frontend/secretspec.toml  # extends base config
```

**shared/database/secretspec.toml:**
```toml
[project]
extends = ["../base"]

[profiles.default]
DATABASE_URL = { description = "Primary database", required = true }
DATABASE_POOL_SIZE = { description = "Connection pool", required = false, default = "10" }

[profiles.production]
DATABASE_POOL_SIZE = { description = "Connection pool", required = false, default = "50" }
```

**services/api/secretspec.toml:**
```toml
[project]
extends = ["../../shared/database"]

[profiles.default]
API_PORT = { description = "Server port", required = false, default = "3000" }
JWT_SECRET = { description = "JWT signing key", required = true }
```

Result: The API service inherits all database secrets plus base configuration, while adding its own API-specific secrets.