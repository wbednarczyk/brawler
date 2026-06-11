# Project Practices

This document captures day-1 operating rules for Brawler. It complements the product, architecture, and roadmap docs by defining how the project should be maintained.

Use [Project Brief](project-brief.md) for the full documentation map. Related references: [Architecture](architecture.md), [Modularization Design](modularization-design.md), [Engineering Workflow](engineering-workflow.md), [Release Workflow](release-workflow.md), [Roadmap](roadmap.md), and [Radicle/Radboard Tracking](kanban.md).

## Real Feature Completion

Brawler milestones are expected to end with real working product behavior.

Rules:

- Milestone and feature completion require the real application flow to work against the real local runtime, real source, real API, or real agent named by the milestone.
- Test samples, mocks, seed data, fake endpoints, and placeholder providers are allowed only as intermediate development steps and in automated tests.
- Test samples, mocks, seed data, fake endpoints, and placeholder providers are not enough to close a user-facing feature or milestone unless the roadmap explicitly says that item is a mock/sample-only spike.
- When real source/API/agent integration is unsafe, unreliable, unavailable, legally questionable, or too costly, agents must call out the conflict before weakening the scope and ask whether the roadmap should change.
- Every milestone closure should include real-use validation evidence appropriate to the milestone, such as a manual smoke test, packaged-app check, real source refresh, real API call, or real local workflow verification.
- Default automated tests should remain deterministic and secret-free; live checks are manual or opt-in when they require credentials or external services.

## Milestone Planning

Every new milestone starts with explicit task planning and architecture decisions before implementation begins.

Rules:

- At the start of each milestone, break the milestone into the concrete tasks needed to deliver it and record the active task breakdown in Radicle issues for Radboard.
- Radboard milestones are version targets such as `milestone:v0.25.0`; epics are major capability slices marked with the `epic` label; tasks are reviewable work slices linked to an epic with `parent:<epic-hex7>`.
- Radboard issue titles should be meaningful without milestone numbering. Use `epic: <capability>` for epic titles and plain action-oriented titles for tasks. Do not prefix titles with `E##`, `M##`, or similar numbering; version targeting belongs in `milestone:*` labels.
- Use repeated Radicle label flags, not comma-separated labels. For example: `--labels epic --labels milestone:v0.25.0 --labels area:research-workspace`.
- Present the important architecture decisions to the user before implementation. Keep each decision short, explain the practical options and tradeoffs, and require explicit user answers.
- Do not guess on architecture. If ownership boundaries, storage shape, provider model, security posture, UI placement, configuration, persistence, background-job behavior, observability, or release impact are unclear, ask until the decision is clear enough to implement.
- Architecture decisions must be settled before code changes for that milestone begin, except for small discovery spikes that are explicitly framed as research.
- Agents may implement all approved milestone tasks, but milestone closure is a separate manual signoff step. Do not move a milestone to Done, mark the roadmap status completed, or perform the milestone version bump until the user explicitly approves closure.
- After user signoff, milestone closure ends with Radicle/Radboard cleanup: after the version bump and final validation are complete, mark completed task issues and the completed epic as solved with `rad issue state --solved`. Do not use `--closed` for completed work; Radicle closed means abandoned or won't-fix.
- If the milestone description is missing something that would materially improve the application, its maintainability, or its user-facing workflow, propose it to the user instead of silently ignoring it.
- Once decisions are made, update roadmap, Radicle issues, contracts, architecture docs, or ADRs as needed before or alongside implementation so later agents inherit the decision.

## ADR Hygiene

ADRs record durable architecture and policy decisions. They should stay current as part of normal feature work, not as a separate cleanup project.

Rules:

- Before non-trivial implementation work, check whether the task changes a durable decision about storage, security, permissions, provider boundaries, source policy, AI behavior, observability, licensing, packaging, dependencies, or release workflow.
- If the task changes one of those decisions, add a new ADR or update the relevant accepted ADR before or alongside implementation.
- If the task only implements an already documented decision, no new ADR is needed.
- Milestone closure should include an ADR checkpoint: explicitly confirm that no ADR is needed, or list the ADRs added/updated.
- Prefer short ADRs that capture context, decision, and consequences over duplicating roadmap tasks or implementation details.
- Do not bury new durable decisions only in roadmap, Radicle issues, code comments, or commit messages.

## License And Public Posture

Brawler uses an open-core public posture. The desktop core is licensed under MPL-2.0. Public license, contribution, publication, and private-operations decisions must be recorded in ADRs and kept synchronized with the private sibling repository policy in [ADR 0023](adr/0023-public-private-documentation-split.md).

Rules:

- Public docs must avoid unnecessary personal infrastructure detail, owner-only strategy, private key paths, raw license-token operations, and speculative monetization experiments.
- Owner-only strategy and operations should live in `../brawler-private` when available locally.
- Agents may read `../brawler-private` for owner context, but must not copy private-repo content into public docs unless explicitly asked.
- Accept outside contributions only through the public contribution policy.
- Do not publish public release artifacts until the public-opening ADR, license file, contribution policy, security policy, and repository audit are complete.
- License private signing material must never be stored in this repository, app database, logs, exported settings, Nix files, `.envrc`, GitHub Actions secrets, or plaintext private Git repositories unless a future ADR explicitly approves encrypted secret storage.
- The local license/entitlement module should remain extensible for future gated features and official entitlements, but the open desktop core must not rely on a license token for normal use.

## Secrets And Config

Secrets and settings have separate sources of truth.

Rules:

- API keys and provider secrets live in the OS keychain.
- Credential handling should use a reusable typed boundary for provider, purpose, and secret kind so future API keys, username/password credentials, session tokens, and other secret forms can be added without changing the UI or storage model from scratch.
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

V1 observability is local-only. There is no telemetry, remote error reporting, remote log shipping, hosted metrics, or hosted tracing.

Rules:

- Sources screen shows adapter and job errors.
- Developer diagnostics are structured, SQLite-backed, visible only in Developer mode, and recorded only while Developer mode is enabled.
- Runtime logs are a separate append-only local file framework with rotation and conservative log-level defaults.
- Metrics are local operational health signals, not product analytics or user behavior tracking.
- Logs, diagnostics, and metrics must not include API keys, full prompts, full source bodies, full transcript text, raw provider responses, license private material, or full license secrets by default.
- Logs may include IDs, source URLs, statuses, timestamps, and error classes when doing so is useful and not private.
- Diagnostic summary copy/export should be redacted and user-triggered.
- OpenTelemetry-compatible naming/structure is acceptable when cheap, but do not add OpenTelemetry dependencies, exporters, or compatibility-only code unless a later implementation proves the overhead is low.
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

## Modularity And Configurability

Modularity, extensibility, pluggability, and configurability are first-class design constraints across the whole application.

Rules:

- Keep business logic in backend/domain boundaries. React should remain a presentation and thin-controller layer that renders backend read models, captures user intent, and calls typed commands; it should not own cross-domain aggregation, review semantics, entitlement decisions, source matching, import/export planning, provider behavior, or other durable product rules.
- Before non-trivial implementation work, identify the owning domain and layer using [Modularization Design](modularization-design.md).
- New features should define clear boundaries for providers, sources, credentials, models, settings, collectors, renderers/exporters, storage-facing operations, and user-visible workflow options.
- Design modules around stable internal contracts so future implementations can be added as adapters or plugins where a real extension path is plausible.
- Apply this principle to every domain, not only obvious provider/source systems: UI surfaces, commands, jobs, storage access, observability, settings, credentials, import/export, packaging, licensing, and future sync should all keep logical extension points explicit.
- Prefer reusable typed configuration surfaces over one-off hard-coded provider/source behavior.
- Keep defaults practical and conservative, but make likely future provider/source/model changes configurable when doing so is cheap and clear.
- Avoid abstracting for hypothetical futures that are not connected to the roadmap, contracts, or an explicit user requirement.
- During feature implementation, explicitly check whether any new or changed user action should become a shortcut action. Repeated workflow actions, keyboard-first research actions, and high-frequency commands should be registered in the shortcut framework with a default binding or an explicit decision that no shortcut is needed.
- Shortcut-capable actions must still remain available through visible UI controls, and shortcut defaults must avoid common browser, OS, and text-editing conflicts. User-configurable shortcut bindings should be used instead of hard-coded key handlers.
- When a feature introduces a real external dependency, separate the runtime implementation from test-sample/mocked implementations so tests stay deterministic and the real workflow can still be validated.
- Keep source files split by responsibility before they become hard to reason about. Large files should be treated as architecture debt, especially UI shells, storage modules, command registration, and broad test files.
- Prefer extracting cohesive modules during nearby feature work instead of doing disruptive repo-wide rewrites. A good extraction has a clear owner boundary, such as transcript UI, settings UI, notebook UI, storage migrations, storage transcript operations, source adapter state, or provider clients.
- When touching existing UI, adopt shared components/hooks for the touched area when they preserve behavior and class semantics.
- Do not split cohesive files only to reduce line count; split when a file gains multiple reasons to change or starts mixing layers.

## UX Quality

Intuitive UX and responsive UI are first-class project requirements, not polish to add at the end.

Rules:

- Prefer workflows that make the likely next action obvious.
- Prefer direct row interaction for list/detail workflows. When a row opens more context, clicking the row should expand details inline near that row, and clicking the same row again should collapse it when that behavior is natural.
- Avoid adding explicit row-level buttons for primary open/inspect behavior when the whole row can safely be the target. Keep buttons for secondary actions such as delete, source links, or explicit state changes.
- Mutating actions should provide quick visual feedback.
- App-owned user objects should have an explicit delete/remove workflow unless deletion does not make product sense or a feature contract explicitly excludes it. Delete behavior must be typed, confirmed when destructive, and must clean up dependent links or memberships without deleting unrelated canonical objects.
- Buttons and controls should communicate intent through position, label, icon, color, and state.
- Multi-panel workspaces should expose visible, keyboard-accessible resize handles for important column/pane widths unless the layout is intentionally fixed and that decision is documented.
- Dense investor workflows must remain scannable and keyboard/mouse efficient.
- Temporary UX shortcuts are allowed during early scaffolding, but known UX debt should be recorded in docs or Radicle/Radboard.
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
- Real Gemini YouTube transcription is required for M10 closure, but the live smoke path is manual or opt-in and must not become part of default CI/local checks.
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

## Radicle/Radboard Workflow

Brawler uses a hybrid workflow.

Rules:

- Docs, ADRs, and contracts are canonical for product and architecture decisions.
- Radicle issues rendered by Radboard are the active planning board.
- Radboard milestones track app version targets. Radboard epics track major capability slices. Radboard tasks track reviewable implementation work.
- Deferred bugs are tracked as Radicle issues so they are visible in Radboard. If a bug is reported or discovered and will not be fixed immediately in the current work, create a bug issue before moving on.
- Bug issues use the plain `bug` label plus `state:*`, `priority:*`, and an `area:*` label when the owning area is clear. Add `milestone:v0.x.0` when the fix is targeted for a version.
- Bugs discovered inside an active epic may also use `parent:<epic-hex7>` so they appear under the relevant epic. Otherwise, keep them as standalone bug issues.
- If a deferred bug blocks active work, add `blocked:<bug-hex7>` to the blocked task or epic after creating the bug issue.
- Bugs fixed immediately in the same work item do not require a separate Radicle issue, but the final summary should mention the bug and the verification performed.
- PRs or patches should reference a Radicle issue when implementation begins.
- Behavior changes must update docs/contracts in the same PR.
- Important decisions must not live only in Radicle issue, patch, or PR comments.

## Product Scope And Tradeoff Communication

Agents are expected to exercise engineering judgment, including pushing back when evidence suggests the current path is unreliable, too costly, legally risky, or poor UX. That pushback must be explicit and collaborative.

Rules:

- Do not silently weaken, defer, or remove a product requirement because implementation is difficult.
- If a planned implementation path looks unreliable, explain the evidence and propose alternatives.
- If a user proposal conflicts with roadmap, contracts, ADRs, source policy, privacy, security, or cost posture, call out the conflict before implementing.
- When a required feature has a risky implementation path, keep the requirement intact and discuss alternate paths rather than making the feature optional.
- Docs may record uncertainty, fallback options, and technical risk, but they must not downgrade required scope without explicit user confirmation.
- It is acceptable and expected to disagree with the project owner when the evidence supports it; the disagreement should be specific, sourced when possible, and framed around the product goal.

## UI Regression Guardrails

UI polish work should leave executable guardrails behind when it fixes repeated layout or copy regressions.

Rules:

- Normal user-facing UI must avoid implementation and architecture terms such as SQLite, Tauri, adapter, schema, database, module, collector, and local/Local. Use product language instead.
- Developer-only Diagnostics may use implementation terms when Developer mode is explicitly enabled.
- Source-provided content and URLs may contain arbitrary text, but test samples used in normal UI tests should not accidentally include forbidden terms.
- Screens with fixed app chrome should keep navigation, top bar, and screen-level headers/filters visible while the primary content area scrolls internally.
- Companies, Watchlists, Notebooks, Inbox, and Events need automated scroll/layout contract coverage when their layout CSS changes.
- Cross-screen navigation affordances, such as company links and watchlist membership links, should have workflow tests when they are added or moved.
- Prefer shared renderers for repeated visual concepts, especially tickers, memberships, status pills, and source links, and add tests when a shared renderer is adopted across screens.
- Browser UI smoke tests should be added or updated when jsdom/CSS contract tests cannot realistically catch a repeated layout regression, especially fixed chrome, global scrollbar, panel scrolling, row sizing, or viewport-specific behavior.
- Playwright browser smoke is opt-in at first. Do not add it to default `make check` or default CI until the suite is stable and explicitly promoted.

## Local And CI Build Parity

Local build and test commands are the primary development interface. GitHub Actions mirrors local commands.

Current repository posture: automatic GitHub Actions triggers remain conservative during the public-opening transition. The CI workflow is kept as a manual `workflow_dispatch` entry point until the project owner explicitly revisits public CI behavior.

Rules:

- Every default CI check must have a documented local equivalent.
- GitHub Actions should run the same commands as local development or thin wrappers around them.
- Avoid CI-only logic.
- Default CI must not require secrets or live external services.
- Live provider smoke checks, including Gemini transcription checks, must be documented and opt-in because they require credentials and external service availability.
- CI may use Linux for cost reasons, but local Windows development must remain supported.
- Keep GitHub Actions minutes, artifacts, and packaging jobs conservative until public CI behavior is explicitly revisited.

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

Brawler uses SemVer-style `0.x.y` versions from the first scaffold. The detailed release workflow lives in [Release Workflow](release-workflow.md).

Initial version mapping:

- `0.1.0`: desktop shell, theme, health command
- `0.2.0`: SQLite/storage, companies, watchlists, sample feed
- `0.3.0`: inbox and company workspace
- `0.4.0`: notebooks and claims
- `0.5.0`: GPW adapter

Rules:

- New commits use Conventional Commits and should pass the repo-local `commit-msg` hook.
- Before milestone or epic closure, the project owner commits all feature, fix, refactor, test, and documentation work. Closure starts from committed feature history, not from uncommitted working-tree changes.
- Every completed milestone bumps the minor version during closure after manual user signoff.
- Milestone closure must update the app version consistently in `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`.
- Milestone and patch closure must update `CHANGELOG.md`. Agents own running changelog generation from committed Git history, reviewing/editing the generated entry for clarity, and including the changelog update in the release commit.
- After an epic or milestone wrap-up that includes a version bump, agents create the final release commit with `chore(release): bump version to x.y.z`.
- Radicle/Radboard cleanup is part of closure after the version bump and final validation: mark completed tasks and the completed epic solved, leaving abandoned work closed only when it is intentionally won't-fix.
- Patch versions are for fixes.
- Git tags mark meaningful build candidates. Historical release tags may be created after auditing exact release commits; old commit messages are not rewritten.
- Public release automation waits until packaging is ready.
- `CHANGELOG.md` records release history. Entries through `0.24.1` are curated from [Kanban Archive](kanban-archive.md); future entries are generated with `git-cliff` from Conventional Commits and may be edited for clarity.
- `1.0.0` requires stable enough behavior for external users.
