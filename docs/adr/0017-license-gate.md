# ADR 0017: License Gate

## Status

Accepted

## Context

Brawler needs a lightweight gate before v1 friend-test artifacts are distributed outside the project owner. The goal is to prevent casual redistribution while preserving local-first behavior and keeping the final public license decision open. The project owner also needs an author-only license path for normal personal use and for manual validation of the same gate that friend-test users will exercise.

The license and monetization assessment that informed M17 is owner-only context and belongs in the private sibling repository described by [ADR 0023](0023-public-private-documentation-split.md). [ADR 0008](0008-license-and-project-governance.md) captures the historical private posture. Public-opening work later chose MPL-2.0 for the open desktop core.

The gate must work offline. It must not require cloud accounts, telemetry, hosted activation, billing infrastructure, or any remote entitlement service. It also must not store private signing material in the repository, app database, logs, exported settings, Nix files, `.envrc`, GitHub Actions, or app build outputs.

## Decision

M17 implements licensing as an extensible local entitlement module. The author and friend-test license gate is the first entitlement policy, not final DRM and not the final public license model.

The licensing module uses these boundaries:

- token parser: decodes a versioned signed license token
- verifier adapter: verifies signatures using embedded public verification material
- entitlement policy: maps verified claims, app version, build channel, and current time to allowed app capabilities
- secret-store adapter: stores and clears the raw accepted token
- storage adapter: persists only derived redacted metadata/status in SQLite
- command boundary: exposes typed Tauri commands for status, submit, and clear
- UI boundary: renders the first-run gate and Settings/About status without receiving private signing material

The original M17 local policy required a valid offline signed license token for normal app use. Public-opening work changed this policy: the open desktop core remains usable without a license token, and license status now represents optional or future gated entitlements. Missing, malformed, tampered, expired, unsupported-license-version, unsupported-channel, and wrong-app-version states must remain recoverable through Settings, but must not block normal core navigation.

M17 supports two signed channels:

- `author`: owner-only license with `edition: "author"` and `features: ["*"]`; signed by the separate author private key matching `key_id: "owner_author_2026_06"`.
- `friend_test`: friend-test license with bounded features and expiry, but no app-version restriction; signed by the friend-test private key matching `key_id: "owner_friend_test_2026_06"`.

The author channel is not a bypass. It uses the same token parser, signature verifier, OS keychain storage, SQLite metadata storage, Tauri commands, and UI gate as friend-test tokens. This keeps manual testing representative while allowing the project owner to keep all local features enabled.

M17 author and friend-test tokens are not app-version bounded and should use `app_version_range: "*"`. The entitlement module keeps a version-limit policy path so future channels can opt into app-version ranges without a ground-up refactor.

License tokens use a versioned signed JSON envelope. The token should be readable and testable, without introducing JWT semantics or a hosted identity model.

License claims should include:

- `license_id`
- `holder`
- `channel`
- `edition`
- `features`
- `issued_at`
- `expires_at`
- `app_version_range`
- `key_id`

The first verifier uses Ed25519. The app embeds one or more public verification keys keyed by `key_id` so later key rotation can be added without rewriting token parsing or entitlement policy. Author and friend-test licenses use separate signing keys so owner access can rotate independently from friend-test distribution. Private production signing keys and production key generation workflows stay outside the repository and build outputs. Test keys may exist only as clearly marked test material for deterministic automated tests.

The raw accepted license token is treated as a bearer secret and stored through the OS keychain. SQLite stores only derived, redacted, non-secret metadata needed for status display, diagnostics, and local recovery. Logs, diagnostics, metrics, settings export, tests, and UI state must not include full license tokens, private signing material, or raw private key material.

The entitlement module runs after initial local startup. It no longer blocks normal open-core navigation when no valid license is present. It still allows the user to enter, replace, inspect safe metadata, and clear a license for gated entitlements.

## Threat Model

M17 protects against casual redistribution and accidental use of friend-test builds by users who were not given a valid token.

M17 does not try to defeat:

- binary patching
- reverse engineering
- system clock tampering
- extracting embedded public verification material
- rebuilding from future public source
- copying a valid token from one machine to another
- hosted subscription abuse or entitlement revocation

Stronger commercial controls, hosted activation, account identity, subscription refresh, revocation, cloud sync, or billing require a future ADR.

## Consequences

- Brawler remains local-first in v1.
- The project owner can activate offline with an author token that exercises the same entitlement path as friend-test tokens.
- Friend-test users can activate offline by pasting a signed token from the project owner when they need gated entitlements.
- Future paid feature policies can replace or extend the friend-test policy without rewriting the app shell or Settings workflows.
- App code has explicit extension points for future parser, verifier, secret-store, storage, entitlement-policy, and presentation adapters.
- Keychain availability becomes part of license activation behavior; missing keychain support must produce a clear recoverable error.
- Release-owner production signing material remains an external operational concern, not repository content.
- The app cannot remotely revoke a license or prove the local clock is honest in M17.
