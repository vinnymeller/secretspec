---
title: Installation
description: Install SecretSpec on your system
---

SecretSpec can be installed using several methods. Choose the one that best fits your environment.

## Static Binary

The easiest way to install SecretSpec is using our installation script:

```bash
$ curl -sSL https://secretspec.dev/install | sh
```

This will download and install the latest version of SecretSpec to your system.

## Devenv.sh

If you're using [devenv.sh](https://devenv.sh) for development environments, see the [devenv integration guide](https://secretspec.dev/docs/devenv) for setup instructions.

Add SecretSpec to your `devenv.nix`:

```nix
{
  packages = [ pkgs.secretspec ];
}
```

## Nix

For Nix users, install SecretSpec from nixpkgs:

```bash
$ nix-env -iA secretspec -f https://github.com/NixOS/nixpkgs/tarball/nixpkgs-unstable
```

Or add it to your `configuration.nix`:

```nix
environment.systemPackages = with pkgs; [
  secretspec
];
```

## Verify Installation

After installation, verify that SecretSpec is available:

```bash
$ secretspec --version
secretspec 0.1.0
```

## Platform Support

SecretSpec is supported on:
- Linux (x86_64, aarch64)
- macOS (Intel and Apple Silicon)
- Windows (x86_64)

*Please open pull requests once SecretSpec is available in your favorite distribution.*