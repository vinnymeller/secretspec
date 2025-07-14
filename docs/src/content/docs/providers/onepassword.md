---
title: OnePassword Provider
description: OnePassword secrets management integration
---

The OnePassword provider integrates with OnePassword for team-based secret management with advanced access controls.

## Prerequisites

- OnePassword CLI (`op`)
- OnePassword account
- Signed in via `op signin`

## Configuration

### URI Format

```
onepassword://[account@]vault[/path]
onepassword+token://[token@]vault[/path]
```

- `account`: Optional account shorthand
- `vault`: Target vault name (defaults to "Private")
- `token`: Service account token
- `path`: Reserved for future use

### Examples

```bash
# Use specific vault
$ secretspec set API_KEY --provider onepassword://Production

# Use specific account and vault
$ secretspec set DATABASE_URL --provider "onepassword://work@DevVault"

# Use service account token
$ secretspec set SECRET --provider "onepassword+token://ops_token123@Production"

# Default vault (Private)
$ secretspec set KEY --provider onepassword://
```

## Usage

### Basic Commands

```bash
# Set a secret
$ secretspec set DATABASE_URL
Enter value for DATABASE_URL: postgresql://localhost/mydb
✓ Secret DATABASE_URL saved to OnePassword

# Get a secret
$ secretspec get DATABASE_URL

# Run with secrets
$ secretspec run -- npm start
```

### Profile Configuration

```toml
# secretspec.toml
[development]
provider = "onepassword://Development"

[production]
provider = "onepassword://Production"
```

### CI/CD with Service Accounts

```bash
# Set token
$ export OP_SERVICE_ACCOUNT_TOKEN="ops_eyJ..."

# Run command
$ secretspec run --provider onepassword://Production -- deploy
```