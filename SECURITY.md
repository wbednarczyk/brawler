# Security Policy

## Supported Versions

Brawler is pre-1.0. Security fixes target the current development line unless a release branch is explicitly announced.

## Reporting A Vulnerability

Do not disclose security-sensitive details in a public issue before the maintainer has had a reasonable chance to assess them.

If no private reporting channel is listed on the public forge yet, open a minimal public issue that says a private security report is needed, without exploit details, secrets, tokens, logs, databases, or personal data.

## Scope

Relevant issues include:

- secret leakage
- unsafe filesystem access
- Tauri command boundary bypasses
- local database exposure
- license token leakage
- provider credential leakage
- dependency vulnerabilities that affect the packaged app

Out of scope for the default security process:

- speculative investment or trading outcomes
- public source availability
- unsupported modified builds
- issues that require physical access to an already-compromised machine

## Security Posture

Brawler is local-first. User data and secrets should remain on the user's machine unless a user explicitly configures an external provider workflow.
