# ADR 0009: Local Config, Secrets, Data, and Observability

Status: Accepted

## Context

Brawler stores personal research, watchlists, source history, settings, provider configuration, and eventually API keys. The app should stay local-first, private by default, and easy to debug without telemetry.

## Decision

Secrets live in the OS keychain. `.env` files are allowed only for local development and tests.

SQLite is the runtime source of truth for settings. YAML is supported as an import/export/bootstrap format, but secrets must never be stored in YAML.

SQLite data and local logs live in the OS app data directory by default, with a development-only override allowed.

V1 uses local logs only. Telemetry and remote error reporting are not allowed without a future ADR.

## Consequences

- Settings panel reads/writes SQLite.
- YAML import validates supported settings and writes them into SQLite.
- YAML export excludes secrets.
- Source and job errors surface in the Sources screen.
- Logs must redact secrets and should avoid full note bodies and full transcript text by default.
- Cloud backup/sync requires separate design and a future ADR.
