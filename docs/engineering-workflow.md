# Engineering Workflow

How Brawler is built, checked, and tested during development.

Doc map: [CLAUDE.md](../CLAUDE.md) § Required Reading. Related: [Testing](testing.md), [Release Workflow](release-workflow.md), [Roadmap](roadmap.md), [Kanban](kanban.md), [ADR 0007](adr/0007-github-build-and-lean-testing.md).

## Posture

- Local build/test commands are the primary dev interface; GitHub Actions mirrors them — every CI check has a documented local equivalent. Nix (`flake.nix`, `nix develop`) is the reproducible environment from scaffold.
- Automated tests protect important behavior, avoiding bloat that slows iteration; every milestone is demoable and CI-checkable.
- App target: Windows first, cross-platform preserved (OS split: next section). Environment differences (e.g. `CI=true`) are explicit.

## WSL And Windows Runtime Split

WSL2 Ubuntu 24.04 with Nix is the primary development layer; Windows 11 is the primary hands-on runtime target — an intentional split: WSL/Nix owns automated checks, builds, tests, CI-equivalent validation; Windows owns clickable desktop sanity, native Tauri window/OS behavior (dialogs, keychain), packaging checks, subjective UX review.

Do not assume a Linux GUI inside WSL — a WSL Tauri build is a Linux application, not a Windows executable.

**Disk hygiene.** WSL2's grow-only vhdx means a full host drive kills sessions while `df` inside WSL shows free — `disk-guard` (first `check`/`check-local` step) watches WSL root + the vhdx/swap-hosting drives (`Lxss` autodetect; fail <10 GiB, warn <40) and names the remedy: `make disk-clean` / `disk-clean-deep`; vhdx shrink is host-side only (`wsl --shutdown` + `--set-sparse true`).

Recommended workflow:

- `make check` before opening a PR; `make build` to validate frontend production output; `make frontend-preview` for a quick browser layout check from Windows (no Tauri APIs).
- `make smoke-gemini-transcript`/`smoke-keyring`: opt-in live smoke tests needing local credentials/OS state.
- Runtime logs: local JSON Lines under the app-data logs dir, level/rotation via Settings or `BRAWLER_LOG_*` env vars. Local metrics: Developer-mode-only Diagnostics snapshots, not telemetry.
- Work lands via PRs (`gh pr create`); `make install-git-hooks` wires the local `commit-msg` hook. Release model + guardrails: [Release Workflow](release-workflow.md).
- Native Windows checkout/worktree for hands-on testing; `scripts/windows/dev.ps1` starts Tauri dev mode there. Packaging paths: the `packaging` skill.

## Nix Development Environment

Brawler uses Nix from the first scaffold: `flake.nix` is canonical, `nix develop` the explicit entrypoint (optional `direnv`). Nix provides toolchains, not a command hiding place — build/test commands stay runnable inside `nix develop`. Secrets stay outside the Nix store (never in `flake.nix`/`flake.lock`/`.envrc`); commit `flake.lock`. Flake provides Rust + fmt/lint, Node.js/npm, Linux Tauri prerequisites, SQLite dev libs, `pkg-config`, plus `devShells.windows-cross` (`packaging` skill). CI runs the same `make` targets inside `nix develop`.

## CI Posture

Public repo, free Actions minutes ([ADR 0090](adr/0090-github-canonical-forge-and-continuous-release.md)) — **the PR's required checks are the ONLY gate** ([ADR 0096](adr/0096-quality-gate-architecture-under-continuous-release.md)); nothing post-merge blocks shipping. One full gate, one workflow: **`full-check.yml`** on `pull_request` (required checks, per-PR cancellation) + `workflow_dispatch`; no "fast" mode in CI. Required alongside it: **`Frontend coverage ratchet`** (`make coverage-frontend`, floor 80.0%) and **`Rust coverage ratchet`** (`make coverage-rust`, floor 86.5%), both vs `coverage-baseline.json`. A labeled merge skips the re-run (ADR 0090: up-to-date + green ⇒ master ≡ PR tree); `release.yml` builds at once, in parallel. Label gate: **`release-label.yml`** — sole `labeled`/`unlabeled` listener; ~4s, never the full gate. **Makefile everywhere:** every job runs one granular `make check-*` target in `nix develop` (job↔target list: Command Reference) — CI and local runs are identical; `gate-integrity` asserts every `run:` step matches `make <target>`. Windows is first-class on code PRs: `windows-build` + the real-.exe boot smoke `windows-boot-smoke` (#206).

**Paths-filter:** a docs-only PR runs `check-docs-gates`+`check-commits` only (skipped jobs still satisfy required checks). Setup uses a ghcr.io devshell image (rebuilt on `flake.lock` change), `install-nix-action` the fallback; **`mutation-audit.yml`** (renamed from `mutants.yml`) auto-triggers on monitored-risk-path `master` pushes (ADR 0096 dec. 5) + manual dispatch — advisory, never blocking. Standard runners only; no macOS/scheduled; secret-free. **Master always green (server-side):** the ruleset requires the `full-check` jobs green **and** the branch up-to-date before merge (merge tree = tested tree); bisect merge history with `git bisect --first-parent`.

## Local Developer Commands

The Makefile is the preferred local command surface from WSL; targets stay thin wrappers around documented project commands.

**Only the `commit-msg` hook remains** ([ADR 0096](adr/0096-quality-gate-architecture-under-continuous-release.md)) — `pre-commit`/`pre-push` are deleted: a local commit ships nothing under continuous release, so nothing local needs to gate it. Rules: Conventional Commits, single `[a-z0-9._-]+` scope, no subject-length limit (ADR 0090). Pre-validate: `scripts/release/validate-commit-message.sh --message "<subject>"`; CI's `commit-lint` re-checks every commit. `--no-verify` is WIP-only, never valid under "done".

**What runs where** (`make check-local`, renamed from `check-fast`, is the inner loop + pre-handover [DoD](#definition-of-done-the-handover-gate) step — invoked deliberately, never hook-triggered; docs-only uses `make check-docs`; multi-phase epics gate each phase on `check-local`, the matrix runs in the PR's CI):

| When | Command | Where |
| --- | --- | --- |
| Every commit | `commit-msg` validation (ms) | Local hook |
| Before handover / inner loop | `make check-local` | Local, on demand |
| Data/extraction work (advisory) | `realdata-gt-score` / `realdata-extraction-check` / `realdata-honesty-check` / `make live-cycle` | Local, on demand |
| Every PR (required checks) | `make check` composition + both coverage ratchets | CI only |
| Risk-path pushes to `master` | mutation audit (advisory) | CI only |
| Never locally | full gate, coverage, mutants, bench audit | CI only |

## Test-Driven Development Loop

Brawler is **spec-driven for intent** (docs/ADRs define behavior before code) and **test-driven for the loop** ([ADR 0062](adr/0062-mandatory-test-gate-and-test-driven-loop.md)) — tests are the guardrails of a data-heavy app, so the loop is organized around them.

**The loop for every behavior change:** 1) **write/extend the test first**, at the cheapest layer that proves the behavior (map below) — a feature isn't "done" until a test **reddens when it breaks**; 2) **iterate against a targeted, fast subset** (seconds; see "Targeted run" below, or `make check-local`); 3) the **full `make check` runs only as the PR's required checks** ([ADR 0096](adr/0096-quality-gate-architecture-under-continuous-release.md)) — the local pre-handover floor is `make check-local`.

**Anti-rot rule (`gate-integrity`).** Every deterministic/hermetic suite is a hard-fail step of `make check`; no step may be `-`-prefixed. Exclusions: [Testing](testing.md). **Anti-drift rule (`docs-drift`, [ADR 0065](adr/0065-spec-code-drift-gates.md)).** The same gate fails if contracts.md/ui-information-architecture.md/data-model.md diverge from the code, or ADR `Status:`/INDEX.md hygiene rots — a spec-ahead-of-code section is tagged `Status: planned (vX.Y.Z, ADR NNNN)`, never left silently wrong.

**Which test where** — change type → layer → targeted run. Details in [testing.md](testing.md); this is the scannable index.

| Change | Test layer(s) — required | Targeted run |
| --- | --- | --- |
| Rust domain logic / read model | unit/contract test vs. `open_in_memory_database` | `rtk cargo nextest run <mod>` |
| **Data transform** (dedup/normalize/match/merge/rank) | + `proptest` **invariants** (idempotence, order-independence, round-trip, stable identity, no-panic) + golden `insta` snapshot ([ADR 0049](adr/0049-test-architecture-v2-data-transform-correctness.md)) | `rtk cargo nextest run <mod>` |
| **New hot path** | + behavioral **scale gate** (offloaded, bounded, not wall-clock) | `rtk cargo nextest run <scale_test>` |
| **New IPC command** | mock-runtime handler (`runtime.ts`) + **dual-execution fidelity corpus** step + `make types` + narrow-window layout spec if panel-driving | `rtk npm test -- runtime`; `rtk npx playwright test <panel>-layout.spec.ts` |
| **UI component / screen** | Vitest workflow test + Playwright (broad-clickable + overflow); strings via `text()` en+pl | `rtk npm test -- -t "<name>"`; `rtk npx playwright test <spec>.spec.ts` |
| New/changed `src/ui` primitive | `PrimitiveGallery.tsx` **and** `primitives.test.tsx` (a11y clean) | `rtk npm test -- primitives` |
| Migration | idempotence + self-heal on real old snapshots | `rtk cargo nextest run migrations` |
| Source adapter | test-sample parse/dedupe/company-match/error + `insta`; drift-guard green | `rtk cargo nextest run <adapter>` |
| Job / scheduler | queue behavior + `run_until_idle`; new kind dispatches | `rtk cargo nextest run jobs` |
| Feature-gated code | build+test with feature on; skips cleanly when absent | `rtk cargo nextest run --features <f>` |
| Code removed/refactored | `knip` clean (part of `check`) | `rtk npm run knip` |

## Definition of Done (the handover gate)

**This is the single stop gate before you report "done" or hand changes back.** "Done" claims the *whole thing* works and is verified — not that your slice compiles. The recurring failure is handing over on a *subset* of checks ("tests pass" but not under Nix / never looked at the UI / never ran the real feature). **Do not hand over until every applicable box is checked; the handoff states what you verified and how (and what you did not).**

It is **scope-aware** (do the sections your change touches; always do §A, §H, §K) and a **living checklist** — when the guardrail-harvest loop ([ADR 0045](adr/0045-guardrail-harvest-loop.md)) produces a lesson that can't be a clean automated gate, add a line here. Commands: [Agent Day-To-Day Check Loop](#agent-day-to-day-check-loop), [Testing](testing.md). When unsure whether something is testable, assume it is — "I didn't know I could test that" is no handoff.

### §0 — Triage: what changed?
Frontend/UI · Rust/backend · dependency or packaging · migration · feature-gated code · code removed/refactored · docs only. Tick the sections below that apply.

### §A — Always
- [ ] Implemented to spec: read the canonical doc(s) for the area (the [Required Reading](../CLAUDE.md) map) — don't infer architecture/field/command names from code alone. ADR added/confirmed if durable architecture or policy changed — shipped `Accepted` (dated to the owner's decision) once the owner approved **that decision**: a plan that schedules a study/spike approves the study, never its verdict — a delegated study's ADR stays `Proposed` until the owner rules on the outcome (harvest 2026-08-19, ADR 0105). Epic closure never flips a status.
- [ ] **`make check-local` passes under Nix** (not host) before handover — a host pass is a hint, not a verdict — and the **full `make check` gate is green as the PR's required checks for the exact head SHA** ([ADR 0062](adr/0062-mandatory-test-gate-and-test-driven-loop.md)/[ADR 0096](adr/0096-quality-gate-architecture-under-continuous-release.md)); the full gate never runs locally. **Re-run `check-local` after the last fix; never hand over on a stale or partial run.**
- [ ] Canonical doc(s) whose behavior changed are updated **in this change** (contracts / data-model / product-spec / ui-flows / ui-information-architecture / architecture / roadmap).
- [ ] **Touched a file pinned in `file-size-baseline.json`?** Grew → extract in this change (or hand-raise the pin as a reviewed decision); shrank → `node scripts/check/file-size-ratchet.mjs --write` in the same change ([ADR 0103](adr/0103-file-size-ratchet-fitness-function.md)).
- [ ] Nothing committed or pushed unless the user asked, or via the release workflow.

### §B — If frontend/UI changed
- [ ] Matched the **destination** screen's scaffold (`feed-panel` shell + `PanelHeader` + padded scrollable body) **and its control idioms** (which `Button` variant, status pattern). **On a relocation, re-check against the new screen's siblings — old-screen conventions don't travel** (Diagnostics uses `compact-button`; Settings uses the `Button` primitive / `secondary-button`). See [ui-authoring.md](ui-authoring.md).
- [ ] Pre-write self-check: primitive for the shape (`src/ui`), domain component for the data shape (`src/shared/components`, e.g. `TickerLabel`), no raw `<input>/<select>/<textarea>`, no inline `style={{…}}`.
- [ ] **New panel/screen or redesign: approved mockup in `docs/mockups/` first** (ui-authoring); goes in every UI subagent brief.
- [ ] Every user-visible string via `text("…")` with **both** `en.ts`/`pl.ts` entries — translation guard green; counts use `pluralNoun`.
- [ ] New UI workflow/behavior has a Vitest component/workflow test. Added/changed a primitive → added to `PrimitiveGallery.tsx` **and** `primitives.test.tsx` (clean under the a11y suite).
- [ ] **You rendered the changed screen and looked at it** — don't defer the visual check to the user. "No GUI in WSL" is not a reason: the browser harness renders any screen headlessly in Chromium ([Testing → Browser UI regression smoke](testing.md#browser-ui-regression-smoke-playwright)) — drive a throwaway Playwright spec, `await page.screenshot(...)`, read the PNG; add any command it calls to `src/test/browserSmokeRuntime.ts`.
- [ ] **`make ui-smoke` (Playwright) green**, including the narrow tall-window viewport matrix in `playwright.config.ts`. Triage every failure — fix or file a tracked issue.
- [ ] **A panel rendering variable/unbreakable content (filenames, headings) has a narrow-window overflow assertion against the inner scroll container, not just the document** (which reads 0 for inner-`overflow:auto` scrolls): assert `scrollWidth ≤ clientWidth+1` on that container + the panel; grid chain uses `min-width:0` + `minmax(0,1fr)` ([ui-authoring.md](ui-authoring.md)). A new IPC command driving the panel joins `src/test/scenarios/runtime.ts` so the assertion can render it.

### §C — If Rust/backend changed
- [ ] Rust gate validated **under Nix** (host clippy/fmt can differ — lints like `is_multiple_of` / `zip(into_iter())` hide there).
- [ ] New command / read model / migration / adapter / mapping has automated tests. Migrations are append-only, idempotent, self-healing; reads of new columns/settings tolerate a missing row with a safe default.
- [ ] **A new category of durable-queue work gets its own worker lane** (or a deliberate lane assignment), and — if it shares an external resource — the matching lock/limit (per-source serialization, per-provider concurrency), so a slow kind cannot starve a latency-sensitive one ([ADR 0059](adr/0059-worker-pools-and-queue-fairness.md)).
- [ ] **A job kind's failure is user-visible** — classified in `jobs::failure_surface` + a visibility test it reaches that surface (ADR 0091 dec. 3).
- [ ] **"Newest/latest" selection orders by the domain date, never `created_at`** (backfill makes them diverge); ships a test where `created_at` order ≠ domain-date order. Rationale: [data-model.md](data-model.md#model-principles) (guardrail `d60305c`).
- [ ] Non-trivial CPU/inference/IO work runs **off the UI thread** (`async fn` + `spawn_blocking`) and reads the persisted derived index rather than recomputing the corpus per call.
- [ ] **A new adapter-backed refresh/ingest path calls `record_source_outcome`** + a test that `last_success_at` gets set — else Sources shows "never refreshed" (harvest 2026-07-15). A path with no `source_adapters` row (the agent-driven KPI ingest, #364) is exempt — the failure mode cannot occur; the stamp lands with the real adapter (#354).
- [ ] **A background/derived-index job has every trigger it needs** — it (re)runs on the events that invalidate it *and* on app startup; verified to populate from a cold/persisted-but-stale state.
- [ ] **No timer/background path deletes or overwrites user data** — destructive actions are user-triggered or owner-approved (product-spec § feed retention).
- [ ] **A new data transform** (dedup / normalization / matching / merge) ships with the `proptest` **invariants** it must satisfy (the committed list, and which one is still open: [Testing](testing.md#data-transform-correctness-property-golden-scale-fuzz-fidelity-pipeline)) plus a **golden `insta` snapshot** of its output. A new **hot path** adds a **behavioral scale gate** (offloaded + algorithmically bounded over a volume dataset, not wall-clock). [ADR 0049](adr/0049-test-architecture-v2-data-transform-correctness.md).
- [ ] **A new IPC command** adds a step to the **dual-execution mock-fidelity corpus** (replayed against both the TS mock runtime and the real Rust `AppState`/storage layer) so the mock cannot silently drift from backend behavior.
- [ ] **Any `#[ts(export)]` type change — including an additive `CommandErrorCode` variant — regenerates the TS bindings in the same change** (`make types`): `check-local` deliberately excludes the ts-rs drift guard, so only CI reddens on it (harvest 2026-08-19, #399 — four MCP error codes shipped without the regen).
- [ ] **Write transactions are `IMMEDIATE`** — `unchecked_transaction`/bare `.transaction()` are banned (source-scan gate `no_write_transaction_is_deferred`); a DEFERRED read→write upgrade under WAL returns `SQLITE_BUSY_SNAPSHOT` past `busy_timeout` (harvest 2026-08-19, #404).

### §D — If feature-gated code
- [ ] Built **and tested with the feature on** — `cargo check/test --features <feature>` — the default gate does not compile it. Compile-green is not "works".
- [ ] A feature-gated runtime test exists, ran against the real resource where available, and **skips cleanly** when absent.

### §E — If a dependency was added/changed, or packaging touched
- [ ] **Windows cross-build green** — `make package-windows-from-linux`. Host/Nix green is not cross-build evidence. Shipped engine deps stay pure-Rust (no transitive C/native: `ring`, `*-sys`, `onig`, openssl). Full packaging paths + the cross-build constraint: the `packaging` skill.

### §F — If code was removed or refactored
- [ ] `rtk npm run knip` clean.

### §G — Real-behavior verification (every functional change)
- [ ] **The feature actually works end-to-end against the real runtime/data it names — not just compiles and passes tests.** Mocks/samples are not completion evidence (roadmap rule). Desktop behavior is verified through the packaged Windows `.exe` / hands-on path, not a WSL Linux build.

### §H — Guardrail harvest (when anything was flagged or discovered) — always check
- [ ] Every defect the user/a review/a gate/you flagged has its **class** closed in this change — a precise gate, or a documented rule + checklist line (the `guardrail-harvest` skill, `.claude/skills/guardrail-harvest/SKILL.md`). A discovered bug not fixed now → a tracked GitHub issue.

### §I — Milestone/epic closure only
- [ ] **Every user-facing capability names the journey it serves** in [ux-journeys.md](ux-journeys.md) (or is explicitly declared a journey-independent utility), and the milestone retro's UX section records which journeys got shorter/longer ([ADR 0074](adr/0074-ux-journeys-and-anti-rot.md)).
- [ ] **Journey E2E + budgets green** — `tests/browser/journeys/` covers the milestone's new user-facing paths via the `journey()` counter; the `budgets.json` floor is tightened when a journey got measurably shorter (ADR 0074).
- [ ] **Owner dogfooding run before release** — the ~15-min real-app journey walk in [dogfooding.md](dogfooding.md); P1 findings block the release, friction feeds the retro's UX section.
- [ ] **Spec-conformance audit against the epic's ADR(s), decision by decision.** For every ADR decision, verify a **live-path invocation exists** (`repoctx callers` from the real job/command/UI entry, not only unit tests) and record a verdict (conforms / partial / deviates / not built). Unit-green modules with no live wiring are the recurring failure this catches (harvest 2026-07-02). "A capability is not done until a user can reach it" applies to every ADR decision.
- [ ] **Epic PR evidence = green required checks** (incl. both coverage ratchets) on the exact tested tree — no separate closure gate ([ADR 0096](adr/0096-quality-gate-architecture-under-continuous-release.md)). Mutation audit auto-triggers on risk-path merges; triage findings into cards, never blockers. Retrospective written (both domains, still-open items honest). · `wiki/` updated. · Version bump via the release workflow — **only on explicit user sign-off**. Epic closure ritual (sub-issue check, ADR audit): [kanban.md](kanban.md) § Epic closure.
- [ ] **Bench audit** when a hot kernel changed — dispatch `bench-audit.yml` (advisory; local `make audit-bench` self-compares).

### §K — Honest handover report — always
- [ ] **A "gate green" claim requires the gate's own evidence.** Full gate: the PR's required checks green **for the exact head SHA** (`gh pr checks`) — never a local run. Local `check-local`: the recipe's own final `CHECK_LOCAL_EXIT=0` line in the saved output (emitted only when every stage passed); a wrapper/task-notification exit code is **never** evidence (it reflects the last shell command, not `make`). A failed step also **aborts the steps after it** — a partially-green log proves nothing about suites that never ran (two S6 gate runs were mis-reported green this way).
- [ ] The handoff states **what was validated and how** (Nix vs host, which suites ran) and **what was NOT run or verified** ("not run on real Windows", "eval not run against the real model", "browser smoke has a pre-existing unrelated failure, filed as X"). No victory lap; surface still-open items rather than implying completeness.

## Agent Day-To-Day Check Loop

Agents minimize token usage via direct `rtk` commands and the local WSL toolchain — a convenience loop, not a replacement for the canonical Nix workflow (`nix develop`/`make check-local` stay authoritative for local verification; the full gate is the PR's CI).

**Host toolchain can be silently split — only Nix is authoritative.** A host/Nix version split has produced false doc-test failures and hidden a real `clippy` lint (`v0.44.0`). Run `check-local` under Nix before claiming green (or `env -u LD_LIBRARY_PATH nix develop -c npm run check:rust`); don't mix host/Nix `cargo` on the same `target/`.

**`cargo check` proves compile, not tests** — the lib can compile while the test build breaks (e.g. an item only used via `use super::*;` in `#[cfg(test)]`). Run `cargo nextest run` before committing storage/test changes; a checkpoint must be test-green, not compile-green.

Preferred commands for targeted iteration: `rtk grep "pattern" path`; `rtk read path --max-lines N --line-numbers` / `rtk sed -n 'a,bp' path`; `rtk npm typecheck`; `rtk npm test` (scoped to `vitest run src` — **not** bare `vitest run`, which sweeps `tests/browser/`) / `rtk npm test -- -t "name"`; `rtk npm build`; `rtk cargo fmt --check` · `clippy --all-targets -- -D warnings` · `nextest run` (preferred) / `cargo test`; `rtk git diff -- path`.

Avoid: `rtk proxy ...` (bypasses filtering); shell-wrapped reads where `rtk grep`/`read`/`sed`/`git status` would work; `make check-local` per small edit. Full local parity when appropriate: `make check-local`, Nix-wrapped env checks, `make package-windows-from-linux`.

**Layered parallelism** ([ADR 0048](adr/0048-test-architecture-sample-data-broad-clickable-coverage-and-layered-parallelism.md)): `make check` stages fast-fail checks then runs Rust/Vitest/build concurrently, workers capped — mechanics in [Testing](testing.md). Local toolchain: `rustup`+`clippy`/`rustfmt`/`cargo-nextest`, Node/npm, `ripgrep`, `fd`, `jq`, `sqlite3`; `flake.nix` is canonical.

## Command Reference

The full `make` target catalog lives in **[command-reference.md](command-reference.md)** (on-demand; the Makefile is the source of truth for recipes). Load it when you need a specific command; the workflow rules above tell you *when* to run what.

## Testing

Strategy, test layers/pyramid, per-area minimum gates, and smoke procedures live in **[Testing](testing.md)**. Run the suites relevant to your change per the [Definition of Done](#definition-of-done-the-handover-gate).

### Visual baseline (ADR 0076 D7)

Committed screenshot baselines under `tests/browser/visual/`: each panel × S/M/L pane widths on `chromium-visual` (dark) + one M pass on `chromium-visual-light`; only these two projects run `tests/browser/visual/**` (others `testIgnore` it).

- **Run:** `rtk npx playwright test --project=chromium-visual --project=chromium-visual-light`. A red diff (> `maxDiffPixelRatio: 0.01`) is either an intended change (update below) or a regression (fix the code). Determinism: animations off, fixed `SAMPLE_NOW`, `document.fonts.ready` per shot.
- **Deliberate update:** `make visual-update SCREEN=<name> REASON="…"`; commit the PNGs naming **which screens changed and why**. **`rm` the affected PNGs first**: a small intended change slips under `maxDiffPixelRatio`, so the compare passes, nothing is rewritten, and the stale baseline re-legitimizes the old UI (harvest 2026-07-16).
- **CI:** `ignoreSnapshots: !!process.env.CI` — CI runs the specs (layout/console gates hold) but skips pixel compare (font rendering varies by machine). **So no gate catches a stale baseline: changing a paneled screen means running these projects locally in that change.** Two drifted unnoticed (harvest 2026-08-03, #314).

### UX quality loop v2 (ADR 0081 — post-pilot only)

Pilot-gated ([ADR 0081](adr/0081-ux-quality-loop-v2.md)): a universal handover check lands here only after the J1/J2 pilot returns `adopt` with owner sign-off. Nothing added yet.
