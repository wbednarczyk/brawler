# Project Practices

This document captures day-1 operating rules for Brawler. It complements the product, architecture, and roadmap docs by defining how the project should be maintained.

See also [Project Brief](project-brief.md), [Architecture](architecture.md), [Engineering Workflow](engineering-workflow.md), [Roadmap](roadmap.md), and [Kanban](kanban.md).

## License Posture

Brawler is all rights reserved for now. The GitHub repository is currently private. Do not add an open-source license until a future ADR resolves the license and commercial boundary.

Rules:

- External contribution is not expected while no license exists.
- Do not accept outside contributions without revisiting license posture.
- Do not publish public release artifacts without revisiting license posture.
- Do not distribute friend-test v1 artifacts until a local license-key gate is implemented and documented.
- The v1 license-key gate should prevent casual redistribution without requiring cloud accounts, telemetry, hosted activation, or billing infrastructure.
- License private signing material must never be stored in the repository, app database, logs, exported settings, Nix files, `.envrc`, or GitHub Actions secrets unless a future ADR explicitly approves the release process.
- The future monetization model is undecided. Open core plus paid convenience features is only one candidate.

## Secrets And Config

Secrets and settings have separate sources of truth.

Rules:

- API keys and provider secrets live in the OS keychain.
- `.env` files are allowed only for local development and tests.
- Runtime settings live in SQLite.
- YAML is allowed as import/export/bootstrap format.
- Secrets must never be stored in YAML, SQLite settings, logs, frontend state, or GitHub Actions.
- Settings panel reads/writes SQLite.
- Importing YAML validates supported settings and writes them into SQLite.
- Exporting settings to YAML excludes secrets.

## Local Data And Backup

V1 stores local app data in the OS app data directory by default. Development builds may override the data directory through a dev-only setting or environment variable.

Rules:

- SQLite database lives under the app data directory by default.
- Local logs live under the app data/log directory.
- Exports are built as normal features during v1 implementation.
- Import/restore and full local backup are late-v1 roadmap items.
- Cloud backup/sync is a future feature and requires a separate design discussion and ADR.

## Observability

V1 uses local logs only. There is no telemetry or remote error reporting.

Rules:

- Sources screen shows adapter and job errors.
- Logs must not include API keys.
- Logs should not include full note bodies or full transcript text by default.
- Logs may include IDs, source URLs, statuses, timestamps, and error classes.
- Diagnostic export may be added later as a user-triggered feature.
- Telemetry or remote reporting requires a future ADR.

## Dependency Policy

Brawler uses a conservative dependency policy.

Rules:

- Prefer maintained, established libraries in the Tauri, Rust, React, and TypeScript ecosystems.
- Add dependencies only when they solve a real problem.
- Avoid large frameworks unless they directly support the product direction.
- Prefer proven libraries for SQLite, migrations, Markdown, date/time, forms, testing, and OS keychain integration.
- Avoid dependencies that require paid or hosted services for core functionality.
- Review licenses before adding runtime dependencies.
- Mention meaningful dependency additions in PR or commit descriptions.

## UX Quality

Intuitive UX and responsive UI are first-class project requirements, not polish to add at the end.

Rules:

- Prefer workflows that make the likely next action obvious.
- Prefer direct row interaction for list/detail workflows. When a row opens more context, clicking the row should expand details inline near that row, and clicking the same row again should collapse it when that behavior is natural.
- Avoid adding explicit row-level buttons for primary open/inspect behavior when the whole row can safely be the target. Keep buttons for secondary actions such as delete, source links, or explicit state changes.
- Mutating actions should provide quick visual feedback.
- Buttons and controls should communicate intent through position, label, icon, color, and state.
- Dense investor workflows must remain scannable and keyboard/mouse efficient.
- Temporary UX shortcuts are allowed during early scaffolding, but known UX debt should be recorded in docs or Kanban.
- Responsiveness is part of correctness: common actions should feel immediate even when background work is pending.

## Security Baseline

Brawler uses a strict Tauri/security baseline from day 1.

Rules:

- Frontend never receives API keys.
- Frontend calls only typed Tauri commands.
- Do not expose arbitrary shell execution.
- Do not expose broad filesystem access.
- Source and provider network requests happen in Rust, not direct browser fetch from React.
- Validate URLs before using them.
- YouTube transcription input must be a valid supported YouTube URL.
- Redact sensitive values in errors.
- Use least-privilege Tauri capabilities and plugins.
- Broad filesystem, network, or shell permissions require a future ADR.

## AI Policy

Default AI mode is `source_grounded`.

Rules:

- AI results require source references.
- AI output must not include buy/sell/hold recommendations.
- AI output must not include portfolio allocation advice.
- Price targets are not generated unless directly quoted from a source.
- AI should separate summary from interpretation.
- Significance labels require reasoning.
- Uncertainty is allowed and should be explicit.
- User confirms AI-suggested notes before saving.
- Prompt templates should be versioned once they exist.

Future opinionated mode:

- `opinionated` mode may be added behind explicit user opt-in.
- Opinionated output must still cite sources and show uncertainty.
- Opinionated mode still cannot provide buy/sell/hold or personalized portfolio advice.

## Export, Import, And Backup

Export is part of normal v1 implementation. Import/restore and full local backup are late-v1 items.

Rules:

- Notes should export as Markdown with metadata.
- Watchlists and companies should export as structured JSON or YAML.
- Settings export uses YAML and excludes secrets.
- Full SQLite backup may be documented as a power-user option.
- Cloud backup/sync requires a separate design discussion before implementation.

## GitHub Workflow

Brawler uses a hybrid workflow.

Rules:

- Docs, ADRs, and contracts are canonical for product and architecture decisions.
- `docs/kanban.md` remains the high-level planning board.
- GitHub Issues may track implementation tasks once useful.
- PRs should reference an issue or Kanban card.
- Behavior changes must update docs/contracts in the same PR.
- Important decisions must not live only in GitHub issue or PR comments.

## Product Scope And Tradeoff Communication

Agents are expected to exercise engineering judgment, including pushing back when evidence suggests the current path is unreliable, too costly, legally risky, or poor UX. That pushback must be explicit and collaborative.

Rules:

- Do not silently weaken, defer, or remove a product requirement because implementation is difficult.
- If a planned implementation path looks unreliable, explain the evidence and propose alternatives.
- If a user proposal conflicts with roadmap, contracts, ADRs, source policy, privacy, security, or cost posture, call out the conflict before implementing.
- When a required feature has a risky implementation path, keep the requirement intact and discuss alternate paths rather than making the feature optional.
- Docs may record uncertainty, fallback options, and technical risk, but they must not downgrade required scope without explicit user confirmation.
- It is acceptable and expected to disagree with the project owner when the evidence supports it; the disagreement should be specific, sourced when possible, and framed around the product goal.

## Local And CI Build Parity

Local build and test commands are the primary development interface. GitHub Actions mirrors local commands.

Current repository posture: automatic GitHub Actions triggers are disabled while the repository is private. The CI workflow is kept as a manual `workflow_dispatch` entry point so checks can be run in GitHub only on demand.

Rules:

- Every default CI check must have a documented local equivalent.
- GitHub Actions should run the same commands as local development or thin wrappers around them.
- Avoid CI-only logic.
- Default CI must not require secrets or live external services.
- CI may use Linux for cost reasons, but local Windows development must remain supported.
- Because the GitHub repository is currently private, be especially conservative with GitHub Actions minutes, artifacts, and packaging jobs.

## Nix Development

Brawler uses Nix from the first scaffold.

Rules:

- `flake.nix` defines the canonical development environment.
- `nix develop` is the explicit environment entrypoint.
- `direnv`/`nix-direnv` may be used as optional convenience.
- WSL2 Ubuntu 24.04 is the primary development environment.
- Application build/test commands must work inside the Nix shell.
- GitHub Actions should run the same commands, preferably inside the same Nix shell when CI cost remains acceptable.
- Commit `flake.lock`.
- Do not store secrets in Nix files, `.envrc`, or derivations.
- Keep Nix packaging outputs minimal until packaging is a roadmap item.

## Agent Command Efficiency

Agents should use direct `rtk` commands with the locally installed WSL toolchain for the normal edit/test loop. This keeps token usage low while preserving `nix develop` and `make check` as the canonical reproducible path.

Rules:

- Prefer direct `rtk` commands for code search, focused file reads, frontend checks, Rust formatting/linting, and Rust tests.
- Prefer `rtk cargo nextest run` for Rust tests when available; use `rtk cargo test` as fallback.
- Avoid `rtk proxy` during normal work because it bypasses RTK filtering.
- Avoid full `make check` after every small change; run targeted checks first.
- Run `make check` for milestone closure, broad changes, and pre-commit confidence.
- If direct local tools disagree with the Nix environment, treat the Nix result as authoritative and update the docs/tooling decision if needed.

## Versioning And Releases

Brawler uses SemVer-style `0.x.y` versions from the first scaffold.

Initial version mapping:

- `0.1.0`: desktop shell, theme, health command
- `0.2.0`: SQLite/storage, companies, watchlists, sample feed
- `0.3.0`: inbox and company workspace
- `0.4.0`: notebooks and claims
- `0.5.0`: GPW adapter

Rules:

- Every completed milestone bumps the minor version before the milestone branch is handed back for commit/merge.
- Milestone closure must update the app version consistently in `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`.
- Patch versions are for fixes.
- Git tags mark meaningful build candidates.
- Public release automation waits until packaging is ready.
- A changelog starts once code exists.
- `1.0.0` requires stable enough behavior for external users.
