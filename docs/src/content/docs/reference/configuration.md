---
title: Configuration Reference
description: Complete reference for secretspec.toml configuration
---

SecretSpec uses two configuration files:
- **`secretspec.toml`** - Project-specific secret requirements (checked into version control)
- **`config.toml`** - Global user configuration (stored in user config directory)

## secretspec.toml Format

```toml
[project]
name = "my-app"              # Project name (required)
revision = "1.0"             # Format version (required, must be "1.0")
extends = ["../shared"]      # Optional inheritance

[profiles.default]           # Default profile (required)
DATABASE_URL = { description = "PostgreSQL connection", required = true }
API_KEY = { description = "External API key", required = true }
REDIS_URL = { description = "Redis cache", required = false, default = "redis://localhost:6379" }
```

### Key Fields

| Section | Field | Type | Required | Description |
|---------|-------|------|----------|-------------|
| `[project]` | `name` | string | Yes | Project identifier |
| | `revision` | string | Yes | Must be "1.0" |
| | `extends` | array | No | Paths to parent configs |
| `[profiles.*]` | `description` | string | Yes | Secret purpose |
| | `required` | boolean | Yes* | Is value required? |
| | `default` | string | No** | Fallback value |

*Unless `default` is provided  
**Only when `required = false`

## Global Configuration

Located at:
- **Linux/macOS**: `~/.config/secretspec/config.toml`
- **Windows**: `%APPDATA%\secretspec\config.toml`

```toml
[defaults]
provider = "keyring"              # Default provider
profile = "development"           # Default profile

[projects.my-app]
provider = "1password://vault/Production"
```

### Provider URIs

| Provider | Simple | URI Examples |
|----------|--------|--------------|
| Keyring | `keyring` | `keyring:` |
| 1Password | `1password` | `1password://vault`<br>`1password://vault/Production` |
| Dotenv | `dotenv` | `dotenv:`<br>`dotenv:/path/to/.env` |
| Env | `env` | `env:` |
| LastPass | `lastpass` | `lastpass://folder` |

## Practical Example

```toml
# secretspec.toml
[project]
name = "web-api"
revision = "1.0"

[profiles.default]
APP_NAME = { description = "Application name", required = false, default = "MyApp" }
LOG_LEVEL = { description = "Log verbosity", required = false, default = "info" }

[profiles.development]
DATABASE_URL = { description = "Database", required = false, default = "sqlite://./dev.db" }
API_URL = { description = "API endpoint", required = false, default = "http://localhost:3000" }

[profiles.production]
DATABASE_URL = { description = "PostgreSQL cluster", required = true }
API_URL = { description = "Production API", required = true }
SENTRY_DSN = { description = "Error tracking", required = true }
```

## Configuration Precedence

1. Command-line flags (`--provider`, `--profile`)
2. Environment variables (`SECRETSPEC_PROVIDER`, `SECRETSPEC_PROFILE`)
3. Project config (`[projects.{name}]`)
4. Global defaults (`[defaults]`)
5. Built-in defaults