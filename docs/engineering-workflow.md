# Engineering Workflow

This document defines how Brawler should be built, checked, and tested during development.

See also [Architecture](architecture.md), [Project Practices](project-practices.md), [Roadmap](roadmap.md), [Kanban](kanban.md), and [ADR 0007: GitHub Build and Lean Testing](adr/0007-github-build-and-lean-testing.md).

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

Optional later convenience wrappers:

- `npm run check`: run frontend check suite
- `cargo xtask ci`: run Rust-side CI-equivalent checks
- `npm run ci`: run a local aggregate check when the project structure supports it

Wrappers must remain thin and documented.

## GitHub Buildability

The repository should be designed for GitHub Actions from the first scaffold.

Cost posture:

- The GitHub repository is currently private, so default workflows must be conservative with included GitHub Actions minutes and storage.
- If the repository becomes public later, standard GitHub-hosted runner cost assumptions may improve, but the project should still avoid waste.
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

- CI should run on pull requests and pushes to `master`.
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

Recommended commands:

- `npm run dev`: start the Tauri app in development mode
- `npm run build`: build the frontend
- `npm run test`: run frontend tests
- `npm run typecheck`: run TypeScript checks
- `cargo test`: run Rust tests
- `cargo clippy`: run Rust lints
- `cargo fmt --check`: check Rust formatting

If command wrappers are added later, they should call these underlying tools rather than hide them.

GitHub Actions must use these same commands or thin wrappers around them.

## Lean Testing Strategy

Testing should be fast, focused, and layered.

Default test layers:

- Rust unit tests for domain logic, contracts, parsing, dedupe, migrations, and provider mapping.
- Frontend unit/component tests for UI state and critical workflow components.
- Fixture-based adapter tests for external sources.
- Smoke tests for app startup and command availability.

Avoid by default:

- large end-to-end suites for every UI path
- live network tests in normal CI
- brittle screenshot tests for routine UI
- testing implementation details that do not protect behavior

Use broader tests only when the risk justifies them:

- source adapter parsing
- migration safety
- note provenance
- transcript-to-note workflow
- packaging startup
- local data persistence

## Test Pyramid For V1

Preferred distribution:

- many small Rust unit tests
- some frontend component tests
- fixture-backed integration tests for adapters and migrations
- a few desktop smoke tests

The app does not need exhaustive full-stack end-to-end tests in early v1.

## External Source Testing

Source adapters must be tested with saved fixtures.

Rules:

- Normal CI must not depend on GPW, Gemini, SEC, Nasdaq, or media sites being reachable.
- Live source checks may exist as manual jobs later.
- Fixture refresh should be deliberate and reviewable.
- Adapter tests should cover parsing, dedupe keys, company matching, and error handling.

## AI Provider Testing

AI provider integrations must be mockable.

Rules:

- Normal CI must not require API keys.
- Provider contract mapping should be tested with fixtures.
- Prompt/result shape should be tested without making live calls.
- Live provider checks should be manual or local-only.

## Quality Gates

Minimum gate for ordinary changes:

- docs/contracts updated when behavior changes
- relevant Rust tests pass
- relevant frontend tests pass
- formatting/lint checks pass
- dependency additions are justified and license-reviewed when they affect runtime

Minimum gate for source adapters:

- fixture tests pass
- dedupe and matching tests pass
- source policy documented

Minimum gate for packaging:

- app starts
- Rust command boundary works
- local database opens
- primary screen renders

## GitHub Actions Design

Initial `ci.yml` should prefer parallel jobs:

- `nix`
- `frontend`
- `rust`
- `migrations`

Later jobs:

- `tauri-smoke`
- `windows-package`
- `release`

Packaging jobs can be slower. The default PR feedback loop should stay quick.

Cost-saving workflow defaults:

- trigger heavy jobs only on relevant paths
- cancel in-progress runs for the same branch
- avoid matrix builds until needed
- run one OS by default, preferably Linux
- make packaging `workflow_dispatch`
- keep artifacts small and short-lived
