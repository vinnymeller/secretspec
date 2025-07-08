---
title: 1Password Provider
description: 1Password secrets management integration
---

The 1Password provider integrates with 1Password for team-based secret management with advanced access controls.

## Prerequisites

- 1Password CLI (`op`)
- 1Password account
- Signed in via `op signin`

## Configuration

### URI Format

```
1password://[account@]vault[/path]
1password+token://[token@]vault[/path]
```

- `account`: Optional account shorthand
- `vault`: Target vault name (defaults to "Private")
- `token`: Service account token
- `path`: Reserved for future use

### Examples

```bash
# Use specific vault
$ secretspec set API_KEY --provider 1password://Production

# Use specific account and vault
$ secretspec set DATABASE_URL --provider "1password://work@DevVault"

# Use service account token
$ secretspec set SECRET --provider "1password+token://ops_token123@Production"

# Default vault (Private)
$ secretspec set KEY --provider 1password://
```

## Usage

### Basic Commands

```bash
# Set a secret
$ secretspec set DATABASE_URL
Enter value for DATABASE_URL: postgresql://localhost/mydb
✓ Secret DATABASE_URL saved to 1Password

# Get a secret
$ secretspec get DATABASE_URL

# Run with secrets
$ secretspec run -- npm start
```

### Profile Configuration

```toml
# secretspec.toml
[development]
provider = "1password://Development"

[production]
provider = "1password://Production"
```

### CI/CD with Service Accounts

```bash
# Set token
$ export OP_SERVICE_ACCOUNT_TOKEN="ops_eyJ..."

# Run command
$ secretspec run --provider 1password://Production -- deploy
```