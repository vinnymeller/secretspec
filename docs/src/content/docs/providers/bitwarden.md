---
title: Bitwarden Provider
description: Bitwarden password manager integration
---

The Bitwarden provider integrates with Bitwarden password manager for secure cloud-based secret storage. It stores secrets as custom fields within Login items, using one item per project/profile combination.

## Prerequisites

- Bitwarden CLI (`bw`)
- Bitwarden account
- Signed in via `bw login`

## Configuration

### URI Format

```bash
# Basic (personal vault)
bitwarden://

# Collection (organization)
bitwarden://SecretSpec
bitwarden://myorg@Production

# Folder (personal vault)
bitwarden://folder/Development

# Self-hosted instance
bitwarden://SecretSpec?server=https://vault.example.com
```

### Authentication

```bash
# One-time setup
bw login

# Daily unlock (get session token)
bw unlock
# Copy the session token and set it as environment variable:
export BW_SESSION="your_session_token_here"

# For CI/CD environments
bw login --apikey
export BW_CLIENTID="your_client_id"
export BW_CLIENTSECRET="your_client_secret"
bw unlock --passwordenv BW_PASSWORD
export BW_SESSION="$(bw unlock --passwordenv BW_PASSWORD --raw)"
```

## Storage Structure

Bitwarden provider stores secrets efficiently by creating one Login item per project/profile combination:

- **Item name**: `{project}/{profile}` (e.g., "myapp/production")
- **Custom fields**: Each secret as a separate field (field name = secret key)
- **URI**: `secretspec://{project}/{profile}` for identification
- **Organization**: Uses collections for project organization

Example item in Bitwarden:
- Name: "myapp/production"
- Custom fields:
  - `DATABASE_URL`: `postgres://...`
  - `API_KEY`: `sk-...`
  - `REDIS_URL`: `redis://...`

## Usage

### Basic Commands

```bash
# Set a secret
secretspec set --provider bitwarden:// DATABASE_URL "postgres://localhost/myapp"

# Get a secret
secretspec get --provider bitwarden:// DATABASE_URL

# Run with secrets
secretspec run --provider bitwarden:// -- npm start
```

### Profile Support

```bash
# Production secrets
secretspec set --provider bitwarden:// --profile production \
    DATABASE_URL "postgres://prod-db/myapp"

# Staging secrets
secretspec set --provider bitwarden:// --profile staging \
    DATABASE_URL "postgres://staging-db/myapp"
```

### Collection Organization

```bash
# Use a specific collection
secretspec set --provider bitwarden://SecretSpec DATABASE_URL "..."

# Organization collection
secretspec set --provider bitwarden://myorg@Production API_KEY "..."
```

## Configuration Files

### User Configuration

```toml
# ~/.config/secretspec/config.toml
[profiles.default]
provider = "bitwarden://SecretSpec"

[profiles.production]
provider = "bitwarden://myorg@Production"
```

### Project Configuration

```toml
# secretspec.toml
[secrets]
DATABASE_URL = "Database connection string"
API_KEY = "Third-party API key"

[profiles.staging]
provider = "bitwarden://SecretSpec"

[profiles.production]
provider = "bitwarden://myorg@Production"
```

## Advanced Features

### Self-hosted Bitwarden

```bash
# Set server URL
export BW_SERVER="https://vault.example.com"
# or
secretspec set --provider "bitwarden://SecretSpec?server=https://vault.example.com" API_KEY "..."
```

### Batch Operations

The provider automatically syncs before read operations and efficiently manages items by grouping secrets within single Bitwarden items.

## Troubleshooting

### Common Issues

**"You are not logged in"**
```bash
bw login
```

**"Vault is locked"**
```bash
bw unlock
export BW_SESSION="your_session_token"
```

**"Bitwarden CLI not found"**
```bash
# Install CLI (see Prerequisites above)
which bw
```

### Debug Mode

```bash
# Test authentication
bw status

# Manual sync
bw sync

# List items (to verify storage)
bw list items --search "myproject/production"
```

## Security Considerations

- Session tokens expire and need periodic renewal
- Store `BW_SESSION` securely in CI/CD environments
- Use organization collections for team secret sharing
- Enable two-factor authentication on your Bitwarden account
- Regularly audit access to organizational collections

## Performance

The Bitwarden provider is optimized for minimal API calls:
- Groups multiple secrets per project/profile in single items
- Syncs only before read operations
- Caches authentication status
- Uses efficient search and update operations
