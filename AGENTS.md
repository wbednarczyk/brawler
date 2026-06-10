# Brawler Agent Contract

Brawler is a local-first investor newsfeed desktop app. This repository is run as a spec-driven project: documentation and contracts define intent before implementation.

## Required Reading

Before making non-trivial changes, agents must read enough project context to understand the affected behavior without loading unrelated reference material.

Always read:

- [docs/project-brief.md](docs/project-brief.md) for product intent and the documentation map.
- [docs/project-practices.md](docs/project-practices.md) for standing operating rules.
- The active Radicle issue, epic, or task being implemented. Use [docs/kanban.md](docs/kanban.md) for the Radicle/Radboard tracking pointer.
- For milestone or release closure, read the repository-owned release workflow in [.agents/skills/brawler-release.md](.agents/skills/brawler-release.md).

Then read only the relevant canonical references for the work being done:

- Architecture or runtime boundaries: [docs/architecture.md](docs/architecture.md) and relevant ADRs in [docs/adr/](docs/adr/).
- Public command/data contracts: [docs/contracts.md](docs/contracts.md) and [docs/data-model.md](docs/data-model.md).
- User-facing behavior or UI flows: [docs/product-spec.md](docs/product-spec.md), [docs/ui-flows.md](docs/ui-flows.md), and [docs/ui-information-architecture.md](docs/ui-information-architecture.md).
- Source adapters and source policy: [docs/source-strategy.md](docs/source-strategy.md) and source-specific ADRs.
- Build, test, CI, packaging, or local environment behavior: [docs/engineering-workflow.md](docs/engineering-workflow.md).
- Module ownership or refactoring: [docs/modularization-design.md](docs/modularization-design.md).
- Historical completed-card context only when needed: [docs/kanban-archive.md](docs/kanban-archive.md).

## Working Rules

- Do not implement non-trivial changes without an explicit plan and approval.
- Start every new milestone by breaking it into tasks and presenting all important architecture decisions to the user. Explain options and tradeoffs briefly, require explicit answers, and ask until the architecture is clear before implementation.
- Agents may implement all approved milestone tasks, but must not close a milestone, move it to Done, or perform the milestone version bump until the user explicitly signs off on closure.
- Keep public behavior, contracts, and docs in sync with code changes.
- Before non-trivial implementation and milestone closure, perform an ADR checkpoint: add or update an ADR when the work changes durable architecture or policy decisions, or explicitly confirm that existing ADRs already cover the decision.
- Milestone closure must include the matching app version bump in `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`.
- Milestone and patch closure must include a `CHANGELOG.md` update generated from the release workflow and reviewed for human-readable clarity before handoff.
- Milestone and feature completion require real working application behavior against the real local runtime, real source, real API, or real agent described by the milestone. Samples, mocks, seed data, fake endpoints, and placeholder providers are valid only as intermediate development steps and in automated tests. They are not sufficient to mark a feature or milestone complete unless the roadmap explicitly defines that work item as a mock/sample-only spike.
- If implementation evidence conflicts with a roadmap item or product requirement, explicitly call out the conflict, explain the tradeoff, and ask before weakening or deferring required scope.
- It is acceptable to challenge the user's proposed direction when technical, legal, source-policy, UX, cost, or reliability evidence suggests a better path, but the challenge must be communicated clearly before docs or code change the product commitment.
- Prefer small, reviewable changes that preserve local-first operation.
- Treat modularity, extensibility, pluggability, and configurability as first-class design constraints across the whole application. New features should expose provider/source/model/credential/configuration/collector/renderer/storage-operation boundaries that are easy to extend, while avoiding premature complexity that is not tied to a real planned extension.
- Treat very large source files as architecture debt. When working near a large UI, storage, command, or test file, prefer extracting cohesive modules as part of the feature slice instead of adding more unrelated responsibility to the same file.
- Do not add cloud services, telemetry, hosted dependencies, or paid APIs unless a new ADR approves them.
- Treat `Brawler` as the official application name.
- Preserve user privacy: watchlists, feed data, source history, AI outputs, and settings are local-only in v1.
- Prefer official, public, or RSS-based sources. Avoid fragile or restricted scraping unless a source-specific ADR approves it.
- AI output is decision support only. Do not phrase generated analysis as buy/sell/hold advice.
- Secrets must use the OS keychain in runtime code. `.env` is only for development and tests.
- Use strict Tauri permissions: typed commands only, no arbitrary shell execution, no broad filesystem access.
- Docs, ADRs, and contracts are canonical; Radicle/Radboard issues are active project tracking only.
- Radicle is the canonical project forge. Do not publish, seed, unblock public seeding, change visibility to public, or use `rad init` without `--private` unless the current task is an explicitly approved public-opening or publication operation.
- If the sibling private repository `../brawler-private` exists, agents may read it for owner-only operational context. Do not copy private-repo content into this public repository, public docs, issues, patches, or pull requests unless the user explicitly asks for that specific content to be made public.
- Use Radboard labels with repeated flags only: `epic`, `parent:<epic-hex7>`, `milestone:v0.x.0`, `state:*`, `priority:critical|high|medium|low`, `blocked:*`, and project labels such as `area:research-workspace`.
- Create a Radicle issue for every bug that is reported or discovered and will not be fixed immediately in the current work. Use the plain `bug` label with state, priority, and area labels; link it with `parent:<epic-hex7>` or `blocked:<bug-hex7>` when relevant.
- Keep runtime dependency additions conservative and explain why they are needed.
- Local build/test commands are primary. GitHub Actions should mirror local commands, not introduce CI-only build logic.
- Use Nix from the first scaffold. Local commands should run inside `nix develop`; do not store secrets in Nix files or `.envrc`.
- For day-to-day agent iteration, prefer direct RTK-filtered commands with the locally installed toolchain to reduce token usage: `rtk grep` instead of raw `rg` for broad searches, `rtk read` for whole-file reads, `rtk npm typecheck`, `rtk npm test`, `rtk npm build`, `rtk cargo fmt --check`, `rtk cargo clippy --all-targets -- -D warnings`, and `rtk cargo nextest run` or `rtk cargo test`.
- Use raw `rtk sed` only for tight line ranges when `rtk read --max-lines` is not suitable; it often falls back to passthrough and saves fewer tokens. Use `rtk git diff` or `rtk diff` instead of raw large diffs.
- Avoid `rtk proxy` for normal work because it bypasses RTK output filtering. Use Nix-wrapped checks when reproducibility or parity matters, but expect lower RTK savings.
- Prefer Makefile targets for local WSL automation when available; they must remain thin wrappers around documented `nix develop` commands.
- Treat native Windows hands-on testing as a separate runtime validation path. Do not assume WSL has a GUI or that a WSL Tauri build validates Windows desktop behavior.
- Prefer `make package-windows-from-linux` for the on-demand packaged Windows sanity path once the cross-build spike is implemented. Treat `make windows-package` as a fallback that requires native Windows tooling.

## Testing Expectations

- Keep testing lean and fast. Prefer behavior and contract coverage over testing implementation details.
- Automated tests may use mocks, injected fetchers, and test samples to stay fast and deterministic, but agents must not present mock/sample success as proof that a user-facing feature or milestone is complete.
- Rust contracts, source adapters, deduplication, scheduler behavior, migrations, notebook workflows, transcription workflows, and AI mapping require automated tests.
- UI workflows for watchlists, feed filtering, unread state, source detail, and settings require component or workflow tests once UI exists.
- Desktop packaging changes require smoke tests for Tauri startup, Rust command availability, and local SQLite connectivity.
- Default CI must not require live external services or secrets. Use test samples and mocks for GPW, Gemini, SEC, Nasdaq, and media sources.
- Prefer the terms `test sample`, `sample data`, `seed data`, and `mock` in docs, UI text, and comments. Avoid `fixture` in project-facing language; if a conventional test path still uses `fixtures`, treat it as an internal implementation detail only.
- Keep GitHub Actions usage conservative: avoid larger runners, default macOS CI, scheduled workflows, and packaging on every push unless a later ADR approves them.
- Keep GitHub Actions usage conservative: avoid unnecessary minutes, artifact storage, and packaging jobs until public CI behavior is explicitly revisited.
- Every default CI check must have a documented local equivalent.
- Prefer verifying the Nix environment in CI only when it remains fast and within the GitHub cost posture.

## Repository Notes

The root `.agents/skills/` directory stores repository-owned, agent-neutral workflows. The root `.codex/skills/` directory may contain Codex-specific entrypoints that delegate to those shared workflows. This `AGENTS.md` file remains the primary repo-level instruction source.
