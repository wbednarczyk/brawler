# ADR 0010: Security, Dependencies, and AI Policy

Status: Accepted

## Context

Brawler is a desktop app with Tauri-native capabilities, local personal data, source ingestion, and AI features. Day-1 defaults should prevent broad permissions, dependency bloat, and risky AI framing.

## Decision

Brawler uses a strict Tauri/security baseline:

- frontend never receives API keys
- frontend calls only typed Tauri commands
- no arbitrary shell execution
- no broad filesystem access
- source and provider network requests happen in Rust
- URLs are validated before use
- least-privilege Tauri capabilities and plugins are required

Brawler uses a conservative dependency policy. Dependencies should be maintained, common, license-reviewed, and added only when they solve a real problem.

Default AI mode is `source_grounded`. Future `opinionated` mode may be added behind explicit user opt-in, but it must still cite sources and must not provide buy/sell/hold or personalized portfolio advice.

## Consequences

- Broad filesystem, network, or shell permissions require a future ADR.
- Runtime dependency additions should be visible in PR or commit descriptions.
- AI output requires source references.
- User confirms AI-suggested notes before saving.
- Prompt templates should be versioned once they exist.
