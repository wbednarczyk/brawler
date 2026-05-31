# Brawler Agent Contract

Brawler is the temporary codename for a local-first investor newsfeed desktop app. This repository is run as a spec-driven project: documentation and contracts define intent before implementation.

## Required Reading

Before making non-trivial changes, agents must read:

- [docs/project-brief.md](docs/project-brief.md)
- [docs/architecture.md](docs/architecture.md)
- [docs/contracts.md](docs/contracts.md)
- [docs/project-practices.md](docs/project-practices.md)
- relevant ADRs in [docs/adr/](docs/adr/)
- the active work item in [docs/kanban.md](docs/kanban.md)

## Working Rules

- Do not implement non-trivial changes without an explicit plan and approval.
- Keep public behavior, contracts, and docs in sync with code changes.
- Milestone closure must include the matching app version bump in `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`.
- If implementation evidence conflicts with a roadmap item or product requirement, explicitly call out the conflict, explain the tradeoff, and ask before weakening or deferring required scope.
- It is acceptable to challenge the user's proposed direction when technical, legal, source-policy, UX, cost, or reliability evidence suggests a better path, but the challenge must be communicated clearly before docs or code change the product commitment.
- Prefer small, reviewable changes that preserve local-first operation.
- Do not add cloud services, telemetry, hosted dependencies, or paid APIs unless a new ADR approves them.
- Treat `Brawler` as a codename only; do not hard-code it as the final product name in user-facing copy unless the spec says so.
- Preserve user privacy: watchlists, feed data, source history, AI outputs, and settings are local-only in v1.
- Prefer official, public, or RSS-based sources. Avoid fragile or restricted scraping unless a source-specific ADR approves it.
- AI output is decision support only. Do not phrase generated analysis as buy/sell/hold advice.
- Secrets must use the OS keychain in runtime code. `.env` is only for development and tests.
- Use strict Tauri permissions: typed commands only, no arbitrary shell execution, no broad filesystem access.
- Docs, ADRs, and contracts are canonical; GitHub Issues are implementation tracking only.
- Keep runtime dependency additions conservative and explain why they are needed.
- Local build/test commands are primary. GitHub Actions should mirror local commands, not introduce CI-only build logic.
- Use Nix from the first scaffold. Local commands should run inside `nix develop`; do not store secrets in Nix files or `.envrc`.
- For day-to-day agent iteration, prefer direct `rtk` commands with the locally installed toolchain to reduce token usage: `rtk rg`, `rtk sed`, `rtk npm run typecheck`, `rtk npm run test`, `rtk npm run build`, `rtk cargo fmt --check`, `rtk cargo clippy --all-targets -- -D warnings`, and `rtk cargo nextest run` or `rtk cargo test`.
- Avoid `rtk proxy` for normal work because it bypasses RTK output filtering. Use Nix-wrapped checks when reproducibility or parity matters, but expect lower RTK savings.
- Prefer Makefile targets for local WSL automation when available; they must remain thin wrappers around documented `nix develop` commands.
- Treat native Windows hands-on testing as a separate runtime validation path. Do not assume WSL has a GUI or that a WSL Tauri build validates Windows desktop behavior.
- Prefer `make package-windows-from-linux` for the on-demand packaged Windows sanity path once the cross-build spike is implemented. Treat `make windows-package` as a fallback that requires native Windows tooling.

## Testing Expectations

- Keep testing lean and fast. Prefer behavior and contract coverage over testing implementation details.
- Rust contracts, source adapters, deduplication, scheduler behavior, migrations, notebook workflows, transcription workflows, and AI mapping require automated tests.
- UI workflows for watchlists, feed filtering, unread state, source detail, and settings require component or workflow tests once UI exists.
- Desktop packaging changes require smoke tests for Tauri startup, Rust command availability, and local SQLite connectivity.
- Default CI must not require live external services or secrets. Use test samples and mocks for GPW, Gemini, SEC, Nasdaq, and media sources.
- Prefer the terms `test sample`, `sample data`, `seed data`, and `mock` in docs, UI text, and comments. Avoid `fixture` in project-facing language; if a conventional test path still uses `fixtures`, treat it as an internal implementation detail only.
- Keep GitHub Actions usage conservative: avoid larger runners, default macOS CI, scheduled workflows, and packaging on every push unless a later ADR approves them.
- The GitHub repository is currently private, so treat Actions minutes and artifact storage as constrained.
- Every default CI check must have a documented local equivalent.
- Prefer verifying the Nix environment in CI only when it remains fast and within the GitHub cost posture.

## Repository Notes

The root `.agents/` and `.codex/` directories are currently empty placeholders and should not be treated as authoritative configuration. This `AGENTS.md` file is the primary repo-level instruction source.
