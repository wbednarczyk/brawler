# Command Reference

The `make` target catalog agents work from (extracted from [engineering-workflow.md](engineering-workflow.md), which keeps the workflow rules — what runs where, the TDD loop, the Definition of Done). Loaded on demand; the Makefile itself is the source of truth for recipes and the only complete list.

| `make` target | Underlying command | When |
| --- | --- | --- |
| `install` | `npm ci` | Set up deps. |
| `check` | composes the `check-*` targets | **Mandatory gate**; runs only as the PR's required checks (`full-check.yml`, [ADR 0096](adr/0096-quality-gate-architecture-under-continuous-release.md)). |
| `check-rust-lint` · `check-rust-test` · `check-frontend-static` · `check-frontend-test` · `check-frontend-build` · `check-browser` · `check-visual` · `check-deps` · `check-docs-gates` · `check-commits` | granular gate wrappers | One per `full-check.yml` job (job name mirrors the target, `check-visual` ↔ `Visual baselines (pinned renderer)`); `make check`/`check-local` compose subsets. `check-deps` policy/exceptions: `src-tauri/deny.toml` (network-bound → not in `check-local`). `check-visual` requires docker: runs the two visual Playwright projects inside the pinned renderer (official Playwright image tagged at the locked `@playwright/test` version, #448); zero tolerance. |
| `visual-update SCREEN=<id>\|ALL=1 REASON="why"` | `npm run visual-update` in the pinned renderer (docker) | Deliberate baseline update ([testing.md](testing.md) § Visual baseline); refuses without `REASON` and exactly one of `SCREEN`/`ALL`. |
| `check-release-label` | exactly-one-`release:*` check | Runs in `release-label.yml` (split out so label events re-run a 4s job, not the whole gate). |
| `check-local` | `npm run check:parallel` | Developer inner loop + pre-handover DoD step; never proof of done (renamed from `check-fast`). |
| `check-docs` | docs-only subset | Docs-only changes. |
| `docs-drift` | `node scripts/check/docs-drift.mjs` | Spec↔code drift gate standalone (also a `check` step); `--write-adr-index` regenerates `docs/adr/INDEX.md`. |
| `coverage-frontend` | Vitest v8 coverage + ratchet | PR required check; floor 80.0% vs `coverage-baseline.json` ([ADR 0096](adr/0096-quality-gate-architecture-under-continuous-release.md)). |
| `coverage-rust` | `cargo-llvm-cov` + ratchet | PR required check; floor 86.5% vs `coverage-baseline.json`. |
| `disk-clean` | caches, mutants artifacts, old nix generations, fstrim | Run when `disk-guard` warns. |
| `disk-clean-deep` | + `src-tauri/target` + full nix GC | Space emergencies; full rebuild after. |
| `test` | `npm run test` | Frontend unit tests (Vitest, `src`). |
| `build` | `npm run build` | Frontend production build. |
| `dev` | `npm run dev` | Tauri dev mode; needs Linux GUI forwarding. |
| `frontend-preview` | `npm run preview -- --host 0.0.0.0` | Windows browser layout check; not a Tauri API test. |
| `ui-smoke-install` | `npm run test:browser:install` | Download Chromium for Playwright. |
| `ui-smoke` | `npm run test:browser` | Playwright suite standalone (also a `check` step). |
| `ui-smoke-clickable` | scoped Playwright clickable pass | Broad-clickable subset standalone. |
| `types` | `cargo test --features ts-export export_bindings` | Regenerate TS DTOs from Rust `#[ts(export)]`. |
| `types-check` | `types` + hash diff on generated bindings | Drift guard. |
| `install-git-hooks` | `git config core.hooksPath .githooks` | Wires the `commit-msg` hook (only survivor). |
| `sync-rad` | `git push rad master` + tags | Async Radicle mirror (owner-run; not a process step). |
| `audit-mutants` | `cargo mutants --test-tool nextest -f ...` | Risk-triggered mutation audit; auto on monitored-path `master` pushes, plus manual dispatch. |
| `audit-bench` | `cargo bench --bench transforms` | Advisory local run; honest signal: `bench-audit.yml` (base-vs-head, one runner). |
| `audit-bench-ci` | worktree base bench + head bench + compare | CI-only (`bench-audit.yml` recipe); requires full git history. |
| `live-wait` | CDP-ready poll loop | Internal: shared by `live-up` and `pr-live-cycle`. |
| `live-up` | rebuild + launch on Windows + CDP wait | Live-drive prep from WSL ([testing.md](testing.md) § Live drive). |
| `live-drive` | Playwright vs the real running Windows app | Real-runtime verification; `LIVE_SPEC=<spec>` scopes to one spec. |
| `live-cycle` | `live-up` + `live-drive` | One command from WSL. |
| `live-smoke EXE=<exe>` | boot smoke vs a given exe (clean DB) | CI `windows-boot-smoke` + owner hands-on. |
| `pr-binary PR=<n>` | download the PR's cross-built .exe (head-SHA-pinned) | Owner hands-on testing. |
| `pr-live-cycle PR=<n>` | `pr-binary` + launch + live suite | Drive a PR's exe over the real data (no WSL rebuild). |
| `realdata-gt-score` / `realdata-extraction-check` / `realdata-honesty-check` | maintainer real-DB diagnostics | Advisory, owner-machine only ([testing.md](testing.md)). |
| `package-*`, `windows-package*`, `windows-test-help` | — | Packaging/Windows paths — the `packaging` skill. |
