# License Operations

This document covers the license gate from the release-owner perspective.

Brawler remains all rights reserved for now.

## Architecture

The app validates local signed license tokens through the licensing module:

- token parser: reads `BRAWLER-LIC-1.<payload>.<signature>`
- verifier adapter: checks the Ed25519 signature against embedded public keys selected by `key_id`
- entitlement policy: maps verified claims to local app access
- secret-store adapter: stores the raw accepted token in the OS keychain
- storage adapter: stores only derived redacted metadata in SQLite
- command boundary: exposes `get_license_status`, `submit_license_key`, and `clear_license_key`
- UI boundary: renders the gate screen and Settings license status

The frontend never receives private signing material. SQLite never stores the full token. Logs, diagnostics, metrics, settings export, and tests must not include full license tokens or private keys.

## License Channels

M17 supports two local signed channels:

| Channel | Key id | Purpose |
| --- | --- | --- |
| `author` | `owner_author_2026_06` | Owner-only license with `edition: "author"` and `features: ["*"]`. |
| `friend_test` | `owner_friend_test_2026_06` | Friend-test license with bounded features and expiry, valid for all app versions. |

The channel and key are both meaningful. An author token must contain author claims and must be signed by the author private key. A friend-test token must contain friend-test claims and must be signed by the friend-test private key.

## Private Key Locations

Private keys live outside the repository:

- author key: `/home/<user>/.local/share/rtk/brawler/author-license-ed25519.pem`
- friend-test key: `/home/<user>/.local/share/rtk/brawler/friend-test-license-ed25519.pem`

Do not commit private keys, generated tokens, token inventories, command logs containing tokens, or recipient notes. Local generated outputs should go under `private/`, which is gitignored.

## Generate Or Refresh The Author Key

The author key has already been generated for M17. To generate it on a fresh machine:

```sh
make license-keygen-author
```

The command fails if the key already exists. Do not use `--force` unless rotating the author key deliberately and updating the embedded public key in the app.

## Generate Your Author License

```sh
make license-author
```

Default output:

```text
private/licenses/author.txt
```

Paste the token from that file into the app license gate. The token is verified offline and then stored in the OS keychain.

The default author claims are:

- `channel: "author"`
- `edition: "author"`
- `features: ["*"]`
- `expires_at: "2099-01-01T00:00:00Z"`
- `app_version_range: "*"`
- `key_id: "owner_author_2026_06"`

Override the output path when needed:

```sh
OUT=private/licenses/author-laptop.txt make license-author
```

## Generate A Friend-Test License

```sh
make license-friend HOLDER="Friend Name"
```

Default output:

```text
private/licenses/friend-friend-name.txt
```

Optional overrides:

```sh
EXPIRES_AT="2026-12-31T23:59:59Z" make license-friend HOLDER="Friend Name"
FEATURES="core,notebooks" make license-friend HOLDER="Friend Name"
OUT=private/licenses/friend-custom.txt make license-friend HOLDER="Friend Name"
```

Send only the generated token to the friend. Do not send private keys or key-generation scripts as instructions for them to run.

## Manual Gate Testing

Before distributing a friend-test build, manually verify these states against the real packaged app:

- missing token: app shows the license gate before normal navigation
- invalid token: app stays gated and shows a recoverable error
- valid author token: app opens and Settings shows `channel: author`, `edition: author`, `features: *`
- valid friend-test token: app opens and Settings shows `channel: friend_test`
- clear token: Settings clears the keychain token and returns to the gate

For expired checks, generate a temporary token with a past `EXPIRES_AT`, then paste it into the gate. M17 author and friend-test tokens are not app-version bounded. Future channels can opt into version limits through `app_version_range`.

## Key Rotation

Key rotation is a code change because public verification keys are embedded in the app. Every license token carries a `key_id`; the app first verifies the token signature against the embedded public key for that `key_id`, then checks that the key is allowed for the token channel.

Use a new `key_id` for every rotation. Use a clear date suffix, for example:

- author: `owner_author_2026_09`
- friend-test: `owner_friend_test_2026_09`

### Rotation Option 1: Add A New Key And Keep Old Licenses Working

Use this for normal planned rotation when existing licenses should continue to work.

1. Choose the channel to rotate: `author` or `friend_test`.
2. Choose a new `key_id`.
3. Generate a new private key outside the repository.

For author:

```sh
node scripts/licensing/generate-ed25519-key.mjs \
  --key-path /home/wojtas/.local/share/rtk/brawler/author-license-ed25519-2026-09.pem
```

For friend-test:

```sh
node scripts/licensing/generate-ed25519-key.mjs \
  --key-path /home/wojtas/.local/share/rtk/brawler/friend-test-license-ed25519-2026-09.pem
```

4. Copy the printed `public_key_base64`.
5. Add a new entry to `LOCAL_LICENSE_PUBLIC_KEYS` in `src-tauri/src/licensing/verifier.rs`.

Example:

```rust
VerificationKey {
    key_id: "owner_friend_test_2026_09",
    public_key_base64: "PASTE_PRINTED_PUBLIC_KEY_BASE64_HERE",
},
```

6. Add the new `key_id` to the matching channel allow-list in `src-tauri/src/licensing/entitlement.rs`.

For author:

```rust
const AUTHOR_KEY_IDS: &[&str] = &["owner_author_2026_06", "owner_author_2026_09"];
```

For friend-test:

```rust
const FRIEND_TEST_KEY_IDS: &[&str] = &[
    "owner_friend_test_2026_06",
    "owner_friend_test_2026_09",
];
```

7. Update `scripts/licensing/generate-license.mjs` so the selected license type uses the new private key path and new `keyId`.
8. Generate a new test license for the rotated channel.
9. Verify the old license still works.
10. Verify the new license works.
11. Run the license and full validation checks:

```sh
rtk cargo test licensing
rtk npm run typecheck
rtk npm test -- --run
rtk npm run build
cd src-tauri
rtk cargo fmt --check
rtk cargo clippy --all-targets -- -D warnings
rtk cargo test
```

12. Ship a new app build. Users only gain support for the new public key after installing that build.

### Rotation Option 2: Replace A Key And Stop Old Licenses Working

Use this if the old private key is compromised, or if old licenses should intentionally stop working in future app builds.

1. Follow steps 1-5 from Option 1 to generate a new key and add its public key.
2. Remove the old public key entry from `LOCAL_LICENSE_PUBLIC_KEYS` in `src-tauri/src/licensing/verifier.rs`.
3. Remove the old `key_id` from the matching allow-list in `src-tauri/src/licensing/entitlement.rs`.

For example, after replacing friend-test:

```rust
const FRIEND_TEST_KEY_IDS: &[&str] = &["owner_friend_test_2026_09"];
```

4. Update `scripts/licensing/generate-license.mjs` so future tokens use only the new private key path and new `keyId`.
5. Generate a new test license for the rotated channel.
6. Verify the old license no longer works in the new build.
7. Verify the new license works in the new build.
8. Run the same validation checks listed in Option 1.
9. Ship a new app build. Old licenses stop working only after users install that build.

### Rotation Notes

- Do not reuse a `key_id` with a different key. Reuse makes debugging and support ambiguous.
- Do not use `--force` on an existing private key path unless intentionally overwriting that key.
- Keep old private keys only as long as you still need to generate replacement tokens signed with that old key. The app only needs public keys to verify existing tokens.
- If a private key may be compromised, prefer Option 2 and remove the old public key from the next build.
- If old licenses should remain valid while new licenses move to the new key, prefer Option 1.

Hosted revocation, subscription refresh, billing, and account identity require a future ADR.
