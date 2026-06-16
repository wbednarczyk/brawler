# Brawler Agent Contract

Brawler is a local-first investor newsfeed desktop app. This repository is run as a spec-driven project: documentation and contracts define intent before implementation.

## Three Always-On Rules

These three rules apply at all times, in every session, before anything else. This file (`AGENTS.md`) is loaded into context every session — treat it as read at all times.

1. **Token discipline.** Prefix every shell/file command with `rtk` (see [Tooling and Token Discipline](#tooling-and-token-discipline)).
2. **Doc-first.** This is a spec-driven repo: the docs are the source of intent, not a record to tidy up afterward. Before any non-trivial change, open and read the canonical doc(s) for the area (map in [Required Reading](#required-reading)) and implement to spec. Update the affected doc(s) in the same change. **Never invent or guess** architecture, scope, data shapes, field names, command names, scopes, or error codes — they are specified. If a spec is missing, ambiguous, or contradicted by reality, propose a doc/ADR change and confirm it before (or as part of) the code change; do not silently pick a design. Epic/milestone planning must update all relevant docs to capture the architecture decisions made, as part of completing the planning.
3. **Enforcement is a hard stop.** Brawler deliberately encodes its good practices, architecture, and posture as automated checks — type checks, ESLint/stylelint rules, guard/contract tests, the translation/pluralization/a11y guards, `engine-strict`, and the `check`/release gates. Their purpose is not only to catch bugs but to **halt an agent that is about to do the wrong thing** — especially one acting without consulting the user, or whose context is not enough to realize the change is wrong. A failing gate is a **stop-and-reconsider signal, never an obstacle to clear**: do not weaken, delete, skip, `--no-verify`, baseline-away, or loosen a check, rule, or assertion to make it pass, and do not work around it. If a gate looks wrong, surface it to the user and change the rule deliberately (with the doc/ADR update). The corollary: **when you add a capability or a decision, add the gate that keeps future changes — yours or another agent's — from silently violating it.** This is how the project stays coherent without relying on every agent having full context. See [ADR 0038](docs/adr/0038-enforcement-as-guardrails.md) and [project-practices.md](docs/project-practices.md).

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
- **Building or editing any frontend UI (components, screens, styling): [docs/ui-authoring.md](docs/ui-authoring.md) — compose from the `src/ui` primitives; never hand-roll a control, section, badge, row, or layout a primitive already provides, and do not use inline `style={{…}}`. Run the pre-write self-check in that guide before writing JSX.** Policy: [ADR 0037](docs/adr/0037-ui-component-framework-and-authoring-contract.md).
- Source adapters and source policy: [docs/source-strategy.md](docs/source-strategy.md) and source-specific ADRs.
- Build, test, CI, packaging, or local environment behavior: [docs/engineering-workflow.md](docs/engineering-workflow.md).
- Module ownership or refactoring: [docs/modularization-design.md](docs/modularization-design.md).
- Historical completed-card context only when needed: [docs/kanban-archive.md](docs/kanban-archive.md).

### Single Source Of Truth

Every fact has exactly one canonical home; update it there, do not duplicate it elsewhere (duplication is what causes drift):

- **Milestone intent + the active/upcoming plan** → [docs/roadmap.md](docs/roadmap.md) (forward-looking only).
- **Delivered/release history** → [CHANGELOG.md](CHANGELOG.md) (authoritative per-version) and [docs/kanban-archive.md](docs/kanban-archive.md) (completed-card detail). Not roadmap.
- **Live epic/task status and IDs** → Radicle/Radboard (`rad issue list --all`). [docs/kanban.md](docs/kanban.md) is only the thin pointer + label conventions; it does not carry milestone narrative or an epic list.
- **Commands/IPC** → contracts; **data shapes/DB/migrations** → data-model; **product behavior** → product-spec; **UI flows / IA** → ui-flows / ui-information-architecture; **architecture/boundaries + decisions** → architecture + ADRs; **source policy** → source-strategy; **build/test/CI** → engineering-workflow; **module ownership** → modularization-design.

## Tooling and Token Discipline

This applies to all agents working in this repo (Claude Code and Codex/ChatGPT alike).

- **Prefix shell and file commands with `rtk`** (Rust Token Killer): `rtk git`, `rtk grep`, `rtk read <file>`, `rtk ls`, `rtk cargo`, `rtk rad`, etc. It filters/compresses output before it reaches the model. `rtk proxy <cmd>` runs raw. Run `rtk trust` once in this repo so the project-local filters in `.rtk/` are applied.
- Use **repoctx** for compact repository context instead of opening many files.
- Read **targeted ranges** (line offsets, grep, search) rather than whole files; do not re-read a file you just edited.
- Reserve the strongest model for hard reasoning; prefer a cheaper model for routine edits, lookups, and synthesis.
- Avoid multi-agent fan-out unless the task genuinely needs breadth; reason directly when confident. Batch independent tool calls in one turn.

## Working Rules

- Work doc-first. The canonical docs (`docs/contracts.md`, `docs/data-model.md`, `docs/product-spec.md`, `docs/ui-flows.md`, `docs/ui-information-architecture.md`, `docs/architecture.md`, `docs/roadmap.md`, and the ADRs) are the source of intent and direction, not just records to tidy up afterward. Before non-trivial work in an area, consult the relevant canonical doc/contract first and ground the implementation in its intent — do not rely on reading the code alone. Reading the code is not a substitute for reading the spec; the docs encode intent the code can silently violate.
- Do not implement non-trivial changes without an explicit plan and approval.
- Start every new milestone or epic by breaking it into tasks and presenting all important architecture decisions to the user. Explain options and tradeoffs briefly, require explicit answers, and ask until the architecture is clear before implementation.
- Every epic/milestone planning step must update all relevant docs to capture the architectural decisions made during that planning, as part of completing the planning — not deferred to implementation or closure. When a planning decision changes durable architecture or policy, add or update an ADR; also propagate the decision into the affected canonical docs (contracts, data-model, product-spec, ui-flows, ui-information-architecture, architecture, roadmap) so the docs describe the agreed design before code is written. Radicle/Radboard issues track the work; the docs/ADRs record the decision.
- Agents may implement all approved milestone tasks, but must not close a milestone, move it to Done, or perform the milestone version bump until the user explicitly signs off on closure.
- Keep public behavior, contracts, and docs in sync with code changes. Update the relevant canonical docs as part of the same change that alters behavior, not in a later docs pass.
- Before non-trivial implementation and milestone closure, perform an ADR checkpoint: add or update an ADR when the work changes durable architecture or policy decisions, or explicitly confirm that existing ADRs already cover the decision.
- Milestone closure must include the matching app version bump in `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`.
- Milestone and patch closure must include a `CHANGELOG.md` update generated from the release workflow and reviewed for human-readable clarity before handoff.
- Milestone and feature completion require real working application behavior against the real local runtime, real source, real API, or real agent described by the milestone. Samples, mocks, seed data, fake endpoints, and placeholder providers are valid only as intermediate development steps and in automated tests. They are not sufficient to mark a feature or milestone complete unless the roadmap explicitly defines that work item as a mock/sample-only spike.
- If implementation evidence conflicts with a roadmap item or product requirement, explicitly call out the conflict, explain the tradeoff, and ask before weakening or deferring required scope.
- It is acceptable to challenge the user's proposed direction when technical, legal, source-policy, UX, cost, or reliability evidence suggests a better path, but the challenge must be communicated clearly before docs or code change the product commitment.
- Prefer small, reviewable changes that preserve local-first operation.
- Commit at meaningful checkpoints, not after every small step. Keep individual changes small and reviewable, but batch related work (a coherent slice plus its tests and docs) into one commit rather than creating many granular commits. Commit only when the user asks or at a natural milestone.
- Treat modularity, extensibility, pluggability, and configurability as first-class design constraints across the whole application. New features should expose provider/source/model/credential/configuration/collector/renderer/storage-operation boundaries that are easy to extend, while avoiding premature complexity that is not tied to a real planned extension.
- Treat very large source files as architecture debt. When working near a large UI, storage, command, or test file, prefer extracting cohesive modules as part of the feature slice instead of adding more unrelated responsibility to the same file.
- Frontend UI is **primitive-first**. Before writing JSX, run the pre-write self-check in [docs/ui-authoring.md](docs/ui-authoring.md): compose from the `src/ui` primitives (`SectionHeader`, `TextField`/`SelectField`, `StatusChip`/`StatusPill`, `ListRow`, `EmptyState`, `Modal`, …) instead of hand-rolling controls, section headers, badges, rows, or layouts; do not use inline `style={{…}}`. If a primitive is missing for a genuinely recurring shape, add and document one rather than inlining a bespoke version. This is policy ([ADR 0037](docs/adr/0037-ui-component-framework-and-authoring-contract.md)), and it is what keeps views coherent — incoherence comes from bypassing the framework.
- Migrations are append-only and immutable once applied. Never reuse a migration version number for different content across branches/sessions, and never edit a migration that has already shipped or been applied to any real database — the runner records the version and skips it, so edits silently never re-run (this caused a "no such table" production failure in `v0.40.0`). To change or repair already-applied schema/seed data, add a new forward migration that is idempotent and self-healing (`CREATE TABLE IF NOT EXISTS`, `INSERT OR IGNORE`/upserts, guarded `UPDATE`/`DELETE`) so it converges every database regardless of prior state. Make reads of newer settings/columns tolerate a missing row (safe default) so one absent migration never crashes startup.
- Do not add cloud services, telemetry, hosted dependencies, or paid APIs unless a new ADR approves them.
- Treat `Brawler` as the official application name.
- Preserve user privacy: watchlists, feed data, source history, AI outputs, and settings are local-only in v1.
- Prefer official, public, or RSS-based sources. Avoid fragile or restricted scraping unless a source-specific ADR approves it.
- AI output is decision support only. Do not phrase generated analysis as buy/sell/hold advice.
- Secrets must use the OS keychain in runtime code. `.env` is only for development and tests.
- Use strict Tauri permissions: typed commands only, no arbitrary shell execution, no broad filesystem access.
- Docs, ADRs, and contracts are canonical; Radicle/Radboard issues are active project tracking only.
- Radicle is the canonical project forge. Do not publish, seed, unblock public seeding, change visibility to public, or use `rad init` without `--private` unless the current task is an explicitly approved public-opening or publication operation.
- Agents may run `rad issue ...` commands unattended for normal planning, bug tracking, task creation, labeling, and solved-state updates. This permission does not extend to Radicle publication, seeding, visibility, identity, node, or repository initialization commands.
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

## Standing Agent Guidance

Durable agent guidance for this repository lives in the repository, written agent-neutrally so every agent (Claude, Codex/ChatGPT, and others) shares it. Record durable rules, preferences, and decisions in `AGENTS.md` (operating rules) or an ADR (architecture/policy decisions). Do not keep durable repository guidance only in an agent-private memory store; an agent's private memory is for that agent's own ephemeral or cross-repository notes, not for project rules other agents must follow.

- Do not add AI or agent attribution to commits, co-authors, or trailers (no "Claude", "Codex", "ChatGPT", "Generated by", etc.). Commit history stays authored by the human maintainer.
- AI-based features are async by default. Build the provider/IO layer and AI jobs async unless async genuinely does not fit a specific feature. This keeps the code ready for concurrency, streaming, and a future swap from the in-house provider library to official vendor SDKs. See [ADR 0028](docs/adr/0028-multi-provider-ai-boundary.md).
- Proactively analyze and speak up about architecture. When working in or near a module, surface concrete ways to improve modularity, flexibility, extensibility, and robustness — with options and tradeoffs — before implementing, rather than only executing the literal request. Verify premises (for example, confirm a dependency's existence and maintenance) before recommending it, and do not be needlessly conservative with dependencies when one genuinely makes sense.

## Testing Expectations

- Keep testing lean and fast. Prefer behavior and contract coverage over testing implementation details.
- Automated tests may use mocks, injected fetchers, and test samples to stay fast and deterministic, but agents must not present mock/sample success as proof that a user-facing feature or milestone is complete.
- Rust contracts, source adapters, deduplication, scheduler behavior, migrations, notebook workflows, transcription workflows, and AI mapping require automated tests.
- UI workflows for watchlists, feed filtering, unread state, source detail, and settings require component or workflow tests once UI exists.
- The UI must scale and remain usable (no global horizontal scrollbar, no clipped/overflowing controls, panes stack rather than clip, lists scroll internally) fluidly across the supported desktop window-size **range**, not just at fixed breakpoints. The range explicitly includes **tall, narrow windows ~960–1280px wide** (effective CSS px): the app is commonly run in roughly a quarter of a 49" 5120x1440 ultrawide via Windows 11 FancyZones, which — with zone margins and 100–125% OS scaling — yields a variable-width tall window, not a clean 1/4. Layouts must respond to the available width (e.g. stack a two-column grid when it no longer fits beside the sidebar) rather than assume a specific size. Verify with the browser UI tests' viewport matrix in `playwright.config.ts`, which samples this range (1366x768, 1920x1080, 1280x1440 at 100% scaling, and 1024x1152 for the same window at 125% scaling); the narrow Inbox test additionally asserts no horizontal overflow at 1008px. Add or adjust a sample viewport there when changing supported sizes.
- Desktop packaging changes require smoke tests for Tauri startup, Rust command availability, and local SQLite connectivity.
- Default CI must not require live external services or secrets. Use test samples and mocks for GPW, Gemini, SEC, Nasdaq, and media sources.
- Prefer the terms `test sample`, `sample data`, `seed data`, and `mock` in docs, UI text, and comments. Avoid `fixture` in project-facing language; if a conventional test path still uses `fixtures`, treat it as an internal implementation detail only.
- Keep GitHub Actions usage conservative: avoid larger runners, default macOS CI, scheduled workflows, and packaging on every push unless a later ADR approves them.
- Keep GitHub Actions usage conservative: avoid unnecessary minutes, artifact storage, and packaging jobs until public CI behavior is explicitly revisited.
- Every default CI check must have a documented local equivalent.
- Prefer verifying the Nix environment in CI only when it remains fast and within the GitHub cost posture.

## Repository Notes

The root `.agents/skills/` directory stores repository-owned, agent-neutral workflows. The root `.codex/skills/` directory may contain Codex-specific entrypoints that delegate to those shared workflows. This `AGENTS.md` file remains the primary repo-level instruction source.

<!-- repoctx:start -->
# repoctx

`repoctx` is an indexed code-navigation CLI. Use it for structural
questions about symbols, definitions, source windows, and file outlines.
Keep the repository's RTK-first workflow for broad text search, whole-file
reads, diffs, and build/test commands. The index is built incrementally
over Tree-sitter parses across nine languages (Go, Rust, TypeScript, TSX,
JavaScript, Python, JSON, YAML, TOML, Markdown).

Read commands auto-build the index on first run; nothing to set up.

## When to prefer which command

- **`/usr/local/bin/repoctx symbols <substring>`** — case-insensitive substring
  search across every indexed symbol. Use it to explore (`--kind`,
  `--lang`, `--limit` narrow the result set).
- **`/usr/local/bin/repoctx definition <name>`** — exact-name lookup limited to
  definition kinds (`function`, `method`, `class`, `interface`, `type`,
  `module`, `macro`, `constant`). Prefer over `symbols` when you know
  the identifier — answers the "where is X defined" question without
  field/variable noise.
- **`/usr/local/bin/repoctx context <symbol>`** — exact-name match plus the
  source window around each hit (`--context` lines either side, default
  5; `--limit` matches, default 3). One call returns "where + what",
  beating a `definition` + `Read` round trip.
- **`/usr/local/bin/repoctx outline <file>`** — document-symbol tree for a single
  file. Prefer over reading the whole file when you only need the
  structure.
- **`/usr/local/bin/repoctx status`** — file/symbol counts + staleness. Cheap
  one-line health check before deeper work.
- **`/usr/local/bin/repoctx gain`** — surface how many navigation tokens
  `repoctx` has saved across recent invocations.

## Output

All commands default to TOON for piped reads (token-efficient) and human
for TTYs. Pass `--json` when the caller is `jq` or `serde_json`.

Run commands from the repository root.

<!-- repoctx:end -->
