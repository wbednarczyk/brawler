# ADR 0070: Typed Command Error Envelope

Status: Accepted

Every IPC command returns `Result<T, String>`, flattening the rich internal `StorageError` (~40 structured variants) into opaque strings at the boundary. The frontend cannot distinguish "missing credential" from "network failure" from "not found", so it cannot offer the right recovery action.

## Context

- 2026-07-03 audit: 180 commands uniformly `.map_err(|e| e.to_string())`. Pragmatic and consistent, but the UI branches on failure reasons in exactly zero places because it can't.
- Upcoming features raise the stakes: alert rules, market-data fetches, and MCP tools all want machine-readable failure kinds.

## Decision

1. **`CommandError { code, message }`** — a small serializable envelope (ts-rs generated DTO): `code` is a closed enum (initial set: `not_found`, `invalid_input`, `missing_credential`, `network`, `provider`, `conflict`, `internal`), `message` stays the human-readable detail. A `From<StorageError>` mapping assigns codes centrally.
2. **Strangler adoption**: new commands return `Result<T, CommandError>` from day one; existing commands migrate when touched. No big-bang rewrite. The frontend `callCommand` wrapper accepts both shapes during the transition.
3. Error-code semantics are documented in `docs/contracts.md` once the first migrated commands land (doc-first per slice); codes are additive-only.

## Consequences

- UI can branch: `missing_credential` → link to Settings/AI, `network` → retry affordance, `conflict` → refresh prompt.
- The docs-drift command check (ADR 0065) is unaffected; contract docs gain an error-codes section with the first implementation slice.
- Mock runtime and dual-execution fidelity corpus (ADR 0049) must mirror the envelope for migrated commands.
