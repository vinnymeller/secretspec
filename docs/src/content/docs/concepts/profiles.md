---
title: Profiles
description: Managing environment-specific secret requirements with profiles
---

## What Are Profiles?

Profiles are named configurations that define how secrets behave in different environments. They specify which secrets are required vs optional, provide safe defaults for development, and enforce strict requirements for production.

## Basic Usage

Define profiles in your `secretspec.toml`:

```toml
[profiles.default]
DATABASE_URL = { description = "PostgreSQL connection", required = true }
API_KEY = { description = "External API key", required = true }

[profiles.development]
DATABASE_URL = { description = "PostgreSQL connection", required = false, default = "postgresql://localhost:5432/myapp_dev" }
API_KEY = { description = "External API key", required = false, default = "dev-key-12345" }
DEBUG = { description = "Enable debug mode", required = false, default = "true" }

[profiles.production]
DATABASE_URL = { description = "PostgreSQL connection", required = true }
API_KEY = { description = "External API key", required = true }
SENTRY_DSN = { description = "Error tracking", required = true }
```

## Selecting Profiles

SecretSpec resolves the active profile in this order:

1. **Command line**: `--profile production` (highest priority)
2. **Environment variable**: `SECRETSPEC_PROFILE=staging`
3. **User config**: Default profile in `~/.config/secretspec/config.toml`
4. **Fallback**: `default` profile

```bash
# Use specific profile
$ secretspec check --profile development
✓ DATABASE_URL - PostgreSQL connection (using default)
✓ API_KEY - External API key (using default)

# Set via environment
export SECRETSPEC_PROFILE=production
secretspec run -- npm start
```

## Practical Example

A web application with different requirements per environment:

```toml
[project]
name = "web-app"
revision = "1.0"

[profiles.default]
DATABASE_URL = { description = "PostgreSQL connection", required = true }
REDIS_URL = { description = "Redis for caching", required = true }
JWT_SECRET = { description = "JWT signing key", required = true }

[profiles.development]
DATABASE_URL = { required = false, default = "postgresql://localhost:5432/webapp_dev" }
REDIS_URL = { required = false, default = "redis://localhost:6379/0" }
JWT_SECRET = { required = false, default = "dev-secret-change-in-prod" }
HOT_RELOAD = { required = false, default = "true" }

[profiles.production]
DATABASE_URL = { required = true }
REDIS_URL = { required = true }
JWT_SECRET = { required = true }
SENTRY_DSN = { description = "Error tracking", required = true }
SSL_CERT = { description = "SSL certificate path", required = true }
```