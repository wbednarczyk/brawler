# Engineering Workflow

This document defines how Brawler should be built, checked, and tested during development.

Use [Project Brief](project-brief.md) for the full documentation map. Related references: [Testing](testing.md), [Release Workflow](release-workflow.md), [Roadmap](roadmap.md), [Kanban](kanban.md), and [ADR 0007: GitHub Build and Lean Testing](adr/0007-github-build-and-lean-testing.md).

## Goals

- Make local build and test commands the primary development interface.
- Use Nix from the start for reproducible local development.
- Make the app easy to build in the GitHub ecosystem.
- Ensure GitHub Actions runs the same commands developers run locally.
- Keep feedback fast for daily development.
- Use automated tests where they protect important behavior.
- Avoid bloated test suites that slow iteration without clear value.
- Make every milestone demoable and CI-checkable.

## Local-First Buildability

The app must be buildable and testable locally first. GitHub Actions should mirror the local workflow rather than define a separate build path.

Rules:

- Every CI check must have an equivalent documented local command.
- GitHub Actions should call project scripts or standard commands, not bespoke CI-only logic.
- If a command cannot be run locally without GitHub services, it should not be part of default CI.
- Local commands should work on the primary development platform, Windows 11, once the scaffold exists.
- CI may run on Linux for cost and speed, but it should validate the same code paths as local commands where practical.
- Environment differences must be explicit, for example `CI=true` or a documented test database path.

## WSL And Windows Runtime Split

The primary development layer is WSL2 Ubuntu 24.04 with Nix. The primary hands-on runtime target is Windows 11.

This creates an intentional split:

- WSL/Nix is the canonical environment for automated checks, frontend builds, Rust tests, contract tests, and CI-equivalent validation.
- Windows is the canonical environment for clickable desktop sanity testing, native Tauri window behavior, OS integration, file dialogs, keychain behavior, packaging checks, and subjective UX review.

The project must not assume that the developer has a Linux GUI inside WSL. A Tauri build produced from WSL is a Linux application, not a Windows executable.

Recommended workflow:

- Run `make check` in WSL before pushing or opening a pull request.
- Run `make build` in WSL when validating frontend production output.
- Use `make smoke-gemini-transcript`, `make smoke-gemini-analysis`, and `make smoke-keyring` only as documented opt-in live smoke tests; they require local credentials or OS/runtime state and are not part of default CI.
- Runtime logs are local JSON Lines files under the app data logs directory. Settings controls the normal log level and rotation limits. Development runs may override these with `BRAWLER_LOG_LEVEL`, `BRAWLER_LOG_MAX_FILES`, and `BRAWLER_LOG_MAX_FILE_MEGABYTES`.
- Local metrics are Developer-mode-only snapshots available through Diagnostics. They are collected on demand from local state plus process-lifetime runtime counters and are not telemetry.
- When closing a milestone, bump the app minor version in all package manifests before handing the branch back for commit/merge.
- Release workflow guardrails live in [Release Workflow](release-workflow.md). Use `make install-git-hooks` once per checkout and `make release-check` before release workflow or milestone closure changes are handed back.
- Use `make frontend-preview` only for quick browser-based layout checks from Windows; this does not validate Tauri APIs.
- Use a native Windows checkout or Git worktree for frequent hands-on desktop testing.
- From that Windows checkout, run `scripts/windows/dev.ps1` to start Tauri dev mode.
- Linux release artifacts are built with `make package-linux-amd64`, which produces versioned `.deb`, `.rpm`, and `.AppImage` files under `release-artifacts`.
- The Linux target intentionally uses a split packaging path: `.deb` and `.rpm` are built through the Nix-wrapped Tauri command, while AppImage is built through the host Ubuntu toolchain because `linuxdeploy` dependency discovery does not reliably resolve WebKitGTK from the Nix store.
- Linux runtime startup contains a WSL-only WebKitGTK compatibility fallback for WSLg/EGL startup failures. This should keep `.deb`, `.rpm`, AppImage, desktop launch, and terminal launch behavior consistent without changing native Linux defaults.
- Windows portable release artifacts are built with `make package-windows-portable-zip`, which uses the Linux/WSL `cargo-xwin` path and produces a versioned portable zip under `release-artifacts`.
- Makefile targets that enter `nix develop` clear inherited `LD_LIBRARY_PATH` before launching Nix. This prevents stale libraries from an outer shell from breaking the Nix executable before the intended dev shell is created.
- `make package-release-artifacts` builds the Linux artifacts and Windows portable zip for release publication.
- The M21 portable executable path is still available as `make package-windows-from-linux`: build the versioned portable Windows executable from the Linux/WSL Nix environment and copy it to a Windows test directory.
- Launch the copied portable executable separately with `make package-windows-smoke-run`.
- `make windows-package` remains a fallback that triggers a native Windows package build, but it requires Windows Node/Rust/MSVC tooling.

Do not routinely run Windows npm/Rust builds inside the same working tree used by WSL/Nix. Mixing Windows and Linux `node_modules` and Rust `target` artifacts in one tree can create slow, confusing, and noisy changes. Prefer `package-windows-from-linux` if the spike proves stable. If native Windows packaging is needed, use a separate Windows checkout/worktree.

**Cross-build constraint — runtime engine dependencies must be pure-Rust (no transitive native/C deps).** The `cargo-xwin` Linux→Windows path compiles C/asm sources with `clang-cl` against the xwin SDK, which fails for many native crates. Adding a dependency that transitively pulls a C/native crate — `ring`, `*-sys` bindings (`onig-sys`), `openssl-sys`, etc. — silently breaks `make package-windows-from-linux` even when the host/Nix build is green. This bit the `v0.45.0` embedding engine: `hf-hub`→`ureq`→`rustls`→`ring` and `tokenizers`'s default `onig` both failed the cross-build and were replaced with the existing `reqwest` (native-tls/SChannel, already cross-compiles) and `tokenizers`' pure-Rust `fancy-regex` backend. Rule: when adding a runtime dependency that will ship in the packaged app (especially under the interpretative-layer / engine boundaries), prefer pure-Rust crates and verify with `make package-windows-from-linux` before relying on it; reuse the already-cross-compiling stack (`reqwest`, `rustls`-free TLS) rather than introducing a parallel one. A host or Nix `cargo build` passing is not evidence the cross-build works.

## Nix Development Environment

Brawler uses Nix from the first scaffold.

Development baseline:

- primary developer OS layer: WSL2 Ubuntu 24.04 on Windows 11
- app target: Windows first, with cross-platform capability preserved
- canonical environment: `flake.nix`
- explicit entrypoint: `nix develop`
- optional convenience: `direnv`/`nix-direnv`

Recommended Nix posture:

- Use Nix to provide toolchains and system libraries, not to hide application commands.
- Keep build/test commands runnable inside `nix develop`.
- Keep `.env` and envdir-style local secrets outside the Nix store.
- Do not put secrets into `flake.nix`, `flake.lock`, `.envrc`, or derivations.
- Commit `flake.lock` for reproducible development.
- Keep the flake small at first; add packaging outputs only when needed.

Initial flake should provide:

- Rust toolchain and formatting/lint tools
- Node.js package manager/tooling
- Tauri native prerequisites for Linux development builds
- SQLite development libraries/tools
- useful local tools such as `pkg-config`

Optional later flake outputs:

- `checks` for local/CI test entrypoints
- `packages` for app packaging experiments
- `devShells` split by purpose if the default shell becomes too heavy
- `devShells.windows-cross` for the experimental Windows-from-Linux packaging toolchain

GitHub Actions relationship:

- GitHub should run the same local commands, either inside `nix develop` or with equivalent setup.
- `nix flake check` should be added once checks are meaningful and not too slow.
- Avoid Nix-driven heavy packaging in default CI until release packaging is explicitly in scope.

Recommended command grouping once code exists:

- `npm run dev`: local development app
- `npm run build`: frontend build
- `npm run test`: frontend tests
- `npm run typecheck`: TypeScript checks
- `npm run lint`: ESLint (primitive-first ban + barrel-import discipline + standard TS/react-hooks rules as warnings)
- `npm run stylelint`: CSS hygiene (no hardcoded hex outside tokens/themes, no duplicate selectors/properties, no empty rules)
- `npm run knip`: dead-code audit (unused files/exports/deps) — periodic, run in the nix dev shell
- `cargo test`: Rust tests
- `cargo clippy`: Rust linting
- `cargo fmt --check`: Rust formatting check
- `nix develop`: enter the reproducible development shell
- `nix flake check`: run Nix-defined checks once they exist
- `make install-git-hooks`: configure the checkout to use repo-local Git hooks
- `make release-check`: validate commit-message, version-sync, and changelog-generation guardrails
- `make changelog`: generate future changelog entries with git-cliff

Optional later convenience wrappers:

- `npm run check`: run frontend check suite
- `cargo xtask ci`: run Rust-side CI-equivalent checks
- `npm run ci`: run a local aggregate check when the project structure supports it

Wrappers must remain thin and documented.

## GitHub Buildability

The repository should be designed for GitHub Actions from the first scaffold.

Cost posture:

- Default workflows must be conservative with GitHub Actions minutes and storage.
- Automatic GitHub Actions triggers are currently disabled. CI is manual-only through `workflow_dispatch` until the project owner decides otherwise.
- If the repository becomes public, standard GitHub-hosted runner cost assumptions may improve, but the project should still avoid waste.
- Avoid larger runners because GitHub bills them separately.
- Avoid macOS runners in default CI because they are usually the most expensive runner class when billed.
- Avoid scheduled workflows until they are clearly needed.
- Use manual packaging workflows for heavier builds.
- Keep artifact uploads rare and retention short.
- Keep default CI secret-free so forks and pull requests can run basic checks safely.

Expected CI jobs after scaffolding:

- Nix environment check: validate the flake when practical
- frontend checks: install, typecheck, lint, unit tests
- Rust checks: format check, clippy, unit tests
- migration checks: create a clean SQLite database from migrations
- desktop smoke build: at least one Linux CI build for fast validation
- Windows build: added before the first packaging milestone

Recommended workflow files once code exists:

- `.github/workflows/ci.yml`
- `.github/workflows/package.yml`

Rules:

- CI should run manually while the repository is private. Push and pull request triggers can be restored later when the owner accepts the Actions usage tradeoff.
- Keep the default CI path fast.
- Packaging jobs can be manual or release-triggered until v1 stabilizes.
- Do not require secrets for default CI.
- Tests that require live external services must be opt-in and skipped by default.
- Use `paths-ignore` or equivalent filters so documentation-only changes do not run heavy build jobs.
- Use workflow concurrency cancellation so superseded pushes do not keep running.
- Set artifact retention to the shortest useful period, for example 1-7 days for experimental builds.

## Free-Tier Constraints

Current GitHub documentation says GitHub Actions usage is free for standard GitHub-hosted runners in public repositories and for self-hosted runners. For private repositories, accounts receive included minutes and storage depending on plan, and usage beyond included amounts can be billed. GitHub's larger runners are billed separately and are not part of the included minutes model.

Project decisions for early v1:

- Treat GitHub Actions minutes/storage as constrained because the repo is private.
- Use standard `ubuntu-latest` runners for default CI.
- Use Nix in GitHub Actions only if it does not make default CI unreasonably slow.
- Do not use larger runners.
- Do not use default macOS CI.
- Add Windows packaging later as manual workflow, not default PR CI.
- Do not upload large build artifacts on every push.
- Do not run live source/provider checks in GitHub Actions by default.

Open decision for the project owner:

- Before enabling heavier CI or packaging, decide whether to keep it on GitHub Actions, use spending controls, run it manually, or move heavier checks to local/self-hosted execution.

## Local Developer Commands

Once the scaffold exists, the repo should expose a small set of predictable commands.

The Makefile is the preferred local command surface from WSL. Targets must remain thin wrappers around documented project commands.

**Pre-push hook (cadence guardrail, ADR 0045).** `make install-git-hooks` installs a `.githooks/pre-push` that runs the **data-driven `smoke-walk`** Playwright spec (the rendered sidebar destinations × the viewport matrix) on every `git push` — fast (~10-20s), so the layout-overflow gate runs *often* rather than only, non-gating, at epic closure (the gap that let the browser suite rot silently red). It auto-skips with a notice if Playwright is not installed (`make ui-smoke` once). The full browser suite still belongs to `make ui-smoke` / `make check-epic`. Do not `--no-verify` past it.

## Definition of Done (the handover gate)

**This is the single stop gate before you report "done" or hand changes back.** "Done" is a claim that the *whole thing* is working and verified — not that the slice you touched compiles. The recurring failure is handing over on a *subset* of checks ("it compiles", "host is green", "tests pass" but not the periodic ones / not under Nix / never actually looked at the UI / never ran the real feature). **Do not hand over until every box that applies is checked, and your handoff message states what you verified and how (and what you did not).**

It is **scope-aware** (do the sections your change touches; always do §A, §H, §K) and a **living checklist** — when the guardrail-harvest loop ([ADR 0045](adr/0045-guardrail-harvest-loop.md)) produces a lesson that can't be a clean automated gate, add a line here. Command reference: [Agent Day-To-Day Check Loop](#agent-day-to-day-check-loop) and [Testing](testing.md). When unsure whether something is testable a given way, assume it is and check [Testing](testing.md) — "I didn't know I could test that" is not an acceptable handoff.

### §0 — Triage: what changed?
Frontend/UI · Rust/backend · dependency or packaging · migration · feature-gated code · code removed/refactored · docs only. Tick the sections below that apply.

### §A — Always
- [ ] Implemented to spec: read the canonical doc(s) for the area (the [Required Reading](../AGENTS.md) map) — don't infer architecture/field/command names from code alone. ADR added/confirmed if durable architecture or policy changed.
- [ ] **`make check` passes under Nix** (not host) — `cargo fmt --check` → `clippy --all-targets -D warnings` → typecheck → ESLint → stylelint → Vitest → `build`. A host pass is a hint, not a verdict (the toolchain can be split). **Re-run the full gate after the last fix; never hand over on a stale or partial run.**
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
- [ ] Non-trivial CPU/inference/IO work runs **off the UI thread** (`async fn` + `spawn_blocking`) and reads the persisted derived index rather than recomputing the corpus per call.
- [ ] **A background/derived-index job has every trigger it needs** — it (re)runs on the events that invalidate it *and* on app startup; verified to populate from a cold/persisted-but-stale state, not just the one action you wired.
- [ ] **A new data transform** (dedup / normalization / matching / merge) ships with its **invariants** (idempotence, order-independence, round-trip, stable identity, associativity, no-panic — the `proptest` helpers) and a **golden `insta` snapshot** of its output. A new **hot path** adds a **behavioral scale gate** (offloaded + algorithmically bounded over a volume dataset, not wall-clock). [ADR 0049](adr/0049-test-architecture-v2-data-transform-correctness.md), [Testing](testing.md#data-transform-correctness-property-golden-scale-fuzz-fidelity-pipeline).
- [ ] **A new IPC command** adds a step to the **dual-execution mock-fidelity corpus** (replayed against both the TS mock runtime and the real Rust `AppState`/storage layer) so the mock cannot silently drift from backend behavior.

### §D — If feature-gated code (e.g. `embedding-model`)
- [ ] Built **and tested with the feature on** — `cargo check/test --features <feature>` — because the default gate does not compile it. Compile-green is not "works".
- [ ] A feature-gated runtime test exists, ran against the real resource where available (e.g. the cached model), and **skips cleanly** when absent. (Embedding model: also run the model-vs-lexical eval — [Testing](testing.md).)

### §E — If a dependency was added/changed, or packaging touched
- [ ] **Windows cross-build green** — `make package-windows-from-linux`. Host/Nix green is not cross-build evidence. Shipped engine deps stay pure-Rust (no transitive C/native: `ring`, `*-sys`, `onig`, openssl).

### §F — If code was removed or refactored
- [ ] `rtk npm run knip` (dead-code) clean.

### §G — Real-behavior verification (every functional change)
- [ ] **The feature actually works end-to-end against the real runtime/data it names — not just compiles and passes tests.** Mocks/samples are not completion evidence (roadmap rule). Desktop behavior is verified through the packaged Windows `.exe` / hands-on path, not a WSL Linux build.

### §H — Guardrail harvest (when anything was flagged or discovered) — always check
- [ ] Every defect the user/a review/a gate/you flagged has its **class** closed in this change — a precise gate, or a documented rule + checklist line ([guardrail-harvest](../.agents/skills/guardrail-harvest.md)). A discovered bug not fixed now → a tracked Radicle issue.

### §I — Milestone/epic closure only
- [ ] `make check-epic` (all suites incl. knip + Playwright); triage every failure (fix or file a tracked issue — this is how `2d9825a` was caught). **`check-epic`'s knip + browser-smoke steps are prefixed with `-` in the Makefile — their exit code is IGNORED, so the target can print Playwright/knip FAILURES and still exit 0. A green exit code does NOT mean the browser suite passed; you must READ the output and triage it (a silent-red Playwright suite — broken by nav/landing changes across two sessions and masked exactly this way — was caught only at a later wrap-up). The `smoke-walk` overflow gate is deliberately data-driven over the rendered sidebar buttons so a nav rename/slim or a new mode is covered automatically.** · Retrospective written (both domains, still-open items honest). · `wiki/` created/updated for every user-facing change. · Version bump via the release workflow — **only on explicit user sign-off**.
- [ ] **`make mutants`** at closure cadence (ADR 0049): mutation-tests the deterministic transform cores (DSL eval, migrations, feed dedup/matching, source normalization) to prove the property/golden tests *kill* defects, not just execute them. Slow + periodic (never in `make check`); when a new dedup/normalization/matching transform lands, extend the `-f` scope in the `mutants` target. · **`make bench`** when a hot kernel changed — the bench-ratchet flags relative regressions against `bench-baseline.json` (informational, machine-dependent, never a hard gate).

### §K — Honest handover report — always
- [ ] The handoff states **what was validated and how** (Nix vs host, which suites ran) and **what was NOT run or verified** ("not run on real Windows", "eval not run against the real model", "browser smoke has a pre-existing unrelated failure, filed as X"). No victory lap; surface still-open items rather than implying completeness.

## Agent Day-To-Day Check Loop

Agents should minimize token usage during normal implementation by using direct `rtk` commands and the locally installed WSL toolchain whenever possible.

This is a convenience loop, not a replacement for the canonical Nix workflow. `nix develop` and `make check` remain the reproducible parity path for milestone closure, pre-commit confidence, and CI-equivalent verification.

**The host toolchain can be silently split, so host "green" does not count — only Nix is authoritative.** A host shell may have a `cargo`/`rustc` (e.g. from `rustup`) at a *different version* than the Nix-pinned `rustdoc`/`clippy`. This bit `v0.44.0`: the host (cargo 1.96 / rustdoc 1.95) produced **false** `cargo test --doc` failures (`E0514`) and **hid a real `clippy` lint** that only the Nix toolchain (1.95) flags. So a `rtk cargo …` pass on the host is a hint, not a verdict. **Before claiming a Rust change green — and always before any release/closure — run the Rust gate under Nix** (`make check`, or `env -u LD_LIBRARY_PATH nix develop -c npm run check:rust`). Do not mix host and Nix `cargo` runs on the same `target/` dir: their artifacts are mutually incompatible and force a full clean rebuild (the real cost of "going Nix" is this cache-thrash from *mixing*, not the wrapper — staying Nix-only keeps the cache warm).

**`cargo check` proves compile, not tests — run `cargo nextest run` (or `cargo test`) before committing a storage- or test-touching change.** The library can compile while the *test* build breaks: e.g. removing an item that is unused in shipped code but still referenced via `use super::*;` in a `#[cfg(test)]` module. `cargo check --lib` passes; `nextest` fails. An interim commit checkpoint must be **test-green**, not merely compile-green (this bit an Architecture v2 storage commit that passed `cargo check --lib` but broke the test build, because a removed constant was still referenced from a `#[cfg(test)]` module).

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

Use full parity checks when appropriate:

- `make check`: milestone closure, broad behavior changes, or before user commit/merge
- Nix-wrapped commands: when validating the canonical environment or investigating environment drift
- `make package-windows-from-linux`: portable Windows executable packaging path for M21 candidate validation

**Layered parallelism (see [ADR 0048](adr/0048-test-architecture-sample-data-broad-clickable-coverage-and-layered-parallelism.md)).** The suites run in parallel both within and across frameworks to keep the loop fast: Rust uses `cargo nextest` (parallel by default), Vitest uses its default pool, and Playwright runs `fullyParallel` (safe only because the browser mock runtime is per-test isolated). `make check` is a staged concurrent orchestrator — a fast-fail stage (typecheck ‖ fmt ‖ lint ‖ stylelint) then the heavy independent suites (Rust clippy+nextest ‖ Vitest ‖ build) concurrently; the real win is overlapping the Rust compile with the JS suites. **Guardrails:** per-framework worker counts are capped so the sum ≈ core count (oversubscription causes false timeout failures — a quality regression, not a speedup), output is captured and printed grouped with hard-stop on first failure (a serial mode stays available for clean logs), and any parallelism change is kept only if a measured before/after on the WSL2 reference machine actually wins.

The local convenience toolchain currently expected in WSL includes Rust through `rustup`, `clippy`, `rustfmt`, `cargo-nextest`, Node/npm, `ripgrep`, `fd`/`fdfind`, `jq`, `sqlite3`, and the native libraries needed by the Rust/Tauri tests. Keep `flake.nix` as the source of truth for reproducible dependency intent even when agents use the direct toolchain for faster feedback.

Recommended WSL commands:

- `make install`: install npm dependencies inside `nix develop`
- `make check`: run the full local automated check suite inside `nix develop`
- `make test`: run frontend tests inside `nix develop`
- `make ui-smoke-install`: download Chromium for the opt-in Playwright browser UI smoke suite
- `make ui-smoke`: run the opt-in Playwright browser UI smoke suite
- `make build`: build the frontend inside `nix develop`
- `make dev`: start Tauri dev mode inside `nix develop`, only useful when Linux GUI forwarding exists
- `make frontend-preview`: serve the frontend preview to a Windows browser; not a native Tauri test
- `make package-windows-from-linux`: target for building the versioned portable Windows executable from Linux/WSL
- `make package-linux-amd64`: target for building versioned Linux `.deb`, `.rpm`, and `.AppImage` release artifacts
- `make package-windows-portable-zip`: target for building the Windows portable release zip from Linux/WSL
- `make package-release-artifacts`: target for building all current public release artifacts
- `make package-windows-smoke-run`: launch the latest copied portable Windows executable for manual smoke testing
- `make windows-package`: fallback target that calls Windows PowerShell to build and copy the native packaged Windows app from the default `D:\Brawler` checkout
- `make windows-package-no-run`: compatibility alias for the fallback build-and-copy behavior
- `make windows-test-help`: print the Windows hands-on testing path

Underlying commands:

- `npm run dev`: start the Tauri app in development mode
- `npm run build`: build the frontend
- `npm run test`: run frontend tests
- `npm run test:browser:install`: download Chromium for the opt-in Playwright browser UI smoke suite
- `npm run test:browser`: run the opt-in Playwright browser UI smoke suite
- `npm run typecheck`: run TypeScript checks
- `npm run lint`: run ESLint (primitive-first ban + barrel-import discipline + standard TS/react-hooks rules as warnings)
- `npm run stylelint`: run CSS hygiene checks (tokens, no duplicate selectors/properties, no empty rules)
- `npm run knip`: run the dead-code audit (unused files/exports/deps)
- `cargo test`: run Rust tests
- `cargo clippy`: run Rust lints
- `cargo fmt --check`: check Rust formatting

GitHub Actions must use these same commands or thin wrappers around them.

Recommended Windows commands:

- `powershell -ExecutionPolicy Bypass -File scripts/windows/dev.ps1`: start native Tauri dev mode from a Windows checkout
- `powershell -ExecutionPolicy Bypass -File scripts/windows/dev.ps1 -Check`: run checks before native dev mode
- `powershell -ExecutionPolicy Bypass -File scripts/windows/dev.ps1 -Build`: create a native Windows Tauri build
- `powershell -ExecutionPolicy Bypass -File scripts/windows/package.ps1 -NoRun`: build and copy the packaged Windows executable

`scripts/windows/package.ps1` accepts:

- `-WindowsRepo`: native Windows checkout path; defaults to `BRAWLER_WINDOWS_REPO` or `D:\Brawler`
- `-OutputDir`: copied artifact directory; defaults to `BRAWLER_WINDOWS_OUT` or `D:\Brawler\Builds\latest`
- `-NoRun`: copy the executable without launching it
- `-OpenOutput`: open the artifact directory in Explorer
- `-SkipInstall`: skip `npm ci`

When using fallback `make windows-package` from WSL, `BRAWLER_WINDOWS_REPO` and `BRAWLER_WINDOWS_OUT` may use WSL-style `/mnt/c/...` paths. The Makefile converts them before invoking PowerShell.

Portable Windows-from-Linux packaging target:

- `package-windows-from-linux`
- `package-windows-portable-zip`

Implementation direction:

- Use the dedicated Nix shell named `windows-cross`.
- Include the Rust `x86_64-pc-windows-msvc` target, `cargo-xwin`, NSIS, LLVM/LLD, Clang, Node, npm, and Tauri CLI prerequisites.
- Run the Tauri build from Linux with a Windows target and `--no-bundle`.
- Copy the resulting portable executable to `D:\Brawler\Builds\latest` with a versioned name such as `brawler-0.21.0-windows-x64-portable.exe`.
- Package public Windows release artifacts as a versioned zip containing `brawler.exe` and `README-portable-windows.txt`.
- Stop already-running copied `brawler*` processes before replacing the portable artifact.
- Launch the copied executable through `powershell.exe` only through `make package-windows-smoke-run`.
- Treat Windows installer generation as a later target; the first Windows-from-Linux loop validates the runnable `.exe`.

Portable Windows data policy:

- M21 Windows release executables store runtime data in `data/` next to the executable.
- Development builds keep using the OS app-data directory.
- Runtime secrets continue to use the OS keychain and may need to be re-entered when a portable folder is moved to another machine or user profile.
- The portable executable relies on the system WebView2 runtime. Bundling a fixed WebView2 runtime or producing an installer is deferred.

Linux release packaging targets:

- `package-linux-amd64`
- `package-release-artifacts`

Implementation direction:

- Build `.deb` and `.rpm` through the Nix-wrapped Tauri bundling path.
- Build AppImage through the host Ubuntu toolchain. The AppImage bundler uses `linuxdeploy` runtime dependency discovery, which is fragile against Nix-store WebKitGTK paths.
- Set `APPIMAGE_EXTRACT_AND_RUN=1` for AppImage packaging, including GitHub Actions, so downloaded linuxdeploy AppImages can self-extract instead of relying only on FUSE availability on the runner.
- Install the host AppImage runtime tools in GitHub Actions, including `libfuse2t64`, `librsvg2-dev`, `squashfs-tools`, `desktop-file-utils`, and `appstream`; the Tauri AppImage bundler downloads and executes linuxdeploy AppImages at packaging time, and the GTK linuxdeploy plugin requires `librsvg-2.0.pc`.
- Collect release artifacts under `release-artifacts` with names such as `brawler-0.28.0-linux-amd64.deb`, `brawler-0.28.0-linux-amd64.rpm`, and `brawler-0.28.0-linux-amd64.AppImage`.
- GitHub release packaging caches npm package data, Cargo registry/git data, `src-tauri/target`, and `.xwin-cache`. Cache keys must stay lockfile-driven to avoid stale dependency reuse.
- Treat AppImage as the Arch-friendly artifact until native Pacman packaging is explicitly designed.
- Linux release builds store runtime data under `~/.brawler`.
- Installed Linux packages must not write runtime data beside package-managed executable paths.

## Testing

Strategy, test layers/pyramid, per-area minimum gates, and the browser / manual / live / packaging smoke procedures all live in **[Testing](testing.md)**. Run the suites relevant to your change per the [Definition of Done](#definition-of-done-the-handover-gate).

## GitHub Actions Design

Initial `ci.yml` should prefer parallel jobs:

- `nix`
- `frontend`
- `rust`
- `migrations`

Later jobs:

- `tauri-smoke`
- `package-windows-from-linux`
- `release`

Packaging jobs can be slower. The default PR feedback loop should stay quick.

Cost-saving workflow defaults:

- trigger heavy jobs only on relevant paths
- cancel in-progress runs for the same branch
- avoid matrix builds until needed
- run one OS by default, preferably Linux
- make packaging `workflow_dispatch`
- keep artifacts small and short-lived
