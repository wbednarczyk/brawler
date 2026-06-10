# Engineering Workflow

This document defines how Brawler should be built, checked, and tested during development.

Use [Project Brief](project-brief.md) for the full documentation map. Related references: [Project Practices](project-practices.md), [Release Workflow](release-workflow.md), [Roadmap](roadmap.md), [Kanban](kanban.md), [Live Smoke Tests](live-smoke-tests.md), and [ADR 0007: GitHub Build and Lean Testing](adr/0007-github-build-and-lean-testing.md).

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
- The M21 portable executable path is `make package-windows-from-linux`: build the versioned portable Windows executable from the Linux/WSL Nix environment and copy it to a Windows test directory.
- Launch the copied portable executable separately with `make package-windows-smoke-run`.
- `make windows-package` remains a fallback that triggers a native Windows package build, but it requires Windows Node/Rust/MSVC tooling.

Do not routinely run Windows npm/Rust builds inside the same working tree used by WSL/Nix. Mixing Windows and Linux `node_modules` and Rust `target` artifacts in one tree can create slow, confusing, and noisy changes. Prefer `package-windows-from-linux` if the spike proves stable. If native Windows packaging is needed, use a separate Windows checkout/worktree.

## M18 Polish Smoke Checklist

M18 visual regression is manual. Do not add Playwright or another browser automation dependency for this milestone.

Run the app in the normal desktop path when possible. `make frontend-preview` is acceptable for browser-only layout review, but it does not validate Tauri commands, keychain behavior, or native window behavior.

Manual review path:

- Settings: open every local settings section, verify the subnavigation stays stable, controls remain readable, and the active panel scrolls independently.
- Appearance: switch dark/light/system, then switch `night-neon` and `midnight-horizon`; verify the palette changes the app tokens without changing the brightness mode unexpectedly.
- Notebooks: select a company, select a note, create a note, edit a long note, use tag filtering and clear it; verify the company list, note list, and detail/editor pane scroll independently.
- Inbox: scan feed rows, change filters, clear representative filter/search inputs, open details, and verify the destructive feed cleanup action is separated from routine review controls.
- Sources: verify adapters are grouped by purpose, disabled/review candidates are visually distinct, expanded rows remain readable, registry search works, and the clear control resets it.
- Companies: create a watchlist, toggle company membership on and off, verify feedback and selected states, and clear representative company form fields.
- Global search: type a query, clear it with the field button, and verify focus returns to the search input.
- Polish locale: switch to Polish and check the updated labels in Settings, Sources, Notebooks, Companies, and licensing views.

Record pass/fail notes in the milestone review before asking for closure signoff.

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

## Agent Day-To-Day Check Loop

Agents should minimize token usage during normal implementation by using direct `rtk` commands and the locally installed WSL toolchain whenever possible.

This is a convenience loop, not a replacement for the canonical Nix workflow. `nix develop` and `make check` remain the reproducible parity path for milestone closure, pre-commit confidence, and CI-equivalent verification.

Preferred agent commands for targeted iteration:

- `rtk grep "pattern" path`: search code and docs with compact grouped output.
- `rtk read path --max-lines N --line-numbers`: inspect focused files or short files.
- `rtk sed -n 'start,endp' path`: inspect tight line ranges only when `rtk read` is not suitable.
- `rtk npm typecheck`: TypeScript checks.
- `rtk npm test`: frontend tests.
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

Implementation direction:

- Use the dedicated Nix shell named `windows-cross`.
- Include the Rust `x86_64-pc-windows-msvc` target, `cargo-xwin`, NSIS, LLVM/LLD, Clang, Node, npm, and Tauri CLI prerequisites.
- Run the Tauri build from Linux with a Windows target and `--no-bundle`.
- Copy the resulting portable executable to `D:\Brawler\Builds\latest` with a versioned name such as `brawler-0.21.0-windows-x64-portable.exe`.
- Stop already-running copied `brawler*` processes before replacing the portable artifact.
- Launch the copied executable through `powershell.exe` only through `make package-windows-smoke-run`.
- Treat Windows installer generation as a later target; the first Windows-from-Linux loop validates the runnable `.exe`.

Portable Windows data policy:

- M21 Windows release executables store runtime data in `data/` next to the executable.
- Development builds keep using the OS app-data directory.
- Runtime secrets continue to use the OS keychain and may need to be re-entered when a portable folder is moved to another machine or user profile.
- The portable executable relies on the system WebView2 runtime. Bundling a fixed WebView2 runtime or producing an installer is deferred.

## Lean Testing Strategy

Testing should be fast, focused, and layered.

Default test layers:

- Rust unit tests for domain logic, contracts, parsing, dedupe, migrations, and provider mapping.
- Frontend unit/component tests for UI state and critical workflow components.
- Test-sample-based adapter tests for external sources.
- Smoke tests for app startup and command availability.

Avoid by default:

- large end-to-end suites for every UI path
- live network tests in normal CI
- brittle screenshot tests for routine UI
- testing implementation details that do not protect behavior

Use broader tests only when the risk justifies them:

- source adapter parsing
- migration safety
- note origin
- transcript-to-note workflow
- packaging startup
- local data persistence

### Browser UI Regression Smoke

M23 accepts a small Playwright-based browser smoke layer for UI/layout regressions that Vitest/jsdom cannot reliably catch. The first implementation targets the Vite preview app in Chromium and remains opt-in until it proves stable.

Setup and run:

- `make ui-smoke-install`: first-time Chromium download for the local Playwright cache
- `make ui-smoke`: run the browser UI smoke suite
- `npm run test:browser:install` and `npm run test:browser` are the direct npm equivalents

The command starts a Vite preview/test server with deterministic browser-smoke data. It does not read live sources or the user's local app database.

Use browser UI smoke tests for repeated layout risks:

- fixed app chrome and absence of a global application scrollbar
- independently scrollable panels in Companies, Watchlists, Notebooks, Inbox, Events, and Sources
- dense row and category sizing in Sources and other list-heavy screens
- compact desktop and normal desktop viewport regressions
- tiny cross-screen navigation smoke when it helps prove the preview harness is wired correctly

Do not use the first Playwright suite for:

- live external source/API testing
- real Tauri desktop file dialogs, keychain, taskbar, packaging, or WebView2 validation
- broad end-to-end coverage of every product workflow
- screenshot comparison as pass/fail evidence

Evidence policy:

- DOM/layout assertions are the pass/fail signal.
- Screenshots and traces are retained only on failure.
- Visual snapshot comparisons are deferred until a specific stable use case justifies the maintenance cost.

Runtime split:

- WSL/Nix owns automated Playwright smoke against the Vite preview app.
- Native Windows remains responsible for hands-on desktop behavior, native OS integrations, and packaged executable smoke testing.
- Browser smoke tests should use deterministic frontend test data, not live sources or the user's local app database.

## Test Pyramid For V1

Preferred distribution:

- many small Rust unit tests
- some frontend component tests
- test-sample-backed integration tests for adapters and migrations
- a few desktop smoke tests

The app does not need exhaustive full-stack end-to-end tests in early v1.

## External Source Testing

Source adapters must be tested with saved test samples.

Rules:

- Normal CI must not depend on GPW, Gemini, SEC, Nasdaq, or media sites being reachable.
- Live source checks may exist as manual jobs later.
- Test sample refresh should be deliberate and reviewable.
- Adapter tests should cover parsing, dedupe keys, company matching, and error handling.

## AI Provider Testing

AI provider integrations must be mockable.

Rules:

- Normal CI must not require API keys.
- Provider contract mapping should be tested with test samples.
- Prompt/result shape should be tested without making live calls.
- Live provider checks should be manual or local-only.
- Required milestone live checks are documented in [Live Smoke Tests](live-smoke-tests.md) and must remain outside default CI/local checks.

## Quality Gates

Minimum gate for ordinary changes:

- docs/contracts updated when behavior changes
- relevant Rust tests pass
- relevant frontend tests pass
- formatting/lint checks pass
- dependency additions are justified and license-reviewed when they affect runtime

Minimum gate for source adapters:

- test-sample tests pass
- dedupe and matching tests pass
- source policy documented

Minimum gate for packaging:

- app starts
- Rust command boundary works
- local database opens
- primary screen renders
- packaged friend-test builds enforce the local license gate before normal navigation

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
