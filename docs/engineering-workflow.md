# Engineering Workflow

How Brawler is built, checked, and tested during development.

Doc map: [CLAUDE.md](../CLAUDE.md) § Required Reading. Related: [Testing](testing.md), [Release Workflow](release-workflow.md), [Roadmap](roadmap.md), [Kanban](kanban.md), [ADR 0007](adr/0007-github-build-and-lean-testing.md).

## Posture

- Local build/test commands are the primary dev interface; GitHub Actions mirrors them — every CI check has a documented local equivalent. Nix (`flake.nix`, `nix develop`) is the reproducible environment from the first scaffold.
- Automated tests protect important behavior, avoiding bloat that slows iteration; every milestone is demoable and CI-checkable.
- App target: Windows first, cross-platform preserved (dev-vs-runtime OS split: next section). Environment differences (e.g. `CI=true`) are explicit.

## WSL And Windows Runtime Split

WSL2 Ubuntu 24.04 with Nix is the primary development layer; Windows 11 is the primary hands-on runtime target — an intentional split: WSL/Nix is canonical for automated checks, frontend builds, Rust tests, contract tests, CI-equivalent validation; Windows is canonical for clickable desktop sanity testing, native Tauri window behavior, OS integration, file dialogs, keychain behavior, packaging checks, subjective UX review.

Do not assume a Linux GUI inside WSL — a WSL Tauri build is a Linux application, not a Windows executable.

Recommended workflow:

- `make check` in WSL before pushing/opening a PR; `make build` to validate frontend production output; `make frontend-preview` for a quick browser layout check from Windows (no Tauri APIs).
- `make smoke-gemini-transcript`/`smoke-gemini-analysis`/`smoke-keyring`: opt-in live smoke tests needing local credentials/OS state.
- Runtime logs: local JSON Lines under the app-data logs dir, level/rotation via Settings or `BRAWLER_LOG_*` env vars. Local metrics: Developer-mode-only Diagnostics snapshots, not telemetry.
- Bump the app minor version in all package manifests when closing a milestone. Release guardrails: [Release Workflow](release-workflow.md); `make install-git-hooks` once, `make release-check` before closure handoff.
- Native Windows checkout/worktree for hands-on testing; `scripts/windows/dev.ps1` there starts Tauri dev mode. Packaging paths (portable, Linux artifacts, native fallback, cross-build constraint): the `packaging` skill.

## Nix Development Environment

Brawler uses Nix from the first scaffold: `flake.nix` is canonical, `nix develop` the explicit entrypoint (optional `direnv`/`nix-direnv`). Nix provides toolchains/libraries, not a command hiding place — build/test commands stay runnable inside `nix develop`. `.env`/envdir secrets stay outside the Nix store (never in `flake.nix`/`flake.lock`/`.envrc`); commit `flake.lock`. Flake provides: Rust + fmt/lint, Node.js/npm, Linux Tauri prerequisites, SQLite dev libs, `pkg-config`, plus `devShells.windows-cross` (`packaging` skill). GitHub Actions runs the same commands inside `nix develop` or equivalent; avoid heavy Nix packaging in default CI.

## CI Posture

- CI is manual-only via `workflow_dispatch`; conservative with minutes/storage (no larger runners, no default macOS, no scheduled workflows, standard `ubuntu-latest`). Secret-free by default; live-service tests are opt-in.
- `paths-ignore` skips docs-only jobs; concurrency cancellation; short artifact retention (1-7 days), uploads rare.
- Expected jobs: Nix flake check, frontend (install/typecheck/lint/unit), Rust (fmt/clippy/unit), migrations (clean SQLite DB), a Linux desktop smoke build; Windows build before the first packaging milestone.
- Workflow files: `ci.yml` (parallel `nix`/`frontend`/`rust`/`migrations`, later `tauri-smoke`), `package.yml` (`workflow_dispatch`, slower), `mutants.yml` (`workflow_dispatch`, thin wrapper around `make mutants MUTANTS_JAIL=off`, moves the OOM-prone sweep off the owner's WSL). Actions calls project scripts, never bespoke logic; Windows packaging stays manual.

## Local Developer Commands

The Makefile is the preferred local command surface from WSL; targets stay thin wrappers around documented project commands.

**Pre-commit gate ([ADR 0062](adr/0062-mandatory-test-gate-and-test-driven-loop.md)).** `make install-git-hooks` installs `.githooks/pre-commit`, running the **whole `make check`** (hard-fail, under Nix) before every commit — *be sure at commit time; at push it is too late*. **Fail-not-skip:** a missing tool fails with a fix instruction, not a silent skip. `--no-verify` exists for WIP/emergency commits, but a "done"/handback claim is **never** valid on one.

**Commit-message hook runs AFTER the gate** — a rejected message wastes a full `make check` run. Rules: Conventional Commits, single `[a-z0-9._-]+` scope (no commas), subject after the colon ≤ 72 chars. Pre-validate every non-trivial message first: `scripts/release/validate-commit-message.sh --message "<subject line>"` (guardrail, 2026-07-03).

**Pre-push hook (ADR 0045).** `make install-git-hooks` also installs `.githooks/pre-push`: a cheap re-check running the **data-driven `smoke-walk`** Playwright spec (sidebar destinations × viewport matrix) on every `git push` (~10-20s). Auto-skips with a notice if Playwright isn't installed. Do not `--no-verify` past it.

## Test-Driven Development Loop

Brawler is **spec-driven for intent** (docs/ADRs define behavior before code) and **test-driven for the loop** ([ADR 0062](adr/0062-mandatory-test-gate-and-test-driven-loop.md)) — tests are the guardrails of a data-heavy app, so the loop is organized around them.

**The loop for every behavior change:** 1) **write/extend the test first**, at the cheapest layer that proves the behavior (map below) — a feature isn't "done" until a test **reddens when it breaks**; 2) **iterate against a targeted, fast subset** (seconds; see "Targeted run" below, or `make check-fast`); 3) **before commit, run the full `make check`** — the pre-commit hook enforces it; this is the floor, not the ceiling.

**Anti-rot rule (`gate-integrity`).** Every deterministic/hermetic suite is a hard-fail step of `make check`; no step may be `-`-prefixed (silent red is what rotted the browser suite). Exclusions and rationale: [Testing](testing.md). **Anti-drift rule (`docs-drift`, [ADR 0065](adr/0065-spec-code-drift-gates.md)).** The same gate fails if contracts.md/ui-information-architecture.md/data-model.md diverge from the code, or ADR `Status:`/INDEX.md hygiene rots — a spec-ahead-of-code section is tagged `Status: planned (vX.Y.Z, ADR NNNN)`, never left silently wrong.

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

**This is the single stop gate before you report "done" or hand changes back.** "Done" is a claim that the *whole thing* is working and verified — not that the slice you touched compiles. The recurring failure is handing over on a *subset* of checks ("it compiles", "host is green", "tests pass" but not the periodic ones / not under Nix / never actually looked at the UI / never ran the real feature). **Do not hand over until every box that applies is checked, and your handoff message states what you verified and how (and what you did not).**

It is **scope-aware** (do the sections your change touches; always do §A, §H, §K) and a **living checklist** — when the guardrail-harvest loop ([ADR 0045](adr/0045-guardrail-harvest-loop.md)) produces a lesson that can't be a clean automated gate, add a line here. Command reference: [Agent Day-To-Day Check Loop](#agent-day-to-day-check-loop) and [Testing](testing.md). When unsure whether something is testable a given way, assume it is and check [Testing](testing.md) — "I didn't know I could test that" is not an acceptable handoff.

### §0 — Triage: what changed?
Frontend/UI · Rust/backend · dependency or packaging · migration · feature-gated code · code removed/refactored · docs only. Tick the sections below that apply.

### §A — Always
- [ ] Implemented to spec: read the canonical doc(s) for the area (the [Required Reading](../CLAUDE.md) map) — don't infer architecture/field/command names from code alone. ADR added/confirmed if durable architecture or policy changed.
- [ ] **`make check` passes under Nix** (not host) — the **single mandatory gate** ([ADR 0062](adr/0062-mandatory-test-gate-and-test-driven-loop.md)): `npm run check` (Rust fmt · clippy `-D warnings` · nextest · doc, then typecheck · ESLint · stylelint · Vitest · build) → `knip` → `make types-check` (ts-rs drift) → the **full Playwright browser suite** → `gate-integrity`. Every step hard-fails; `.githooks/pre-commit` runs this whole gate before each commit, so a green commit *is* the proof. `make check-fast` is inner-loop iteration only, **never** proof of done. A host pass is a hint, not a verdict (the toolchain can be split). **Re-run the full gate after the last fix; never hand over on a stale or partial run.**
- [ ] Canonical doc(s) whose behavior changed are updated **in this change** (contracts / data-model / product-spec / ui-flows / ui-information-architecture / architecture / roadmap).
- [ ] Nothing committed or pushed unless the user asked, or via the release workflow.

### §B — If frontend/UI changed
- [ ] Matched the **destination** screen's scaffold (`feed-panel` shell + `PanelHeader` + padded scrollable body) **and its control idioms** (which `Button` variant, status pattern). **On a relocation, re-check against the new screen's siblings — old-screen conventions don't travel** (Diagnostics uses `compact-button`; Settings uses the `Button` primitive / `secondary-button`). See [ui-authoring.md](ui-authoring.md).
- [ ] Pre-write self-check: primitive for the shape (`src/ui`), domain component for the data shape (`src/shared/components` — e.g. `TickerLabel` for any qualified ticker), no raw `<input>/<select>/<textarea>`, no inline `style={{…}}`.
- [ ] Every user-visible string via `text("…")` with **both** `en.ts`/`pl.ts` (or `plText`) entries — translation guard green; counts use `pluralNoun`.
- [ ] New UI workflow/behavior has a Vitest component/workflow test. Added/changed a primitive → added to `PrimitiveGallery.tsx` **and** `primitives.test.tsx` (clean under the a11y suite).
- [ ] **You rendered the changed screen and looked at it** — don't defer the visual check to the user. "No GUI in WSL" is not a reason: the browser harness renders any screen headlessly in Chromium ([Testing → Browser UI regression smoke](testing.md#browser-ui-regression-smoke-playwright)). Drive a throwaway Playwright spec to the screen, `await page.screenshot(...)`, read the PNG; add any command it calls to `src/test/browserSmokeRuntime.ts`.
- [ ] **`make ui-smoke` (Playwright) green**, including the narrow tall-window viewport matrix in `playwright.config.ts`. Triage every failure — fix or file a tracked issue.
- [ ] **A panel rendering variable/unbreakable content (filenames, headings) has a narrow-window overflow assertion against the inner scroll container, not just the document.** `document.scrollWidth` reads 0 when the scroll lives in an inner `overflow:auto` element; assert `scrollWidth ≤ clientWidth+1` on that container + the panel. The grid chain feeding it uses `min-width:0` + `grid-template-columns: minmax(0,1fr)` ([ui-authoring.md](ui-authoring.md) styling rules). A new IPC command driving the panel is added to `src/test/scenarios/runtime.ts` so the assertion can render it.

### §C — If Rust/backend changed
- [ ] Rust gate validated **under Nix** specifically (host clippy/fmt can differ — this is where lints like `is_multiple_of` / `zip(into_iter())` hide).
- [ ] New command / read model / migration / adapter / mapping has automated tests. Migrations are append-only, idempotent, self-healing; reads of new columns/settings tolerate a missing row with a safe default.
- [ ] **A new category of durable-queue work gets its own worker lane** (or a deliberate lane assignment), and — if it shares an external resource (a host, an AI provider) — the matching lock/limit (per-source serialization, per-provider concurrency). A single undifferentiated worker that lets a slow kind starve a latency-sensitive one is the regression this prevents ([ADR 0059](adr/0059-worker-pools-and-queue-fairness.md)).
- [ ] **Any "newest / latest / most recent" selection or ranking orders by the domain date (publication / period / event date), never `created_at`/ingestion order** — backfill/re-ingest make `created_at` diverge from chronology; ships a test against real backfilled data where `created_at` order ≠ domain-date order. Rule + rationale in [data-model.md](data-model.md#model-principles) (guardrail from `d60305c`).
- [ ] Non-trivial CPU/inference/IO work runs **off the UI thread** (`async fn` + `spawn_blocking`) and reads the persisted derived index rather than recomputing the corpus per call.
- [ ] **A background/derived-index job has every trigger it needs** — it (re)runs on the events that invalidate it *and* on app startup; verified to populate from a cold/persisted-but-stale state, not just the one action you wired.
- [ ] **A new data transform** (dedup / normalization / matching / merge) ships with its **invariants** (idempotence, order-independence, round-trip, stable identity, associativity, no-panic — the `proptest` helpers) and a **golden `insta` snapshot** of its output. A new **hot path** adds a **behavioral scale gate** (offloaded + algorithmically bounded over a volume dataset, not wall-clock). [ADR 0049](adr/0049-test-architecture-v2-data-transform-correctness.md), [Testing](testing.md#data-transform-correctness-property-golden-scale-fuzz-fidelity-pipeline).
- [ ] **A new IPC command** adds a step to the **dual-execution mock-fidelity corpus** (replayed against both the TS mock runtime and the real Rust `AppState`/storage layer) so the mock cannot silently drift from backend behavior.

### §D — If feature-gated code (e.g. `embedding-model`)
- [ ] Built **and tested with the feature on** — `cargo check/test --features <feature>` — because the default gate does not compile it. Compile-green is not "works".
- [ ] A feature-gated runtime test exists, ran against the real resource where available (e.g. the cached model), and **skips cleanly** when absent. (Embedding model: also run the model-vs-lexical eval — [Testing](testing.md).)

### §E — If a dependency was added/changed, or packaging touched
- [ ] **Windows cross-build green** — `make package-windows-from-linux`. Host/Nix green is not cross-build evidence. Shipped engine deps stay pure-Rust (no transitive C/native: `ring`, `*-sys`, `onig`, openssl). Full packaging paths and the cross-build dependency constraint: the `packaging` skill (`.claude/skills/packaging/SKILL.md`).

### §F — If code was removed or refactored
- [ ] `rtk npm run knip` (dead-code) clean.

### §G — Real-behavior verification (every functional change)
- [ ] **The feature actually works end-to-end against the real runtime/data it names — not just compiles and passes tests.** Mocks/samples are not completion evidence (roadmap rule). Desktop behavior is verified through the packaged Windows `.exe` / hands-on path, not a WSL Linux build.

### §H — Guardrail harvest (when anything was flagged or discovered) — always check
- [ ] Every defect the user/a review/a gate/you flagged has its **class** closed in this change — a precise gate, or a documented rule + checklist line (the `guardrail-harvest` skill, `.claude/skills/guardrail-harvest/SKILL.md`). A discovered bug not fixed now → a tracked Radicle issue.

### §I — Milestone/epic closure only
- [ ] **Every user-facing capability names the journey it serves** in [ux-journeys.md](ux-journeys.md) (or is explicitly declared a journey-independent utility), and the milestone retro's UX section records which journeys got shorter/longer ([ADR 0074](adr/0074-ux-journeys-and-anti-rot.md)).
- [ ] **Journey E2E + budgets green** — `tests/browser/journeys/` covers the milestone's new user-facing paths via the `journey()` counter; the `budgets.json` floor is tightened when a journey got measurably shorter (ADR 0074).
- [ ] **Owner dogfooding run before release** — the ~15-min real-app journey walk in [dogfooding.md](dogfooding.md); P1 findings block the release, friction feeds the retro's UX section.
- [ ] **Spec-conformance audit against the epic's ADR(s), decision by decision.** For every ADR decision, verify a **live-path invocation exists** (`repoctx callers` from the real job/command/UI entry, not only unit tests) and record a verdict (conforms / partial / deviates / not built). Unit-green modules with no live wiring are the recurring failure this catches (ADR 0061 epic, harvested 2026-07-02; earlier: the v0.49 autopilot enable-toggle). "A capability is not done until a user can reach it" applies to every ADR decision.
- [ ] `make check-epic` — the full mandatory gate + the coverage ratchet, **all hard-fail** ([ADR 0062](adr/0062-mandatory-test-gate-and-test-driven-loop.md); no `-`-prefixed steps — `gate-integrity` fails if any return). Triage every failure (fix or file a tracked issue). · Retrospective written (both domains, still-open items honest). · `wiki/` updated for every user-facing change. · Version bump via the release workflow — **only on explicit user sign-off**.
- [ ] **`make mutants`** at closure cadence (ADR 0049): mutation-tests the deterministic transform cores to prove the property/golden tests *kill* defects. Slow + periodic — never in `make check`, kept separate from `check-epic`; extend the `-f` scope when a new dedup/normalization/matching transform lands. · **`make bench`** when a hot kernel changed — bench-ratchet vs `bench-baseline.json` (informational, machine-dependent, never a hard gate).

### §K — Honest handover report — always
- [ ] **A "gate green" claim requires the gate's own exit code as evidence** — for a backgrounded `make check`, grep the echoed `EXIT=`/`${PIPESTATUS[0]}` line from the saved output; a wrapper/task-notification exit code is **never** evidence (it reflects the last shell command, not `make`). A failed step also **aborts the steps after it** — a partially-green log proves nothing about suites that never ran (two S6 gate runs were mis-reported green this way).
- [ ] The handoff states **what was validated and how** (Nix vs host, which suites ran) and **what was NOT run or verified** ("not run on real Windows", "eval not run against the real model", "browser smoke has a pre-existing unrelated failure, filed as X"). No victory lap; surface still-open items rather than implying completeness.

## Agent Day-To-Day Check Loop

Agents minimize token usage via direct `rtk` commands and the local WSL toolchain — a convenience loop, not a replacement for the canonical Nix workflow (`nix develop`/`make check` stay authoritative for closure and CI-equivalent verification).

**Host toolchain can be silently split — only Nix is authoritative.** A version-mismatched host `cargo`/`rustc` vs. Nix-pinned `rustdoc`/`clippy` produced false `cargo test --doc` failures and hid a real `clippy` lint in `v0.44.0`. Run the gate under Nix before claiming green (`make check`, or `env -u LD_LIBRARY_PATH nix develop -c npm run check:rust`); don't mix host/Nix `cargo` on the same `target/`.

**`cargo check` proves compile, not tests** — the lib can compile while the test build breaks (e.g. an item only used via `use super::*;` in `#[cfg(test)]`). Run `cargo nextest run` before committing storage/test changes; a checkpoint must be test-green, not compile-green.

Preferred agent commands for targeted iteration:

- `rtk grep "pattern" path`: search code and docs with compact grouped output.
- `rtk read path --max-lines N --line-numbers`: inspect focused files or short files.
- `rtk sed -n 'start,endp' path`: inspect tight line ranges only when `rtk read` is not suitable.
- `rtk npm typecheck`: TypeScript checks.
- `rtk npm test`: frontend tests. Use this, **not** a bare `vitest run` — `npm test` is scoped to `vitest run src`, whereas bare `vitest run` also sweeps in the Playwright specs under `tests/browser/` and fails with a confusing `test.describe() not expected here` error.
- `rtk npm test -- -t "test name"`: focused frontend test run.
- `rtk npm build`: frontend production build when UI/build behavior changed.
- `rtk cargo fmt --check`: Rust formatting check
- `rtk cargo clippy --all-targets -- -D warnings`: Rust linting
- `rtk cargo nextest run`: preferred Rust test runner when installed
- `rtk cargo test`: fallback Rust test runner
- `rtk git diff -- path` or `rtk diff`: compact changed-line review.

Avoid for normal agent iteration:

- `rtk proxy ...`, because it bypasses RTK output filtering and saves no tokens
- large shell-wrapped read commands when a direct `rtk grep`, `rtk read`, `rtk sed`, or `rtk git status` command would work
- full `make check` after every small edit

Full parity when appropriate: `make check` (mandatory gate), Nix-wrapped commands to validate the environment, `make package-windows-from-linux` for packaging validation.

**Layered parallelism ([ADR 0048](adr/0048-test-architecture-sample-data-broad-clickable-coverage-and-layered-parallelism.md)).** `make check` stages fast-fail checks then runs Rust/Vitest/build concurrently, workers capped against oversubscription — mechanics in [Testing](testing.md). Local WSL toolchain: `rustup` + `clippy`/`rustfmt`/`cargo-nextest`, Node/npm, `ripgrep`, `fd`, `jq`, `sqlite3`; `flake.nix` stays the source of truth.

## Command Reference

| `make` target | Underlying command | When |
| --- | --- | --- |
| `install` | `npm ci` | Set up deps. |
| `check` | `check`→`knip`→`types-check`→`test:browser`→`gate-integrity.mjs`→`docs-drift.mjs` | **Mandatory gate**, before every commit. |
| `docs-drift` | `node scripts/check/docs-drift.mjs` | Spec↔code drift gate standalone (also a `check` step); `--write-adr-index` regenerates `docs/adr/INDEX.md`. |
| `check-fast` | `npm run check:parallel` | Inner-loop only, never proof of done. |
| `check-epic` | `check` + `coverage` | Epic closure: full gate + coverage ratchet. |
| `test` | `npm run test` | Frontend unit tests (Vitest, `src`). |
| `build` | `npm run build` | Frontend production build. |
| `dev` | `npm run dev` | Tauri dev mode; needs Linux GUI forwarding. |
| `frontend-preview` | `npm run preview -- --host 0.0.0.0` | Windows browser layout check; not a Tauri API test. |
| `ui-smoke-install` | `npm run test:browser:install` | Download Chromium for Playwright. |
| `ui-smoke` | `npm run test:browser` | Playwright suite standalone (also a `check` step). |
| `types` | `cargo test --features ts-export export_bindings` | Regenerate TS DTOs from Rust `#[ts(export)]`. |
| `types-check` | `types` + hash diff on generated bindings | Drift guard. |
| `install-git-hooks` | `git config core.hooksPath .githooks` | Wires pre-commit/push hooks. |
| `release-check` | `npm run release:check` | Validate commit-msg/version/changelog guardrails. |
| `changelog` | `git-cliff ... --prepend CHANGELOG.md` | Generate scaffold; curate before release (`brawler-release` skill). |
| `mutants` | `cargo mutants --test-tool nextest -f ...` | Closure-cadence mutation testing (periodic). |
| `bench` | `cargo bench --bench transforms` + `bench:ratchet` | Closure-cadence benchmarks (periodic). |
| `package-*`, `windows-package*`, `windows-test-help` | — | Packaging/Windows paths — the `packaging` skill. |

## Testing

Strategy, test layers/pyramid, per-area minimum gates, and smoke procedures live in **[Testing](testing.md)**. Run the suites relevant to your change per the [Definition of Done](#definition-of-done-the-handover-gate).

### Visual baseline (ADR 0076 D7)

Committed screenshot baselines under `tests/browser/visual/`: each panel × S/M/L pane widths on `chromium-visual` (dark) + one M pass on `chromium-visual-light`; only these two projects run `tests/browser/visual/**` (others `testIgnore` it).

- **Run:** `rtk npx playwright test --project=chromium-visual --project=chromium-visual-light`. A red diff (> `maxDiffPixelRatio: 0.01`) is either an intended change (update below) or a regression (fix the code). Determinism: animations off, fixed `SAMPLE_NOW`, `document.fonts.ready` per shot.
- **Deliberate update:** re-run with `--update-snapshots`, commit the PNGs with a message naming **which screens changed and why** — an unexplained baseline update is a review rejection.
- **CI:** `ignoreSnapshots: !!process.env.CI` — CI executes the specs (layout/console gates hold) but skips pixel compare (font rendering differs across machines; pixels are a local check).
