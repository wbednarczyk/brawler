# ADR 0017: Local Entitlement Module

Status: Accepted

## Context

Brawler includes a local entitlement boundary so future gated features can be enabled without making the open desktop core depend on hosted activation, telemetry, billing infrastructure, or cloud accounts.

The detailed owner-only licensing and token operations that informed the original implementation belong in the private sibling repository described by [ADR 0023](0023-public-private-documentation-split.md). Public-opening work chose MPL-2.0 for the open desktop core and changed the entitlement policy so normal desktop use does not require a license token.

## Decision

Brawler keeps licensing code as an extensible local entitlement module rather than a first-run app-access blocker.

The entitlement module is organized around these boundaries:

- parser/verifier boundary for locally supplied entitlement material
- entitlement policy boundary for mapping verified claims to capabilities
- secret-store boundary for any raw accepted entitlement material
- storage boundary for derived non-secret metadata/status
- typed command boundary for status, submit, and clear operations
- presentation boundary for Settings/About status without exposing secret material

Normal open-core desktop use remains available when no valid entitlement is present. Missing, malformed, expired, unsupported, or storage-error states must remain recoverable through Settings and must not block normal core navigation.

Future paid-feature, subscription, hosted-activation, or support-entitlement policies must be added as adapters behind these boundaries and require a later ADR when they introduce hosted services, accounts, billing, or remote checks.

## Consequences

- The open desktop core remains local-first and usable without activation.
- Optional/future gated features can reuse the same entitlement boundary.
- Secret material must not be stored in the repository, app database, logs, exported settings, Nix files, GitHub Actions, or app build outputs.
- The UI may show safe entitlement status and metadata but must not expose raw entitlement material.
- Stronger commercial controls, hosted activation, account identity, subscription refresh, revocation, cloud sync, or billing remain future decisions.
