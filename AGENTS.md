# Brawler Agent Contract

Brawler is the temporary codename for a local-first investor newsfeed desktop app. This repository is run as a spec-driven project: documentation and contracts define intent before implementation.

## Required Reading

Before making non-trivial changes, agents must read:

- [docs/project-brief.md](docs/project-brief.md)
- [docs/architecture.md](docs/architecture.md)
- [docs/contracts.md](docs/contracts.md)
- relevant ADRs in [docs/adr/](docs/adr/)
- the active work item in [docs/kanban.md](docs/kanban.md)

## Working Rules

- Do not implement non-trivial changes without an explicit plan and approval.
- Keep public behavior, contracts, and docs in sync with code changes.
- Prefer small, reviewable changes that preserve local-first operation.
- Do not add cloud services, telemetry, hosted dependencies, or paid APIs unless a new ADR approves them.
- Treat `Brawler` as a codename only; do not hard-code it as the final product name in user-facing copy unless the spec says so.
- Preserve user privacy: watchlists, feed data, source history, AI outputs, and settings are local-only in v1.
- Prefer official, public, or RSS-based sources. Avoid fragile or restricted scraping unless a source-specific ADR approves it.
- AI output is decision support only. Do not phrase generated analysis as buy/sell/hold advice.

## Testing Expectations

- Rust contracts, source adapters, deduplication, scheduler behavior, migrations, notebook workflows, transcription workflows, and AI mapping require automated tests.
- UI workflows for watchlists, feed filtering, unread state, source detail, and settings require component or workflow tests once UI exists.
- Desktop packaging changes require smoke tests for Tauri startup, Rust command availability, and local SQLite connectivity.

## Repository Notes

The root `.agents/` and `.codex/` directories are currently empty placeholders and should not be treated as authoritative configuration. This `AGENTS.md` file is the primary repo-level instruction source.

The current environment contains an empty, read-only `.git` placeholder that blocks `git init`. Replace or repair that placeholder before expecting normal Git commands to work.
