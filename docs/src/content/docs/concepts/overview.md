---
title: Concepts Overview
description: Understanding the core concepts of SecretSpec
---

SecretSpec is a declarative secrets manager for development workflows that solves the fundamental problem of secret management: how to safely manage environment variables across different environments without hardcoding them or compromising security.

## Core Architecture

SecretSpec follows a modular architecture with three main components:

1. **CLI Tool** (`secretspec`) - Command-line interface for managing secrets
2. **Rust Library** - Core functionality for parsing, validation, and provider abstraction
3. **Provider System** - Pluggable backends for different secret storage solutions

### Design Principles

- **Declaration over Configuration**: Define what secrets you need, not how to get them
- **Provider Agnostic**: Same configuration works with any storage backend
- **Fail Fast**: Validate all secrets before running your application
- **Type Safety**: Optional code generation for compile-time guarantees
- **Profile-based**: Different requirements for different environments

## The Three Pillars

### 1. WHAT - Secret Declarations

The **WHAT** defines which secrets your application needs in `secretspec.toml`:

```toml
[project]
name = "my-app"
revision = "1.0"

[profiles.default]
DATABASE_URL = { description = "PostgreSQL connection string", required = true }
API_KEY = { description = "External API key", required = true }
JWT_SECRET = { description = "Secret for JWT signing", required = true }
```

Each secret declaration includes:
- **Name**: The environment variable name (e.g., `DATABASE_URL`)
- **Description**: Human-readable explanation of the secret's purpose
- **Required**: Whether the secret must be provided
- **Default**: Optional default value if not set

### 2. HOW - Profiles

The **HOW** manages environment-specific requirements through profiles:

```toml
[profiles.development]
DATABASE_URL = { description = "Dev database", required = false, default = "sqlite://./dev.db" }
API_KEY = { description = "Dev API key", required = false, default = "dev-key-12345" }
JWT_SECRET = { description = "Dev JWT secret", required = true }

[profiles.production]
DATABASE_URL = { description = "Production database", required = true }
API_KEY = { description = "Production API key", required = true }
JWT_SECRET = { description = "Production JWT secret", required = true }
```

Profiles enable:
- Environment-specific defaults
- Flexible requirement enforcement
- Configuration inheritance via `extends`

### 3. WHERE - Providers

The **WHERE** abstracts secret storage through a trait-based provider system:

| Provider | Description | Use Case | Allows Set |
|----------|-------------|----------|------------|
| **keyring** | System credential store | Recommended for local development | ✓ |
| **dotenv** | Traditional `.env` files | Legacy compatibility | ✓ |
| **env** | Environment variables | CI/CD, containers | ✗ |
| **1password** | 1Password vaults | Team environments | ✓ |
| **lastpass** | LastPass folders | Personal/team use | ✓ |

Provider flexibility:
```bash
# Development with system keyring
$ secretspec run --provider keyring -- npm start

# CI/CD with environment variables
$ secretspec run --provider env -- npm test

# Production with 1Password
$ secretspec run --provider 1password://Production -- npm start
```

## Key Features

### Validation & Prompting

SecretSpec validates all required secrets before execution and can interactively prompt for missing values:

```bash
$ secretspec check
Checking secrets in my-app using keyring (profile: development)...

✓ DATABASE_URL - Dev database (has default)
✗ JWT_SECRET - Dev JWT secret (required)

The following required secrets are missing:

JWT_SECRET - Dev JWT secret
Enter value for JWT_SECRET (profile: development): ****

✓ Secret 'JWT_SECRET' saved to keyring (profile: development)
```

### Configuration Inheritance

Projects can extend configurations from other directories:

```toml
[project]
name = "api-service"
extends = ["../common", "../auth"]

[profiles.default]
SERVICE_KEY = { description = "Service-specific key", required = true }
```

### Global Configuration

User preferences are stored in a global config file:

```toml
[defaults]
provider = "keyring"
profile = "development"

[projects]
my-app = { provider = "1password://Work" }
```

## How It Solves Secret Management

1. **No More `.env` Files in Repos**: Secrets are stored in secure backends, not in code
2. **Early Validation**: Catch missing secrets before runtime failures
3. **Team Coordination**: Everyone uses the same secret definitions
4. **Environment Portability**: Same app works locally, in CI, and production
5. **Interactive Setup**: Guides new developers through secret configuration

## Technical Implementation

- **Rust-based**: Fast, memory-safe implementation
- **Error Handling**: Comprehensive error types with helpful messages
- **Async Support**: Provider operations can be async
- **Extensible**: Easy to add new providers via the `Provider` trait
- **Cross-platform**: Works on macOS, Linux, and Windows