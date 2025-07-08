# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

SecretSpec is a declarative secrets manager for development workflows written in Rust. It provides a CLI tool and Rust library for managing environment variables and secrets across different environments using multiple storage backends (keyring, dotenv, environment variables).

## Build and Development Commands

```bash
# Enter development environment
devenv shell

# Run tests
cargo test --all

# Run the CLI
cargo run -- <command>
cargo run -- init --from .env
cargo run -- check
cargo run -- set DATABASE_URL
cargo run -- run -- npm start

# Format code
cargo fmt

# Run linter
cargo clippy
```

## Architecture

The project is organized as a Rust workspace with three main crates:

1. **secretspec** (src/): Main CLI and library
   - `main.rs`: CLI entry point using clap
   - `lib.rs`: Core library functionality
   - `provider/`: Storage backend implementations with trait-based architecture

2. **secretspec-derive**: Proc macro for type-safe code generation
   - Generates strongly-typed structs from secretspec.toml
   - Supports profile-specific types

3. **secretspec-types**: Shared type definitions

## Provider System

The provider system uses a trait-based architecture defined in `src/provider/mod.rs`. When implementing new providers:

1. Create module in `src/provider/your_provider.rs`
2. Implement the `Provider` trait
3. Register in `ProviderRegistry::new()` in `src/provider/mod.rs`

## Testing

- Unit tests are located alongside the code
- Integration tests in `secretspec-derive/tests/`
- UI tests using `trybuild` for macro error testing
- Run specific test: `cargo test test_name`

## Key Files

- `secretspec.toml`: Project secrets configuration
- `src/provider/mod.rs:27`: Provider trait definition
- `src/main.rs:20`: CLI command definitions
- `secretspec-derive/src/lib.rs`: Code generation macro implementation
