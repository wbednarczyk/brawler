# Brawler — Claude agent contract

Brawler is a local-first investor newsfeed desktop app. This is a **spec-driven** repository: docs and contracts define intent before implementation. This file is the canonical agent contract ([ADR 0063](docs/adr/0063-claude-native-context-architecture.md)); `AGENTS.md` is a pointer stub. Each rule is stated once — here or in its canonical doc — and pointered elsewhere.

## Three Always-On Rules

Apply at all times, in every session state, before anything else.

### 1. Token discipline

Prefix every shell/file command with `rtk` (`rtk git`, `rtk grep`, `rtk read`, `rtk ls`, `rtk cargo`, `rtk npm`, `rtk rad`, `rtk make`, `rtk npx playwright`) — it compresses output before it reaches context. `rtk proxy <cmd>` runs raw; avoid for normal work. Run `rtk trust` once per checkout so `.rtk/` filters apply. Use **repoctx** for structural code questions (definition, callers, outline, impact, changed) instead of grep/find/whole-file reads — full reference in the `repoctx` skill; fall back to `rtk grep` for prose/string-literal scans. Read targeted ranges; never re-read a file just edited. Reserve the strongest model for hard reasoning; prefer cheaper models for routine edits/lookups. Batch independent tool calls in one turn. Subagent fan-out only for genuine breadth — every subagent uses rtk + repoctx too.

### 2. Doc-first

Before any non-trivial change, open and read the canonical doc(s) for the area (map in [Required Reading](#required-reading)) and implement to spec; update the affected doc(s) **in the same change**, not a later pass. **Never invent or guess** architecture, scope, data shapes, field names, command names, scopes, or error codes — they are specified. If a spec is missing, ambiguous, or contradicted by reality, propose a doc/ADR change and confirm it before (or as part of) the code change; do not silently pick a design — reading the code is not a substitute for reading the spec. Epic/milestone planning must update all relevant docs (and add/update ADRs for durable decisions) as part of the planning, not deferred to implementation.

### 3. Enforcement is a hard stop

Brawler encodes its practices as automated checks (types, lints, guard/contract tests, translation/a11y guards, the `check` gates) whose purpose is to **halt an agent about to do the wrong thing** — especially one whose context is too small to realize the change is wrong. A failing gate is a stop-and-reconsider signal, never an obstacle to clear: do not weaken, delete, skip, `--no-verify`, baseline-away, or loosen a check to make it pass, and do not work around it. If a gate looks wrong, surface it to the user and change the rule deliberately (with the doc/ADR update). Corollary: adding a capability or decision means adding the gate that keeps future changes from silently violating it. See [ADR 0038](docs/adr/0038-enforcement-as-guardrails.md).

## Required Reading

Mandatory in every session state — new, resumed, compacted, large-context — above all other docs: **this file** and **[docs/engineering-workflow.md](docs/engineering-workflow.md)** (build/test/toolchain discipline, the test-driven loop, and the [Definition of Done](docs/engineering-workflow.md#definition-of-done-the-handover-gate) — the stop-gate to run and report against before claiming "done"; never hand over partial). If a session has drifted, re-ground in these two before acting.

Then load only what the task needs:

- [docs/project-brief.md](docs/project-brief.md) — product-intent detail (markets, source priorities, open-core posture); the digest lives in [Product Intent](#product-intent) below, and this area map is the documentation map.
- The active Radicle issue: `rad issue show <hex7>` ([docs/kanban.md](docs/kanban.md) is the pointer; `rad issue list --all` for the board).
- Implementing a planned milestone task: the per-milestone execution plan in [docs/plans/](docs/plans/) (start at its README — non-normative; ADRs/canonical docs win on conflict).
- Area docs: architecture/boundaries → [docs/architecture.md](docs/architecture.md) + [docs/adr/](docs/adr/) · commands/IPC → [docs/contracts.md](docs/contracts.md) · data/DB/migrations → [docs/data-model.md](docs/data-model.md) · product behavior → [docs/product-spec.md](docs/product-spec.md) · UI flows/IA → [docs/ui-flows.md](docs/ui-flows.md) / [docs/ui-information-architecture.md](docs/ui-information-architecture.md) · sources → [docs/source-strategy.md](docs/source-strategy.md) · module ownership → [docs/modularization-design.md](docs/modularization-design.md) · tests → [docs/testing.md](docs/testing.md) · completed-card history (rarely) → [docs/kanban-archive.md](docs/kanban-archive.md).
- **Any frontend UI work: [docs/ui-authoring.md](docs/ui-authoring.md) first** — primitive-first ([ADR 0037](docs/adr/0037-ui-component-framework-and-authoring-contract.md)): compose from `src/ui` primitives; never hand-roll a control, section, badge, row, or layout a primitive provides; no inline `style={{…}}`. Run the pre-write self-check before writing JSX.
- Milestone/release closure → `brawler-release` skill. Packaging → `packaging` skill.

## Single Source Of Truth

Every fact has exactly one canonical home; update it there, do not duplicate it elsewhere (duplication is what causes drift):

- **Milestone intent + the active/upcoming plan** → [docs/roadmap.md](docs/roadmap.md) (forward-looking only; deferred scope's one home: roadmap *Not In V1*).
- **Delivered/release history** → [CHANGELOG.md](CHANGELOG.md) (authoritative per-version) and [docs/kanban-archive.md](docs/kanban-archive.md) (completed-card detail); never normative.
- **Live epic/task status** → Radicle/Radboard (`rad issue list --all`).
- **Commands/IPC** → contracts; **data shapes/DB/migrations** → data-model; **product behavior** → product-spec; **UI flows/IA** → ui-flows / ui-information-architecture; **architecture/boundaries + decisions** → architecture + ADRs; **source policy** → source-strategy; **build/test/CI** → engineering-workflow; **test strategy** → testing; **module ownership** → modularization-design.
- **Decision rationale, rejected options, investigation evidence** → ADRs (normative); execution chronicle → CHANGELOG/kanban-archive (never normative).

## Product Intent

Brawler is a personal investor newsfeed workspace: one place where an individual investor follows public companies — watchlists, official reports (GPW ESPI/EBI first; US/EU adapters later without changing the core feed model) plus allowed RSS media, a per-ticker notebook with notes that preserve their origin (a claim traces back to its report/article/transcript), management-claim tracking, and AI decision support. **V1 is not a portfolio tracker, trading tool, or recommendation engine.** Source attribution stays visible and durable; the ticker-based UI stays simple while storage stays collision-safe; dark theme is the default. Detail (markets, source priorities, open-core posture): [docs/project-brief.md](docs/project-brief.md).

## Working Rules

Process:

- Do not implement non-trivial changes without an explicit plan and approval. Start every milestone/epic by breaking it into tasks and presenting the architecture decisions with options and tradeoffs; ask until the architecture is clear.
- **A capability is not done until a user can reach it.** Every new IPC command ships with its UI entry point in the same slice (or is documented headless-only). Enumerate the user-facing usage scenarios (enable, run, review, undo, configure, error/empty states) first, then build to them. `knip` flags orphaned `src/api` exports; clear them before calling a feature's UI complete. Closure-audit version: [Definition of Done §I](docs/engineering-workflow.md#definition-of-done-the-handover-gate).
- Agents may implement approved milestone tasks, but never close a milestone, move it to Done, or bump the version without explicit user sign-off.
- Milestone/feature completion requires real working behavior against the real runtime/source/API named by the milestone. Mocks, samples, and placeholder providers are valid only as intermediate steps and in automated tests — never completion evidence.
- If implementation evidence conflicts with a roadmap item or product requirement, call out the conflict and ask before weakening or deferring scope. Challenge the user's direction when technical/legal/source-policy/UX/cost/reliability evidence suggests a better path — before docs or code change the commitment.
- Prefer small, reviewable changes. Commit at meaningful checkpoints (a coherent slice + tests + docs), only when the user asks or at a natural milestone. Never commit or push unattended.
- **After implementing a milestone, write a retrospective before closure sign-off**: both domains (app + development loop) × what went well / what went wrong (especially unexpected gaps) / what to stop / what to improve; mark each item closed or still-open honestly — the human decides what needs action. Feed still-open items into the guardrail-harvest loop.
- **Guardrail harvest (mandatory feedback loop).** When a defect is flagged — by the user, a review, a gate, or your own noticing — fixing the instance is not enough: convert the **class** into a durable guardrail in the same change (a precise automated gate when cleanly detectable, otherwise a documented rule + checklist line). Never add a broad gate that flags legitimate code. Put the guardrail where every agent reads it, not in private memory. Ritual: the `guardrail-harvest` skill; policy: [ADR 0045](docs/adr/0045-guardrail-harvest-loop.md).
- Create a Radicle issue for every reported/discovered bug not fixed immediately (`bug` + state/priority/area labels; link `parent:<epic-hex7>` / `blocked:<bug-hex7>`).
- Release/closure runs through the `brawler-release` skill and **`make release VERSION=x.y.z`** — never hand-assemble the bump/changelog/commit/tag/push (the target aborts if pre-bumped). Curated releases: `make release-prepare` → curate → `make release`. Updating `wiki/` for every user-facing change is a required release-prep step.

Architecture and design:

- Treat modularity, extensibility, pluggability, and configurability as first-class constraints: expose provider/source/model/credential/collector/renderer/storage boundaries that are easy to extend, without premature complexity. Surface concrete architecture improvements (options + tradeoffs) when working near a module; verify premises (e.g. a dependency's maintenance) before recommending.
- Treat very large source files as architecture debt: extract cohesive modules as part of the feature slice.
- AI-based features are async by default (provider/IO + AI jobs), ready for concurrency/streaming/SDK swaps — [ADR 0028](docs/adr/0028-multi-provider-ai-boundary.md).
- **Keep non-trivial work off the UI thread**: any command doing meaningful CPU work or blocking IO is an `async fn` offloading via `tauri::async_runtime::spawn_blocking`; read persisted derived indexes instead of recomputing the corpus per call. Checklist: [Definition of Done §C](docs/engineering-workflow.md#definition-of-done-the-handover-gate).
- Migrations are **append-only and immutable once applied**: never reuse a version number for different content, never edit a shipped migration (the runner skips it silently). Repair via a new forward, idempotent, self-healing migration; reads of newer settings/columns tolerate a missing row with a safe default. Rules: [docs/data-model.md](docs/data-model.md).
- Keep runtime dependency additions conservative and justified, but not needlessly so when one genuinely fits.

Product and policy:

- Treat `Brawler` as the official app name. Watchlists, feed data, source history, AI outputs, and settings are local-only in v1.
- No cloud services, telemetry, hosted deps, or paid APIs without an approving ADR.
- Prefer official/public/RSS sources; no fragile/restricted scraping without a source ADR.
- AI output is decision support only — never phrase analysis as buy/sell/hold advice.
- Secrets use the OS keychain in runtime code; `.env` is dev/tests only. Strict Tauri permissions: typed commands only, no arbitrary shell, no broad FS access.
- Radicle is the canonical forge. `rad issue ...` commands may run unattended for planning/tracking; publication, seeding, visibility, identity, node, and `rad init` operations may not (`rad init` is always `--private` unless an approved publication task). Label conventions: [docs/kanban.md](docs/kanban.md).
- The private sibling `../brawler-private` (when present) is readable for owner-only context; never copy its content into this public repo unless explicitly asked.
- Local build/test commands are primary; GitHub Actions mirrors them, staying conservative (no larger runners, no default macOS, no scheduled workflows; every CI check has a documented local equivalent). Nix from the first scaffold; no secrets in Nix files/`.envrc`.
- Windows hands-on testing is a separate runtime validation path (WSL has no GUI; a WSL Tauri build is a Linux app).

## Testing Expectations

Canonical strategy/layers: [docs/testing.md](docs/testing.md). Which-test-where map + the single mandatory gate (`make check`, [ADR 0062](docs/adr/0062-mandatory-test-gate-and-test-driven-loop.md)): [docs/engineering-workflow.md](docs/engineering-workflow.md). Non-negotiables:

- **Every behavior has a test that reddens when it breaks** — every command/contract, read model, UI workflow, migration, adapter, job, and fixed regression; "hard to test" is not a reason to skip. Keep suites lean: behavior/contracts, one good test per behavior.
- Tests may use mocks/samples to stay fast, but mock success is never proof a user-facing feature is complete (see Working Rules).
- **Validate against the maintainer's real database** (gitignored `private/realdata/`, see its README) for every new feature — for any similarity/dedup/clustering/matching/ranking feature, real-data validation with a hand-labeled ground-truth set **precedes** committing to an approach ([docs/testing.md](docs/testing.md)).
- Data transforms ship `proptest` invariants + golden `insta` snapshots; new hot paths ship a behavioral scale gate; new IPC commands join the dual-execution mock-fidelity corpus ([ADR 0049](docs/adr/0049-test-architecture-v2-data-transform-correctness.md), details in testing.md).
- The UI must stay usable across the supported window range, including tall narrow ~960–1280px windows (quarter of a 49" ultrawide with FancyZones at 100–125% scaling): no global horizontal scrollbar, panes stack rather than clip, lists scroll internally. Verified by the viewport matrix in `playwright.config.ts`; adjust it when changing supported sizes.
- Default CI needs no live services/secrets. Prefer `test sample` / `sample data` / `seed data` / `mock` in project-facing language (not `fixture`).

## Claude-Native Ecosystem

- Repository-owned workflows are skills under `.claude/skills/` (`repoctx`, `brawler-release`, `guardrail-harvest`, `packaging`) — loaded on demand; invoke them instead of re-deriving the mechanics.
- The session hook (`.claude/hooks/session-context.sh`, all four SessionStart matchers) re-grounds the always-on rules after start/resume/clear/compact. Gate-integrity enforces this file's and the hook's byte budgets and parity markers ([ADR 0063](docs/adr/0063-claude-native-context-architecture.md)).
- Durable rules/decisions live in this repo (this file, ADRs, canonical docs) — not agent-private memory. No AI/agent attribution in commits, co-authors, or trailers; commit history stays authored by the human maintainer.

<!-- repoctx:start -->
## Code navigation with `repoctx`

Prefer `repoctx` over `grep`/`find`/wholesale `Read` for structural
questions about this repo. The `repoctx` skill at
`.claude/skills/repoctx/SKILL.md` carries the full command reference
and choose-the-right-tool guidance.

Quick cues:

- "Get oriented in this repo" → `repoctx overview`
- "Where is X defined?" → `repoctx definition X`
- "Show me X and its surrounding code" → `repoctx context X`
- "Explore symbols matching ..." → `repoctx symbols <substring>`
- "Find X everywhere (defs + textual, incl. comments)" → `repoctx search X`
- "Who calls X / what does X call?" → `repoctx callers X` / `repoctx callees X`
- "Trace the call chain from X" → `repoctx callgraph X --depth N --direction up|down|both`
- "What breaks if I change X?" → `repoctx impact X`
- "What does this branch change + its blast radius?" → `repoctx changed --since main`
- "Find dead code / call cycles" → `repoctx deadcode` / `repoctx cycles`
- "What does this file import / what imports module M?" → `repoctx deps <file>` / `repoctx rdeps <module>`
- "Does layer A import layer B?" → `repoctx boundary --from <path> --to <module>`
- "Circular imports / module build order?" → `repoctx import-cycles` / `repoctx modules`
- "Structure of one file" → `repoctx outline <file>`
- "Index health" → `repoctx status`

All read commands auto-index on first run. Default output is TOON for
pipes (token-efficient) and human for TTYs; pass `--json` when piping
into `jq`. Working tree: `/home/wojtas/projects/brawler`.

<!-- repoctx:end -->
