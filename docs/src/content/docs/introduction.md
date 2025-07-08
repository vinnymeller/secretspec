---
title: Introduction
description: Learn about SecretSpec and the problems it solves
---

Modern applications require secrets - API keys, database credentials, service tokens. Yet we lack a standard way to declare these requirements. Applications either hard-code retrieval mechanisms or fail at runtime with missing environment variables.

## The Problem: Conflating What, How, and Where

Current secret management approaches force applications to simultaneously answer three distinct questions:

- **WHAT** - Which secrets does the application need? (DATABASE_URL, API_KEY)
- **HOW** - What are the requirements? (required vs optional, defaults, validation, environment)
- **WHERE** - Where are these secrets stored? (environment variables, Vault, AWS Secrets Manager)

This coupling creates several problems:

1. **Lack of Portability**: Applications become tightly coupled to specific storage backends, making it difficult to switch providers or adapt to different environments
2. **Runtime Failures**: Missing secrets are only discovered when the application attempts to use them, leading to crashes in production
3. **Poor Developer Experience**: Each developer must understand the specific storage mechanism and manually configure their environment
4. **Inconsistent Practices**: Every application implements its own ad-hoc solution, leading to a fragmented ecosystem

## The Solution: Declarative Secret Requirements

SecretSpec introduces a declarative approach that separates the "what" and "how" from the "where":

- **WHAT** secrets are needed is declared in `secretspec.toml`
- **HOW** requirements vary by environment is managed through `profile`
- **WHERE** secrets are stored depends on where the application runs, configured via `provider`

Applications declare their secret requirements in a `secretspec.toml` file, while the runtime environment determines the storage backend through `provider` configuration and context via `profile` selection.

This separation enables:
- **Portable Applications**: The same application works across different secret storage backends without code changes
- **Early Validation**: Check that all required secrets are available before starting the application
- **Better Tooling**: Standardized format enables ecosystem-wide tooling for secret management
- **Type Safety**: Generate strongly-typed code from declarations for compile-time guarantees