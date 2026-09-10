# ADR 0110: Retire the Local Entitlement Module

Status: Accepted (2026-09-07, owner decision at the architecture audit, N02; implemented 2026-09-10, #462)

Supersedes [ADR 0017](0017-license-gate.md) (the local entitlement module). Amends [ADR 0039](0039-ports-and-adapters-posture.md) (the licensing adapter family is gone from the ports list).

## Context

ADR 0017 kept a local entitlement module "for future gated features": an Ed25519 token parser/verifier, an entitlement policy, an OS-keychain token store, a `license_metadata` storage layer, three IPC commands, a Settings › License tab, and a keychain-read gate in the Rust scheduler and the React lifecycle. Public-opening work (MPL-2.0) had already made every entitlement state `canUseApp = true`, so for the whole life of the module no state ever blocked anything. The 2026-09-07 audit found both traits with exactly one implementation, version-limit logic no supported channel could reach, and ~1,400 lines of Rust plus ~20 frontend files carrying it. Zero product value; open-core optionality is a product decision that lives in the brief, not in code.

## Decision

1. **Delete the module.** `src-tauri/src/licensing/`, `commands/licensing.rs`, `storage/licensing.rs`, the `get_license_status` / `submit_license_key` / `clear_license_key` commands, the Settings › License tab and its state, the `licenseCanUseApp` gates (scheduler tick, daily briefing, attention/activity/lifecycle controllers) — all removed, none feature-flagged. `ed25519-dalek` leaves `Cargo.toml`; `keyring` stays (the transcript-provider secret) and so does the generic secret-name redaction in observability.
2. **The `license_metadata` table stays, empty.** Migrations are append-only; migration `0154` deletes its rows (derived, non-secret metadata; owner-approved). No reader remains.
3. **An entitlement token already in the OS keychain stays inert.** No code reads it; deleting it would keep keychain code alive one more release. The owner may remove the entry by hand.
4. **Retired surface is pinned** in `docs/retired-surface.json` (commands, component, `canUseApp`, Make targets) so no live doc can re-specify it; `docs-drift`, `knip` and the Rust build keep the code from returning.
5. **A future gated feature starts from a new ADR** — hosted activation, accounts, billing, remote checks remain out of scope exactly as before (roadmap § Not In V1).

## Rejected

- **Feature flag / keep the storage layer** — dead code with a switch is still dead code; the audit's point was the maintenance surface.
- **Drop the table** — migrations are append-only and immutable (data-model rules); an empty table costs nothing.
- **One-shot keychain cleanup at startup** — keeps `OsKeychainLicenseTokenStore` alive for one release to delete a harmless entry.

## Consequences

- Settings loses one tab; About shows app name and version only; Import/Export copy no longer mentions licenses.
- The scheduler no longer touches the OS keychain on every tick.
- `docs/bad-ideas.md` records the retreat; ADR 0017 is Superseded.
