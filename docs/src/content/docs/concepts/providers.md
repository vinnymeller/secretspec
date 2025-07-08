---
title: Providers
description: Understanding secret storage providers in SecretSpec
---

Providers are pluggable storage backends that handle the storage and retrieval of secrets. They allow the same `secretspec.toml` to work across development machines, CI/CD pipelines, and production environments.

## Available Providers

| Provider | Description | Read | Write | Encrypted |
|----------|-------------|------|-------|-----------|
| **keyring** | System credential storage (macOS Keychain, Windows Credential Manager, Linux Secret Service) | ✓ | ✓ | ✓ |
| **dotenv** | Traditional `.env` file in your project directory | ✓ | ✓ | ✗ |
| **env** | Read-only access to existing environment variables | ✓ | ✗ | ✗ |
| **1password** | Integration with 1Password password manager | ✓ | ✓ | ✓ |
| **lastpass** | Integration with LastPass password manager | ✓ | ✓ | ✓ |

## Provider Selection

SecretSpec determines which provider to use in this order:

1. **CLI flag**: `--provider` flag (highest priority)
2. **Environment**: `SECRETSPEC_PROVIDER` variable
3. **Project config**: Project-specific setting in user config
4. **Global default**: Default provider in user config
5. **System default**: Falls back to `keyring`

## Configuration

Set your default provider:

```bash
$ secretspec config init
```

Override for specific commands:

```bash
# Use dotenv for this command
$ secretspec run --provider dotenv -- npm start

# Set for shell session
$ export SECRETSPEC_PROVIDER=env
$ secretspec check
```

Configure providers with URIs:

```toml
# ~/.config/secretspec/config.toml
[defaults]
provider = "keyring"

[projects.myapp]
provider = "1password://Personal/Development"

[projects.api]
provider = "dotenv:/home/user/work/.env"
```

## Next Steps

- Learn about specific providers in the [Providers](/providers/keyring/) section
- Understand how providers work with [Profiles](/concepts/profiles/)
- Explore [Configuration Inheritance](/concepts/inheritance/) for complex setups