---
title: Keyring Provider
description: Secure system credential store integration
---

The Keyring provider stores secrets in your system's native credential store. Recommended for local development.

## Supported Platforms

- **macOS**: Keychain
- **Windows**: Credential Manager
- **Linux**: Secret Service (GNOME Keyring, KWallet)

## Installation

Linux only - install if missing:
```bash
# Debian/Ubuntu
$ sudo apt-get install gnome-keyring

# Fedora
$ sudo dnf install gnome-keyring

# Arch
$ sudo pacman -S gnome-keyring
```

## Configuration

```toml
# secretspec.toml
[project]
name = "myapp"

[[providers]]
type = "keyring"
uri = "keyring://"
```

## Usage

```bash
# Set a secret
$ secretspec set DATABASE_URL
Enter value for DATABASE_URL: postgresql://localhost/mydb
✓ Secret DATABASE_URL saved to keyring

# Get a secret
$ secretspec get DATABASE_URL
postgresql://localhost/mydb

# Run with secrets
$ secretspec run -- npm start

# Use with profiles
$ secretspec set API_KEY --profile production
$ secretspec run --profile production -- npm start
```