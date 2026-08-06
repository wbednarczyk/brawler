SHELL := /usr/bin/env bash
.DEFAULT_GOAL := help

NIX := env -u LD_LIBRARY_PATH nix develop -c
NIX_WINDOWS := env -u LD_LIBRARY_PATH nix develop .\#windows-cross -c
WINDOWS_TARGET := x86_64-pc-windows-msvc
# Cargo features compiled into shipped/packaged builds. Empty since ADR 0080
# retired the embedding-model feature; the mechanism stays for a future opt-in
# feature (`make ... RELEASE_FEATURES=<feature>`).
RELEASE_FEATURES ?=
RELEASE_FEATURE_FLAG := $(if $(RELEASE_FEATURES),--features $(RELEASE_FEATURES))
RELEASE_OUT_DIR ?= release-artifacts
WINDOWS_OUT_DIR ?= /mnt/d/Brawler/Builds/latest
WINDOWS_EXE := src-tauri/target/$(WINDOWS_TARGET)/release/brawler.exe
# Read-only MCP stdio adapter (ADR 0078 dec. 6), built alongside the app.
WINDOWS_STDIO_EXE := src-tauri/target/$(WINDOWS_TARGET)/release/brawler-mcp-stdio.exe
# Read the app version without depending on `node` being on the bare PATH. The
# Makefile is evaluated by the host shell (outside `nix develop`), where node is
# not available after the Node 18 removal; a node-based read silently returned
# empty and produced `## v -` changelog headings. This pure-shell read of the
# first `"version"` key (the root package version) needs no toolchain.
APP_VERSION := $(shell sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' package.json | head -1)
WINDOWS_ARTIFACT_NAME := brawler-$(APP_VERSION)-windows-x64-portable.exe
WINDOWS_ARTIFACT := $(WINDOWS_OUT_DIR)/$(WINDOWS_ARTIFACT_NAME)
WINDOWS_PORTABLE_ZIP := $(RELEASE_OUT_DIR)/brawler-$(APP_VERSION)-windows-x64-portable.zip

.PHONY: commit help install dev frontend-preview build check check-local check-docs check-rust-lint check-rust-test check-frontend-static check-frontend-test check-frontend-build check-browser check-docs-gates check-commits check-release-label live-smoke pr-binary sync-rad release-publish stamp-version disk-clean disk-clean-deep coverage coverage-frontend coverage-rust audit-bench audit-bench-ci live-drive-hints pr-live-cycle live-wait report-escaped-defects ux-contact-sheet visual-update audit-mutants types types-check realdata-honesty-check shape-inventory-scan test ui-smoke ui-smoke-clickable ui-smoke-install typecheck frontend-check rust-check install-git-hooks commit-msg-check version-check changelog-check release-notes license-keygen-author license-author license-friend smoke-gemini-transcript smoke-keyring live-drive live-up live-cycle flake-check tauri-build package-linux-amd64 package-windows-from-linux package-windows-portable-zip package-windows-smoke-run package-release-artifacts windows-package windows-package-no-run windows-test-help open-project-windows open-dist-windows package-release-linux package-release-windows

help:
	@printf "Brawler developer commands\n\n"
	@printf "  make install             Install npm dependencies inside nix develop\n"
	@printf "  make check               The single mandatory gate: all deterministic suites, hard-fail (pre-commit runs this)\n"
	@printf "  make check-local         Developer inner-loop / pre-handover check (parallel core, no browser) — NOT proof of done\n"
	@printf "  make check-docs          Docs-only gate: mandatory-read budgets + docs drift, no code suites (pre-commit uses this for docs-only commits)\n"
	@printf "  make check-rust-lint / check-rust-test / check-frontend-static / check-frontend-test / check-frontend-build / check-browser [SHARD=i/N] / check-docs-gates\n"
	@printf "                            Granular CI-parity gate targets — each is one full-check.yml job; 'make check' composes them\n"
	@printf "  make check-commits RANGE=<base>..<head>\n"
	@printf "                            Validate every commit subject in the range against Conventional Commits\n"
	@printf "  make check-release-label PR=<n>\n"
	@printf "                            Assert the PR carries exactly one release:{major,minor,patch,skip} label\n"
	@printf "  make pr-binary PR=<n>     Download a PR's cross-built Windows .exe to /mnt/d/Brawler/Builds/pr-<n>/\n"
	@printf "  make sync-rad             Push master + tags to the Radicle mirror (second path only)\n"
	@printf "  make release-publish VERSION=X.Y.Z\n"
	@printf "                            Emergency local equivalent of the release.yml release job\n"
	@printf "  make disk-clean          Safe temp cleanup: caches, mutants artifacts, old nix generations, journal, fstrim\n"
	@printf "  make disk-clean-deep     disk-clean + cargo target dir (full rebuild next time) + full nix GC\n"
	@printf "  make realdata-honesty-check\n"
	@printf "                            Real-data honesty ratchet on the maintainer's own DB copy (ADR 0091); loud SKIP without it, never in \`make check\`\n"
	@printf "  make shape-inventory-scan\n"
	@printf "                            Regenerate the anonymized shape inventory from the maintainer's DB copy (ADR 0091 dec. 4, epic #40 S6)\n"
	@printf "  make report-escaped-defects\n"
	@printf "                            Advisory escaped-defect taxonomy trend report (ADR 0081 Q7), never part of check\n"
	@printf "  make ux-contact-sheet SCREENS=\"a,b\" (or CHANGED=1)\n"
	@printf "                            Assemble a local HTML contact sheet from the existing visual scenarios (ADR 0081 Q5)\n"
	@printf "  make visual-update SCREEN=<name> REASON=\"why\"\n"
	@printf "                            Deliberate Playwright baseline update — refuses without both\n"
	@printf "  make test                Run frontend tests inside nix develop\n"
	@printf "  make ui-smoke-install    Download Chromium for opt-in Playwright smoke tests\n"
	@printf "  make ui-smoke            Run opt-in Playwright browser UI smoke tests\n"
	@printf "  make ui-smoke-clickable  Run only the @clickable journeys (no layout/density/visual walk)\n"
	@printf "  make build               Build the frontend inside nix develop\n"
	@printf "  make install-git-hooks   Install repo-local commit message hooks\n"
	@printf "  make changelog-check     Validate git-cliff changelog generation\n"
	@printf "  make release-notes TAG=vX.Y.Z\n"
	@printf "                            Print the changelog entry used for GitHub Release notes\n"
	@printf "  make dev                 Start Tauri dev mode inside nix develop, requires Linux GUI/WSLg\n"
	@printf "  make frontend-preview    Serve built frontend preview to Windows browser, not native Tauri\n"
	@printf "  make license-keygen-author\n"
	@printf "                            Generate the external author Ed25519 key if missing\n"
	@printf "  make license-author      Generate an author license token under private/licenses\n"
	@printf "  make license-friend HOLDER=\"Friend Name\"\n"
	@printf "                            Generate a friend-test license token under private/licenses\n"
	@printf "  make smoke-gemini-transcript\n"
	@printf "                            Opt-in live Gemini YouTube transcript smoke test\n"
	@printf "  make smoke-keyring        Opt-in live OS keyring persistence smoke test\n"
	@printf "  make live-up             Rebuild the portable exe, launch it on Windows with CDP open, wait until ready (ADR 0066)\n"
	@printf "  make live-drive          Drive the real running Windows app via WebView2 CDP (ADR 0066), needs a live app\n"
	@printf "  make live-cycle          live-up + live-drive: one command from WSL to rebuild, launch, and test live\n"
	@printf "  make pr-live-cycle PR=<n> [LIVE_SPEC=<spec>]\n"
	@printf "                            Download PR #<n>'s cross-built .exe and drive the live suite against it — no WSL rebuild\n"
	@printf "  make tauri-build         Build the Linux Tauri app from WSL, not a Windows app\n"
	@printf "  make package-linux-amd64\n"
	@printf "                            Build Linux .deb, .rpm, and AppImage release artifacts\n"
	@printf "  make package-windows-from-linux\n"
	@printf "                            Build versioned portable Windows executable from Linux/WSL\n"
	@printf "  make package-windows-portable-zip\n"
	@printf "                            Build zipped portable Windows release artifact from Linux/WSL\n"
	@printf "  make package-release-artifacts\n"
	@printf "                            Build Linux and Windows release artifacts under release-artifacts\n"
	@printf "  make package-windows-smoke-run\n"
	@printf "                            Launch the latest portable Windows executable copied by packaging\n"
	@printf "  make windows-package     Build and copy the packaged Windows app via PowerShell\n"
	@printf "  make windows-test-help   Explain the recommended native Windows sanity-check path\n"

install:
	$(NIX) npm ci

dev:
	$(NIX) npm run dev

frontend-preview:
	$(NIX) npm run preview -- --host 0.0.0.0

build:
	$(NIX) npm run build

# The single mandatory gate composition, run as PR required checks (ADR 0090,
# ADR 0096) — the only pre-ship gate under continuous release. Every
# deterministic/hermetic suite runs here as a hard-fail step, nothing
# exit-ignored; gate-integrity fails the gate if any mandatory suite is dropped
# or any step is `-`-prefixed (silent red). Excluded, advisory/audit only:
# `audit-mutants` (30m–2h, risk-path-triggered), `audit-bench` (machine-dependent
# wall-clock; base-vs-head is audited on demand, see the bench block below),
# the live Gemini/keyring smokes (credentials/network/OS),
# `live-drive` (ADR 0066 — needs a real running Windows app), and packaging
# (OS/toolchain).
check:
	@node scripts/check/disk-guard.mjs
	$(MAKE) check-rust-lint
	$(MAKE) check-rust-test
	$(MAKE) check-frontend-static
	$(MAKE) check-frontend-test
	$(MAKE) check-frontend-build
	$(MAKE) types-check
	$(MAKE) check-browser
	$(MAKE) check-deps
	$(MAKE) check-docs-gates
	$(MAKE) coverage-frontend
	$(MAKE) coverage-rust

# --- Granular gate targets (the CI/Makefile parity contract, ADR 0090) ---------
# Each CI job in .github/workflows/full-check.yml runs exactly ONE of these
# `make <target>` wrappers (asserted by scripts/check/gate-integrity.mjs), so the
# same step runs identically locally, in CI, and in any future infra. `make
# check` above is the composition of the very same targets — one definition per
# step, drift impossible by construction. Every wrapper is thin (no logic beyond
# invoking the existing npm/cargo/script), and every step hard-fails (no `-`
# prefix — gate-integrity forbids it).

# Rust lint gate: format check + clippy-as-errors.
check-rust-lint:
	$(NIX) bash -c 'cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings'

# Rust test gate: nextest (process-per-test) + doc-tests.
check-rust-test:
	$(NIX) bash -c 'cd src-tauri && cargo nextest run && cargo test --doc'

# Frontend static gate: TS typecheck + ESLint + Stylelint + knip (dead-code +
# api-surface guard). Stylelint lives here (CSS static analysis) so the full
# `make check` scope is byte-for-byte the pre-refactor scope.
check-frontend-static:
	$(NIX) npm run typecheck
	$(NIX) npm run lint
	$(NIX) npm run stylelint
	$(NIX) npm run knip

# Frontend test gate: Vitest unit/component suite.
check-frontend-test:
	$(NIX) npm run test

# Frontend build gate: the production Vite build (tsc + vite build).
check-frontend-build:
	$(NIX) npm run build

# Browser gate: install Chromium, then run Playwright. SHARD=i/N runs one shard
# (CI matrix); unset runs the whole suite (local parity). Pixel snapshots
# auto-disable under CI=true inside the Playwright config.
check-browser:
	$(NIX) npm run test:browser:install
	$(NIX) $(if $(SHARD),npx playwright test --shard=$(SHARD),npm run test:browser)

# Dependency-scanning gate (issue #167): known vulnerabilities, yanked crates and
# license policy on the Rust side (cargo-deny, policy in src-tauri/deny.toml),
# plus the npm advisory database on the frontend side. Both hard-fail.
#
# NOT in `check-local`, deliberately: both scanners hit the network (cargo-deny
# fetches the RustSec advisory DB, `npm audit` queries the registry), so putting
# them in the inner loop would break offline work and make it depend on someone
# else's uptime. They belong to the PR's required checks (ADR 0096). Consequence
# to expect, and the point of the gate: a newly published advisory can turn a PR
# red without that PR changing a dependency.
#
# `npm audit --audit-level=high` fails on high/critical only; moderate and below
# are reported without failing (the frontend tree is dev-tooling-heavy, and a
# moderate advisory in a build-time-only package is not worth blocking a merge
# over — `npm audit` alone, unfiltered, shows the full picture).
check-deps:
	$(NIX) bash -c 'cd src-tauri && cargo deny check advisories licenses'
	$(NIX) npm audit --audit-level=high

# Docs gates: gate-integrity meta-guard (ADR 0062/0063) + spec↔code drift
# (ADR 0065) + version sync (manifests agree with each other and, when tags are
# visible, with the newest release tag — ADR 0090 amendment 2026-07-28; before
# that the version-sync check hung off the retired `make release` and ran in no
# gate at all). Seconds; a docs-only PR runs only this + commit-lint in CI.
check-docs-gates:
	$(NIX) node scripts/check/gate-integrity.mjs
	$(NIX) node scripts/check/docs-drift.mjs
	$(NIX) node scripts/check/retired-surface.mjs
	$(NIX) node --test "scripts/check/*.test.mjs"
	$(NIX) npm run release:version-check

# Commit-message gate (ADR 0090): validate every commit subject in RANGE against
# the Conventional Commits schema. CI passes the PR's commit range
# (RANGE=<base>..<head>); locally e.g. RANGE=origin/master..HEAD. Commit
# descriptions ARE the release notes (git-cliff), so this is a hard gate.
check-commits:
	@test -n "$(RANGE)" || { printf "Usage: make check-commits RANGE=<base>..<head>\n" >&2; exit 64; }
	@fail=0; \
	for sha in $$(git rev-list --reverse $(RANGE)); do \
		subject="$$(git log -1 --format=%s "$$sha")"; \
		if ! scripts/release/validate-commit-message.sh --message "$$subject" >/dev/null 2>&1; then \
			printf "✖ commit %s: subject fails Conventional Commits: %s\n" "$$(git rev-parse --short "$$sha")" "$$subject" >&2; \
			fail=1; \
		fi; \
	done; \
	if [ "$$fail" -eq 0 ]; then printf "✓ check-commits: all commits in %s conform to Conventional Commits.\n" "$(RANGE)"; fi; \
	exit $$fail

# Release-label gate (ADR 0090, §D): a PR must carry EXACTLY ONE
# release:{major,minor,patch,skip} label — the label drives continuous release.
# Zero or more than one → red, merge blocked. Needs `gh` authenticated.
check-release-label:
	@test -n "$(PR)" || { printf "Usage: make check-release-label PR=<number>\n" >&2; exit 64; }
	@labels="$$(gh pr view $(PR) --json labels --jq '.labels[].name' | grep -E '^release:(major|minor|patch|skip)$$' || true)"; \
	count="$$(printf '%s\n' "$$labels" | grep -c . || true)"; \
	if [ "$$count" -ne 1 ]; then \
		printf "✖ PR #$(PR) must carry exactly one release:{major,minor,patch,skip} label (found %s: %s)\n" "$$count" "$$(printf '%s ' $$labels)" >&2; \
		exit 1; \
	fi; \
	title="$$(gh pr view $(PR) --json title --jq .title)"; \
	if ! scripts/release/validate-commit-message.sh --message "$$title" >/dev/null 2>&1; then \
		printf "✖ PR #$(PR) title is not Conventional: %s\n" "$$title" >&2; \
		printf "  The title is the squash-merge subject candidate — a non-Conventional squash silently drops the release from the changelog (v0.61.6 hole, 2026-07-29).\n" >&2; \
		exit 1; \
	fi; \
	printf "✓ release-label: PR #$(PR) → %s; title Conventional\n" "$$labels"

# Docs-only pre-commit gate (ADR 0062): the meta-guards a *documentation* change
# can actually break — the mandatory-read byte budgets + parity (gate-integrity,
# ADR 0063) and cross-doc drift — WITHOUT the code suites (types/lint/test/build/
# browser/knip/ts-rs) that a docs-only change cannot affect. The pre-commit hook
# runs this instead of the full `make check` when the staged changeset is
# docs-only; any code/config change still runs the full gate. Seconds, not minutes.
check-docs:
	@node scripts/check/disk-guard.mjs
	$(MAKE) check-docs-gates

# Staged concurrent check (ADR 0048): fast-fail static stage, then the heavy
# suites (Rust clippy+nextest+doc, Vitest, build) concurrently — overlaps the
# Rust compile with the JS suites. Developer inner loop + pre-handover DoD step
# (ADR 0096 decision 6), invoked deliberately — never hook-triggered. Omits
# knip, the ts-rs drift guard, the browser suite, and gate-integrity, so it is
# NEVER proof of done; the PR's required checks are the gate.
check-local:
	@node scripts/check/disk-guard.mjs
	$(NIX) npm run check:parallel
	@printf 'CHECK_LOCAL_EXIT=0\n'

# Disk hygiene (guardrail 2026-07-11: a full host drive killed a session
# mid-work; disk-guard above fails the gate before that point). `disk-clean` is
# the safe routine cleanup; `disk-clean-deep` also drops the cargo target dir
# (tens of GiB — next build recompiles everything) and runs a FULL nix GC.
# Playwright browsers and the cargo registry are kept deliberately: every
# `make check` needs them, so deleting them just re-downloads the same bytes.
# WSL note: freeing space inside WSL does not shrink ext4.vhdx on the host —
# from Windows PowerShell (admin): wsl --shutdown; wsl --manage <distro> --set-sparse true
disk-clean:
	rm -rf src-tauri/mutants.out src-tauri/mutants.out.old
	rm -rf $(HOME)/.npm/_cacache $(HOME)/.cache/pnpm $(HOME)/.cache/pip $(HOME)/.cache/go-build $(HOME)/.cache/node-gyp $(HOME)/.cache/typescript $(HOME)/.cache/mesa_shader_cache
	env -u LD_LIBRARY_PATH nix-collect-garbage --delete-older-than 14d
	@sudo -n journalctl --vacuum-size=50M 2>/dev/null || printf "disk-clean: journal vacuum skipped (no passwordless sudo)\n"
	@sudo -n fstrim -v / 2>/dev/null || printf "disk-clean: fstrim skipped (no passwordless sudo)\n"
	@node scripts/check/disk-guard.mjs

disk-clean-deep: disk-clean
	rm -rf src-tauri/target
	env -u LD_LIBRARY_PATH nix-collect-garbage -d
	@sudo -n fstrim -v / 2>/dev/null || true
	@node scripts/check/disk-guard.mjs

# Coverage ratchet, split into two scoped PR required checks (ADR 0096
# decision 4): frontend (Vitest v8) and Rust (cargo-llvm-cov) line coverage,
# each failing if it drops below its floor in coverage-baseline.json.
# Rust coverage runs under NEXTEST (process-per-test), not plain `cargo test`
# (threads-in-one-process): env-mutating hermetic tests (credential scrubs,
# BRAWLER_MCP_TOKEN) and the loopback-socket test group rely on process
# isolation + .config/nextest.toml — under threaded cargo-test they race each
# other (5 tests reddened the v0.52 closure run exactly this way, 2026-07-12).
coverage-frontend:
	$(NIX) npm run test:coverage
	$(NIX) npm run coverage:ratchet -- --layer=frontend

coverage-rust:
	@mkdir -p coverage
	$(NIX) bash -c 'cd src-tauri && cargo llvm-cov nextest --summary-only --json --output-path ../coverage/rust-summary.json'
	$(NIX) npm run coverage:ratchet -- --layer=rust

coverage: coverage-frontend coverage-rust

# Micro-benchmarks of the hot data-transform kernels (criterion). Local run is
# advisory — criterion self-compares against the previous local run. The honest
# regression signal is the base-vs-head audit on ONE runner:
# .github/workflows/bench-audit.yml → audit-bench-ci (ADR 0096: advisory audit,
# never a required check). Wall-clock is machine-dependent — NEVER part of `make check`.
audit-bench:
	$(NIX) bash -c 'cd src-tauri && cargo bench --locked --bench transforms'

# CI-only (bench-audit.yml): bench BASE_SHA in a detached worktree and HEAD in
# this tree, sharing one criterion dir so the head run produces change/ deltas
# (--baseline-lenient tolerates head-added kernels). Head's Makefile/scripts
# stay authoritative for both measurements. Requires full git history.
audit-bench-ci:
	@set -e; \
	base="$${BASE:-}"; \
	base_sha="$$(git rev-parse --verify "$${base:-$$(git merge-base origin/master HEAD)}^{commit}")"; \
	head_sha="$$(git rev-parse --verify 'HEAD^{commit}')"; \
	if [ "$$base_sha" = "$$head_sha" ]; then \
		printf "✖ audit-bench-ci: base == head (%s) — dispatch from a branch, or pass BASE=<ref>.\n" "$$base_sha" >&2; exit 64; \
	fi; \
	printf "bench-audit: base=%s head=%s\n" "$$base_sha" "$$head_sha"; \
	rm -rf src-tauri/target/criterion /tmp/bench-audit-base; \
	git worktree prune; \
	git worktree add --detach /tmp/bench-audit-base "$$base_sha"; \
	trap 'git worktree remove --force /tmp/bench-audit-base 2>/dev/null || true' EXIT; \
	$(NIX) bash -c 'cd /tmp/bench-audit-base/src-tauri && CARGO_TARGET_DIR="$(CURDIR)/src-tauri/target" cargo bench --locked --bench transforms -- --save-baseline audit-base'; \
	find src-tauri/target/criterion -mindepth 1 -type d \( -name new -o -name change \) -exec rm -rf {} +; \
	$(NIX) bash -c 'cd src-tauri && cargo bench --locked --bench transforms -- --baseline-lenient audit-base'; \
	$(NIX) node scripts/check/bench-compare.mjs

# Escaped-defect taxonomy trend report (ADR 0081, plan Q7). Advisory only,
# never part of `make check`: counts escaped frontend/UX defects by origin
# class and detection stage from docs/retros/*.md's marked tables, prints
# repeated classes (count >= 2) as guardrail-harvest candidates. Never fails
# because counts increased — only a malformed opted-in row exits non-zero.
report-escaped-defects:
	$(NIX) npm run report:escaped-defects

# UX contact sheet (ADR 0081 plan Q5): assemble a local HTML grid of the
# EXISTING visual scenarios (tests/browser/visual/*.spec.ts) for cheap human
# review — committed Playwright baselines remain the regression mechanism.
# SCREENS is a comma list; CHANGED=1 maps a read-only git diff through
# tests/browser/visual/catalog.ts instead. STATE/THEME are optional
# passthroughs. Output: .artifacts/ux-contact-sheets/<build>/index.html
# (gitignored).
ux-contact-sheet:
	@test -n "$(SCREENS)$(CHANGED)" || { printf "Usage: make ux-contact-sheet SCREENS=\"a,b\" (or CHANGED=1)\n" >&2; exit 1; }
	$(NIX) node scripts/ux/contact-sheet.mjs $(if $(SCREENS),--screens=$(SCREENS)) $(if $(CHANGED),--changed) $(if $(STATE),--state=$(STATE)) $(if $(THEME),--theme=$(THEME))

# Deliberate Playwright baseline update. STOP-AND-ASK elsewhere in this repo
# guards against silent baseline drift, so this refuses to run without a named
# SCREEN and a non-empty REASON, and prints both into the run log for the
# change description to cite (docs/testing.md § UX contact sheet).
visual-update:
	@test -n "$(SCREEN)" || { printf "Usage: make visual-update SCREEN=<name> REASON=\"why\"\n" >&2; exit 1; }
	@test -n "$(REASON)" || { printf "Usage: make visual-update SCREEN=<name> REASON=\"why\"\n" >&2; exit 1; }
	SCREEN="$(SCREEN)" REASON="$(REASON)" $(NIX) npm run visual-update

# Mutation testing of the deterministic cores (ADR 0048, scope per ADR 0049):
# verifies tests catch behavior changes, not just execute code — the strong
# signal that the property/golden tests actually KILL defects (line coverage does
# not prove this). Scope follows the highest-risk pure transform logic: the DSL
# parser/evaluator, the migration runner, feed dedup/matching, and the source
# normalization core (slug/Polish/link normalizers, now invariant-tested in T1).
# Uses nextest for a fast per-mutant test pass. Advisory, never in `make
# check` (ADR 0096): slow (rebuilds + runs the suite per mutant), auto-runs on
# `master` pushes touching the scoped risk paths plus manual dispatch
# (mutation-audit.yml); triage every survivor by adding an assertion.
# BRAWLER_SCENARIOS_DIR: cargo-mutants copies only src-tauri/ into a scratch
# tree, so the shared fixtures dir (one level above the workspace) doesn't
# exist there. Export an absolute path to the real directory so build.rs
# (see src-tauri/build.rs) bakes it into every mutant/baseline build; all
# cross-tree fixture reads go through this env (guarded by source_tree_guards).
# Resource caps (harvested 2026-07-03: an uncapped sweep saturated WSL — every
# mutant rebuild used all cores and multi-GB rustc peaks, freezing the desktop).
# Low-priority + bounded build/test parallelism keeps the machine usable; the
# sweep takes longer but never needs killing. Build jobs: MUTANTS_BUILD_JOBS
# (below). Test threads: the `mutants` profile in src-tauri/.config/nextest.toml
# (an env NEXTEST_TEST_THREADS would leak into cargo-mutants' internal
# `nextest --no-run` build call, which rejects it). Override for a dedicated
# box: make audit-mutants MUTANTS_BUILD_JOBS=8 (+ edit the nextest profile).
# MUTANTS_SHARD (e.g. 1/8) runs one shard — split a sweep across quiet moments.
# MUTANTS_MEMORY_MAX: hard memory jail via a systemd user scope — when the
# sweep's builds spike past it, the OOM killer takes the SCOPE, not the whole
# WSL VM (two full-WSL freezes on 2026-07-03 were memory, which nice/job caps
# cannot bound). Requires systemd (WSL: systemctl is-system-running → running).
# MUTANTS_JAIL controls whether the systemd-run jail is used: `auto` (default)
# cheaply probes `systemd-run --user --scope true` and jails only if that
# succeeds — WSL has a user systemd manager, so this is a no-op behavior
# change there; `off` skips the jail unconditionally (a GitHub-hosted
# ubuntu-latest runner has no user systemd manager, so mutation-audit.yml
# passes MUTANTS_JAIL=off instead of relying on the probe to fail).
MUTANTS_BUILD_JOBS ?= 2
MUTANTS_MEMORY_MAX ?= 11G
MUTANTS_SHARD ?=
MUTANTS_JAIL ?= auto
# cargo-mutants encodes the sweep result in its exit code (0 caught, 2 missed,
# 3 timed out), but GNU make maps EVERY failed recipe to its own exit 2 — the
# true code never reaches a caller through make's status. The recipe therefore
# persists it to mutants.out/cargo-mutants-exit-code (the MAKE_CHECK_EXIT=
# marker pattern); mutation-audit.yml's verdict reads the file, never make's status.
# MUTANTS_SHARD is cargo-mutants k/n syntax: k is 0-INDEXED (valid: 0/8..7/8).
audit-mutants:
	@jail_cmd=""; \
	case "$(MUTANTS_JAIL)" in \
	  off) jail_cmd="" ;; \
	  auto) if systemd-run --user --scope true >/dev/null 2>&1; then \
	          jail_cmd="systemd-run --user --scope -p MemoryMax=$(MUTANTS_MEMORY_MAX) -p MemorySwapMax=1G -p OOMPolicy=continue --same-dir"; \
	        fi ;; \
	  *) echo "MUTANTS_JAIL must be 'auto' or 'off' (got: $(MUTANTS_JAIL))" >&2; exit 1 ;; \
	esac; \
	$$jail_cmd $(NIX) bash -c "export BRAWLER_SCENARIOS_DIR='$(CURDIR)/src/test/scenarios'; \
	  export CARGO_BUILD_JOBS=$(MUTANTS_BUILD_JOBS) NEXTEST_PROFILE=mutants; \
	  cd src-tauri && nice -n 19 cargo mutants --test-tool nextest --jobs 1 \
	  $(if $(MUTANTS_SHARD),--shard $(MUTANTS_SHARD)) \
	  -f 'src/fundamentals/expr/**' \
	  -f 'src/storage/migrations.rs' \
	  -f 'src/storage/feed_matching.rs' \
	  -f 'src/source_adapters/parsing.rs' \
	  -f 'src/entity_resolution.rs'; \
	  ec=\$$?; mkdir -p mutants.out; \
	  echo \$$ec >mutants.out/cargo-mutants-exit-code; exit \$$ec"

# Generate the TypeScript API DTOs from the Rust source (ADR 0048): ts-rs emits
# src/api/generated/ from the #[ts(export)] structs (behind the ts-export feature).
# TS_RS_LARGE_INT=number renders i64/u64 as `number` (not the ts-rs default
# `bigint`) so generated DTOs match the hand-written contract — our row counts,
# timestamps-as-millis, and id-free numeric fields are all JS-safe integers.
# Decimal/monetary fields stay explicit `#[ts(type = "string")]` on the field.
types:
	$(NIX) bash -c 'cd src-tauri && TS_RS_LARGE_INT=number cargo test --features ts-export export_bindings'

# Drift guard: regenerate, then fail if regeneration CHANGED the working-tree
# bindings (self-consistency). Deliberately independent of git staging state:
# the previous `git diff --exit-code` compared the worktree to the index, so it
# false-positived on legitimately regenerated-but-not-yet-staged bindings and
# blocked `make check` mid-work; hashing before/after regeneration detects true
# staleness (a Rust struct changed without rerunning `make types`) in every
# flow, including the pre-commit gate.
types-check:
	@before=$$(find src/api/generated -type f -name '*.ts' -print0 | sort -z | xargs -0 sha256sum); \
	$(MAKE) types; \
	after=$$(find src/api/generated -type f -name '*.ts' -print0 | sort -z | xargs -0 sha256sum); \
	if [ "$$before" != "$$after" ]; then \
	  printf "✖ src/api/generated was stale — 'make types' changed it. Review and include the regenerated files.\n"; \
	  exit 1; \
	fi

test:
	$(NIX) npm run test

ui-smoke-install:
	$(NIX) npm run test:browser:install

ui-smoke:
	$(NIX) npm run test:browser

# The clickable journeys alone (`@clickable`-tagged describes), without the
# layout/density/visual walk — the fast subset when iterating on an interaction.
ui-smoke-clickable:
	$(NIX) npm run test:browser:clickable

typecheck:
	$(NIX) npm run typecheck

frontend-check:
	$(NIX) npm run check:frontend

rust-check:
	$(NIX) npm run check:rust

# Convenience direct invocation of the docs-drift step (ADR 0065) — also runs
# as part of `make check`. `--write-adr-index` regenerates docs/adr/INDEX.md.
docs-drift:
	$(NIX) node scripts/check/docs-drift.mjs

install-git-hooks:
	git config core.hooksPath .githooks
	@printf "Installed repo-local git hooks from .githooks\n"

commit-msg-check:
	$(NIX) npm run release:commit-msg-check

version-check:
	$(NIX) npm run release:version-check

changelog-check:
	$(NIX) npm run release:changelog-check

# Validate the commit message BEFORE the expensive pre-commit gate runs (the
# commit-msg hook fires after make check by git design, so a rejected subject
# wastes a full gate run). Stages nothing: git add what you want first.
# Optional BODY adds a second -m paragraph.
commit:
	@test -n "$(MSG)" || { printf 'Usage: make commit MSG="type(scope): subject" [BODY="..."]\n' >&2; exit 64; }
	@scripts/release/validate-commit-message.sh --message "$(MSG)"
	@if [ -n "$(BODY)" ]; then git commit -m "$(MSG)" -m "$(BODY)"; else git commit -m "$(MSG)"; fi

release-notes:
	@test -n "$(TAG)" || { printf "Usage: make release-notes TAG=vX.Y.Z\n" >&2; exit 64; }
	@scripts/release/extract-changelog-entry.sh "$(TAG)" CHANGELOG.md

# Sync the Radicle mirror (second distribution path only — no process depends on
# it, ADR 0090 §D). Owner runs it whenever convenient after releases.
sync-rad:
	git push rad master --follow-tags

# Download a PR's cross-built Windows portable .exe for owner hands-on testing
# (ADR 0090 §B2). Pulls the `windows-build` artifact from the full-check run
# that actually built the PR's CURRENT head SHA (not merely "latest run on the
# branch" — a stale/failed run on an old push would silently serve an old
# binary) into /mnt/d/Brawler/Builds/pr-<n>/. Needs `gh`.
pr-binary:
	@test -n "$(PR)" || { printf "Usage: make pr-binary PR=<number>\n" >&2; exit 64; }
	@set -e; \
	dest="/mnt/d/Brawler/Builds/pr-$(PR)"; \
	head_sha="$$(gh pr view $(PR) --json headRefOid --jq .headRefOid)"; \
	if [ -z "$$head_sha" ]; then printf "Could not resolve head SHA for PR #$(PR).\n" >&2; exit 1; fi; \
	runs_json="$$(gh run list --workflow full-check.yml --commit "$$head_sha" --json databaseId,status,conclusion,url)"; \
	run_id="$$(printf '%s' "$$runs_json" | jq -r '[.[] | select(.conclusion=="success")][0].databaseId // empty')"; \
	if [ -z "$$run_id" ]; then \
		run_id="$$(printf '%s' "$$runs_json" | jq -r '.[0].databaseId // empty')"; \
		if [ -n "$$run_id" ]; then \
			printf "⚠ pr-binary: no green full-check run for %s yet — using the newest run for that SHA (may be in-flight or failed).\n" "$$head_sha" >&2; \
		fi; \
	fi; \
	if [ -z "$$run_id" ]; then printf "No full-check run found for PR #$(PR) head SHA %s.\n" "$$head_sha" >&2; exit 1; fi; \
	run_url="$$(printf '%s' "$$runs_json" | jq -r --arg id "$$run_id" '.[] | select((.databaseId|tostring)==$$id) | .url')"; \
	printf "pr-binary: PR #$(PR) head %s, run %s (%s)\n" "$$head_sha" "$$run_id" "$$run_url"; \
	tmp_dest="$$(mktemp -d /mnt/d/Brawler/Builds/.pr-$(PR)-download.XXXXXX)"; \
	trap 'rm -rf "$$tmp_dest"' EXIT; \
	gh run download "$$run_id" --name windows-build --dir "$$tmp_dest"; \
	if ! find "$$tmp_dest" -maxdepth 1 -type f -name '*.exe' | grep -q .; then \
		printf "pr-binary: downloaded artifact contains no .exe — refusing to replace %s.\n" "$$dest" >&2; exit 1; \
	fi; \
	rm -rf "$$dest"; \
	mv "$$tmp_dest" "$$dest"; \
	trap - EXIT; \
	printf "Downloaded PR #$(PR) Windows artifact to %s\n" "$$dest"

# Stamp the release version into the manifests/binary/UI at build time WITHOUT
# committing (ADR 0090 §D pt 2, amended 2026-07-28: release.yml commits the
# same stamp post-tag, so this build-time copy matches what lands on master).
# Reuses the version bumper as a pure file-stamp; the tree is left dirty
# deliberately (ephemeral in CI; discard locally).
stamp-version:
	@test -n "$(VERSION)" || { printf "Usage: make stamp-version VERSION=X.Y.Z\n" >&2; exit 64; }
	$(NIX) node scripts/release/bump-version.mjs "$(VERSION)"

# Emergency LOCAL equivalent of the release.yml `release` job (ADR 0090 §D pt 3):
# stamp version → build artifacts → tag (idempotent) → GitHub Release + upload
# with git-cliff notes. The workflow is the primary path; this is the parity
# fallback the owner runs from WSL when CI is unavailable.
release-publish:
	@test -n "$(VERSION)" || { printf "Usage: make release-publish VERSION=X.Y.Z\n" >&2; exit 64; }
	$(MAKE) package-release-artifacts VERSION=$(VERSION)
	@if git rev-parse -q --verify "refs/tags/v$(VERSION)" >/dev/null; then \
		printf "Tag v$(VERSION) already exists; skipping tag creation.\n"; \
	else \
		git tag -a "v$(VERSION)" -m "v$(VERSION)"; \
		git push origin "v$(VERSION)"; \
	fi
	@prev="$$(git describe --tags --abbrev=0 "v$(VERSION)^" 2>/dev/null || true)"; \
	range="$${prev:+$$prev..}v$(VERSION)"; \
	$(NIX) git-cliff --config cliff.toml $$range --tag "v$(VERSION)" --strip header > /tmp/brawler-release-notes-$(VERSION).md; \
	if gh release view "v$(VERSION)" >/dev/null 2>&1; then \
		gh release upload "v$(VERSION)" $(RELEASE_OUT_DIR)/* --clobber; \
	else \
		gh release create "v$(VERSION)" $(RELEASE_OUT_DIR)/* --title "v$(VERSION)" --notes-file /tmp/brawler-release-notes-$(VERSION).md; \
	fi

license-keygen-author:
	$(NIX) node scripts/licensing/generate-ed25519-key.mjs

license-author:
	@OUT_PATH="$${OUT:-private/licenses/author.txt}"; \
	$(NIX) node scripts/licensing/generate-license.mjs --type author --out "$$OUT_PATH"

license-friend:
	@if [ -z "$${HOLDER:-}" ]; then \
		printf "HOLDER is required. Example: make license-friend HOLDER=\"Friend Name\"\n"; \
		exit 1; \
	fi
	@OUT_PATH="$${OUT:-private/licenses/friend-$$(printf "%s" "$$HOLDER" | tr '[:upper:]' '[:lower:]' | tr -cs '[:alnum:]' '-' | sed 's/^-//; s/-$$//').txt}"; \
	ARGS=(--type friend --holder "$$HOLDER" --out "$$OUT_PATH"); \
	if [ -n "$${EXPIRES_AT:-}" ]; then ARGS+=(--expires-at "$$EXPIRES_AT"); fi; \
	if [ -n "$${FEATURES:-}" ]; then ARGS+=(--features "$$FEATURES"); fi; \
	$(NIX) node scripts/licensing/generate-license.mjs "$${ARGS[@]}"

smoke-gemini-transcript:
	@if [ -z "$${GEMINI_API_KEY:-}" ]; then \
		printf "GEMINI_API_KEY is required for the live Gemini smoke test.\n"; \
		exit 1; \
	fi
	@if [ -z "$${BRAWLER_GEMINI_SMOKE_YOUTUBE_URL:-}" ]; then \
		printf "BRAWLER_GEMINI_SMOKE_YOUTUBE_URL is required for the live Gemini smoke test.\n"; \
		exit 1; \
	fi
	$(NIX) cargo test --manifest-path src-tauri/Cargo.toml live_gemini_transcribes_youtube_url -- --ignored --nocapture

smoke-keyring:
	$(NIX) cargo test --manifest-path src-tauri/Cargo.toml live_keyring_persists_gemini_transcription_secret -- --ignored --nocapture

# T7-C real-data corpus regression (docs/testing.md § Real-data extraction
# corpus): drives the structured-extraction pipeline over the maintainer's
# throwaway CBF corpus and diffs each document's outcome against the committed
# baseline (src-tauri/src/storage/tests/t7_cbf_corpus_expectations.json).
# Inert without the corpus. Refresh the baseline deliberately (after reviewing
# the printed table) with: BRAWLER_UPDATE_EXPECTATIONS=1 make realdata-extraction-check
# The t7_cbf filter also runs the T7-F double-extraction idempotency anchor,
# which WRITES to the corpus DB — the corpus copy is throwaway by contract.
REALDATA_DB ?= private/realdata/t7-cbf/brawler.sqlite3
REALDATA_DIR ?= private/realdata/t7-cbf
realdata-extraction-check:
	$(NIX) bash -c 'cd src-tauri && BRAWLER_REAL_DB=$(abspath $(REALDATA_DB)) BRAWLER_REAL_DATA_DIR=$(abspath $(REALDATA_DIR)) cargo test t7_cbf -- --ignored --nocapture'

# #182 ESEF/positional ground-truth scorer — DIAGNOSTIC, no floor (2026-08-05
# methodology audit deferred floors to measurement v2; see docs/testing.md);
# storage::tests::real_data_extraction::esef_positional_ground_truth_scores.
# Scores facts ALREADY IN a throwaway copy of the corpus's db-snapshot.sqlite3
# against the hand-merged private/realdata/spikes/esef-positional-gt/
# ground_truth.json — no pipeline run, no writes to the snapshot. Inert
# without the owner's private #182 corpus (loud SKIP, non-failing). Prints the
# full evidence table (incl. currency columns) + per-tier aggregates + a
# per-ticker breakdown, writes scoring-report.json next to the corpus — no
# precision/recall floor to assert (docs/testing.md § "#182 ESEF / positional
# ground-truth scorer").
GT_DIR ?= private/realdata/spikes/esef-positional-gt
realdata-gt-score:
	$(NIX) bash -c 'cd src-tauri && BRAWLER_GT_DIR=$(abspath $(GT_DIR)) cargo test esef_positional_ground_truth_scores -- --ignored --nocapture'

# Real-data honesty ratchet (epic #40 S4, ADR 0091 dec. 4-5; docs/testing.md
# § Real-data honesty harness): measures — through the real read models — how
# honest the Today stream is on the maintainer's own database (specificity floor,
# orphaned-evidence ceiling, filename-as-statement hard zero) and judges the run
# against the committed aggregate baseline.
#
# LOCAL ONLY, owner-only, and deliberately NOT part of `make check`: the real
# database never enters the public repo or default CI (ADR 0091 dec. 4),
# loudly SKIPPING when the master snapshot is absent (any machine but the
# owner's); CI-side coverage of the same invariants comes from the synthetic
# shape corpus (epic #40 S6).
#
# The harness MIGRATES the database it opens, so this target always works on a
# fresh throwaway copy (private/realdata/README.md) and the harness itself
# refuses the master snapshot / the live app DB.
HONESTY_MASTER_DB ?= private/realdata/brawler.sqlite3
HONESTY_DB ?= private/realdata/honesty-worktest.sqlite3
realdata-honesty-check:
	$(NIX) bash scripts/check/check-realdata-ratchet.sh
	@if [ -f $(HONESTY_MASTER_DB) ]; then \
		printf "realdata-honesty-check: refreshing the throwaway copy %s\n" "$(HONESTY_DB)"; \
		cp $(HONESTY_MASTER_DB) $(HONESTY_DB) && \
		$(NIX) bash -c 'cd src-tauri && BRAWLER_REAL_DB=$(abspath $(HONESTY_DB)) cargo test real_data_honesty_metrics -- --ignored --nocapture' && \
		$(NIX) node scripts/check/realdata-ratchet.mjs; \
	else \
		printf "\n!! SKIP realdata-honesty-check: no real database at %s.\n!! The honesty ratchet measures the maintainer's OWN data and runs on the maintainer's machine only (ADR 0091 dec. 4);\n!! public CI covers the same invariants via the synthetic shape corpus. Refresh the snapshot per private/realdata/README.md to enable it.\n\n" "$(HONESTY_MASTER_DB)"; \
	fi

# Shape-inventory scan (epic #40 S6, ADR 0091 dec. 4; docs/testing.md § Synthetic
# shape corpus): rewrites src/test/scenarios/shape-inventory.json from the
# maintainer's database — ANONYMIZED SHAPE DESCRIPTORS ONLY, never row content.
#
# LOCAL ONLY and NOT a `check` step: the inventory changes only when the real
# data grows a state it never had. Run it when that is plausible (after a big
# ingestion, a new job kind, a new reason code) and REVIEW THE DIFF before
# committing. The coverage gate over the synthetic corpus then runs in normal CI
# and reddens on any newly appeared shape the corpus does not reach.
#
# Reuses the honesty throwaway copy (the harness migrates what it opens and
# refuses the master snapshot / the live application DB).
shape-inventory-scan:
	@if [ -f $(HONESTY_MASTER_DB) ]; then \
		printf "shape-inventory-scan: refreshing the throwaway copy %s\n" "$(HONESTY_DB)"; \
		cp $(HONESTY_MASTER_DB) $(HONESTY_DB) && \
		$(NIX) bash -c 'cd src-tauri && BRAWLER_REAL_DB=$(abspath $(HONESTY_DB)) cargo test real_data_shape_inventory_scan -- --ignored --nocapture'; \
	else \
		printf "\n!! SKIP shape-inventory-scan: no real database at %s.\n!! The scan reads the maintainer's OWN data and runs on the maintainer's machine only (ADR 0091 dec. 4);\n!! the committed src/test/scenarios/shape-inventory.json is the public artefact. Refresh the snapshot per private/realdata/README.md to enable it.\n\n" "$(HONESTY_MASTER_DB)"; \
	fi

# Advisory "worth a live-drive" hint (card #337, ADR 0096 principle 5): a dumb
# path -> hint classifier over the PR's changed files, wired as the always-green
# `live-drive-hint` CI job. Never part of `make check` — it decides only what is
# SAID, never what is CHECKED. BASE/HEAD are commit SHAs; PR is optional (used
# only to fill the suggested command).
live-drive-hints:
	$(NIX) env BASE="$(BASE)" HEAD="$(HEAD)" PR="$(PR)" node scripts/check/live-drive-hints.mjs

# Live-drive (ADR 0066, docs/testing.md § Live drive): drives the REAL packaged
# Windows app — real backend, real local SQLite DB — via WebView2's Chrome
# DevTools Protocol, replacing most manual click-through testing. Requires a live
# app already running with remote debugging enabled (`make live-up`, or
# `scripts/windows/dev-live.ps1` directly on Windows). Deliberately excluded from
# `make check`: it needs a live GUI app reachable over CDP, which no
# default/CI environment has, and it is not hermetic (it can observe/act on
# whatever real data the app currently holds).
LIVE_CDP_PORT ?= 9222
# URL handoff between live-up and live-drive: live-up writes the CDP URL it
# actually verified reachable (localhost, or the WSL2-NAT Windows-host IP) to
# this file; live-drive exports it as BRAWLER_CDP_URL when the env var isn't
# already set. /tmp, not the repo tree, so it never pollutes git status.
LIVE_CDP_URL_FILE := /tmp/brawler-live-cdp-url

# Optional scoped live spec (Q6, ADR 0081). Empty = the full historical live
# suite (unchanged default). Set to one spec path to drive a single UX
# checkpoint against the real app, e.g.
#   make live-cycle LIVE_SPEC=tests/live/ux-checkpoint.live.spec.ts
LIVE_SPEC ?=

live-drive:
	@url="$${BRAWLER_CDP_URL:-$$(cat $(LIVE_CDP_URL_FILE) 2>/dev/null || true)}"; \
	if [ -n "$$url" ]; then \
		printf "live-drive: connecting to %s\n" "$$url"; \
		BRAWLER_CDP_URL="$$url" $(NIX) npx playwright test --config playwright.live.config.ts $(LIVE_SPEC); \
	else \
		$(NIX) npx playwright test --config playwright.live.config.ts $(LIVE_SPEC); \
	fi

# One command from WSL: rebuild the portable exe, launch it on Windows with the
# CDP port open, and wait until the endpoint answers. package-windows-from-linux
# already force-stops any running brawler* process (via powershell.exe) before
# replacing the artifact, so re-running live-up always tests the fresh build.
live-up:
	$(MAKE) package-windows-from-linux
	@if ! command -v powershell.exe >/dev/null 2>&1; then \
		printf "powershell.exe not found. live-up is intended for WSL on Windows.\n"; \
		exit 1; \
	fi
	@SCRIPT="$$(wslpath -w scripts/windows/dev-live.ps1)"; \
	OUT_WIN="$$(wslpath -w "$(WINDOWS_OUT_DIR)")"; \
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$$SCRIPT" -Port $(LIVE_CDP_PORT) -OutputDir "$$OUT_WIN"
	$(MAKE) live-wait

# CDP-ready wait loop, shared by live-up (after a rebuild+relaunch) and
# pr-live-cycle (after launching an already-built PR exe) — extracted so
# neither has to duplicate it. Probes localhost first, then the Windows-host IP
# from /etc/resolv.conf's nameserver line — the same resolution order as
# tests/live/helpers/liveConnect.ts.
live-wait:
	@set -e; \
	rm -f "$(LIVE_CDP_URL_FILE)"; \
	gw_ip="$$(ip route show default 2>/dev/null | sed -n 's/^default via \([^[:space:]]*\).*/\1/p' | head -1)"; \
	ns_ip="$$(sed -n 's/^nameserver[[:space:]]*\([^[:space:]]*\).*/\1/p' /etc/resolv.conf 2>/dev/null | head -1)"; \
	deadline=$$((SECONDS + 90)); url=""; \
	while [ $$SECONDS -lt $$deadline ]; do \
		for cand in "http://localhost:$(LIVE_CDP_PORT)" $${gw_ip:+"http://$$gw_ip:$(LIVE_CDP_PORT)"} $${ns_ip:+"http://$$ns_ip:$(LIVE_CDP_PORT)"}; do \
			if curl -sf --max-time 2 "$$cand/json/version" >/dev/null 2>&1; then url="$$cand"; break 2; fi; \
		done; \
		sleep 2; \
	done; \
	if [ -z "$$url" ]; then \
		printf "✖ live-wait: no CDP endpoint reachable on port $(LIVE_CDP_PORT) after 90s.\n"; \
		printf "  Checked http://localhost:$(LIVE_CDP_PORT)/json/version"; \
		if [ -n "$$gw_ip" ]; then printf ", http://%s:$(LIVE_CDP_PORT) (default gw)" "$$gw_ip"; fi; \
		if [ -n "$$ns_ip" ]; then printf ", http://%s:$(LIVE_CDP_PORT) (resolv.conf)" "$$ns_ip"; fi; \
		printf ".\n  Did the app window open on Windows? If yes, Windows Defender Firewall may be\n"; \
		printf "  blocking WSL->Windows traffic (allow it, or enable WSL mirrored networking),\n"; \
		printf "  or set BRAWLER_CDP_URL to a reachable endpoint manually.\n"; \
		exit 1; \
	fi; \
	printf "%s" "$$url" > "$(LIVE_CDP_URL_FILE)"; \
	printf "live app ready — run \`make live-drive\` (CDP: %s, saved to %s)\n" "$$url" "$(LIVE_CDP_URL_FILE)"

# Full cycle: rebuild + launch + wait (live-up), then run the live suite.
live-cycle:
	$(MAKE) live-up
	$(MAKE) live-drive

# Drive a PR's cross-built exe against the owner's REAL app data (#337, ADR
# 0096 principle 5): the exe must sit in latest/ because the portable data dir
# is the exe's parent (src-tauri/src/data_directory.rs). The versioned release
# exe in latest/ is left untouched — the PR binary lands as brawler-pr-live.exe.
pr-live-cycle:
	@test -n "$(PR)" || { printf "Usage: make pr-live-cycle PR=<number> [LIVE_SPEC=<spec>]\n" >&2; exit 64; }
	$(MAKE) pr-binary PR=$(PR)
	@printf "⚠ pr-live-cycle: launching PR #$(PR)'s binary against the LIVE database in $(WINDOWS_OUT_DIR)/data —\n"
	@printf "⚠ pending migrations in the PR will be applied to it. latest/ keeps running this PR binary until the next live-cycle/release copy.\n"
	@if command -v powershell.exe >/dev/null 2>&1; then \
		powershell.exe -ExecutionPolicy Bypass -Command '$$ErrorActionPreference = "SilentlyContinue"; Get-Process | Where-Object { $$_.ProcessName -like "brawler*" } | Stop-Process -Force; exit 0'; \
	fi
	@set -e; \
	src_dir="/mnt/d/Brawler/Builds/pr-$(PR)"; \
	matches="$$(find "$$src_dir" -maxdepth 1 -type f -name 'brawler-*portable.exe' ! -name 'brawler-mcp-stdio.exe')"; \
	count="$$(printf '%s\n' "$$matches" | grep -c .)"; \
	if [ "$$count" -ne 1 ]; then \
		printf "pr-live-cycle: expected exactly one portable .exe under %s, found %d:\n%s\n" "$$src_dir" "$$count" "$$matches" >&2; \
		exit 1; \
	fi; \
	mkdir -p "$(WINDOWS_OUT_DIR)"; \
	cp -f "$$matches" "$(WINDOWS_OUT_DIR)/brawler-pr-live.exe.staging"; \
	mv -f "$(WINDOWS_OUT_DIR)/brawler-pr-live.exe.staging" "$(WINDOWS_OUT_DIR)/brawler-pr-live.exe"; \
	printf "Copied PR #$(PR) binary to %s/brawler-pr-live.exe\n" "$(WINDOWS_OUT_DIR)"
	@if ! command -v powershell.exe >/dev/null 2>&1; then \
		printf "powershell.exe not found. pr-live-cycle is intended for WSL on Windows.\n"; \
		exit 1; \
	fi
	@SCRIPT="$$(wslpath -w scripts/windows/dev-live.ps1)"; \
	EXE_WIN="$$(wslpath -w "$(WINDOWS_OUT_DIR)/brawler-pr-live.exe")"; \
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$$SCRIPT" -Port $(LIVE_CDP_PORT) -ExePath "$$EXE_WIN"
	$(MAKE) live-wait
	$(MAKE) live-drive

# Boot smoke against a GIVEN portable .exe (#206, ADR 0090 §B2): launch it with
# the CDP port open, wait for the endpoint, run the clean-DB-safe boot spec
# (tests/live/boot-smoke.live.spec.ts), always stop the app. Runs in TWO very
# different environments with one recipe: a windows-latest runner (Git Bash —
# no nix, no wslpath, cygpath present) against the windows-build artifact, and
# the owner's WSL (nix + wslpath) against any built exe. Hence the runtime
# wrapper/path probes instead of $(NIX): this target is deliberately the one
# nix-less make entry point, because it runs where the .exe runs.
live-smoke:
	@test -n "$(EXE)" || { printf "Usage: make live-smoke EXE=<portable .exe>\n" >&2; exit 64; }
	@test -f "$(EXE)" || { printf "live-smoke: EXE not found: %s\n" "$(EXE)" >&2; exit 66; }
	@set -e; \
	if command -v nix >/dev/null 2>&1; then RUN="env -u LD_LIBRARY_PATH nix develop -c"; else RUN=""; fi; \
	if [ ! -d node_modules ]; then $$RUN npm ci; fi; \
	if command -v wslpath >/dev/null 2>&1; then EXE_WIN="$$(wslpath -w "$(EXE)")"; \
	elif command -v cygpath >/dev/null 2>&1; then EXE_WIN="$$(cygpath -w "$(EXE)")"; \
	else EXE_WIN="$(EXE)"; fi; \
	stop_app() { powershell.exe -NoProfile -Command "Get-Process brawler* -ErrorAction SilentlyContinue | Stop-Process -Force" >/dev/null 2>&1 || true; }; \
	trap stop_app EXIT; \
	SCRIPT_PATH="scripts/windows/dev-live.ps1"; \
	if command -v wslpath >/dev/null 2>&1; then SCRIPT_PATH="$$(wslpath -w "$$SCRIPT_PATH")"; \
	elif command -v cygpath >/dev/null 2>&1; then SCRIPT_PATH="$$(cygpath -w "$$SCRIPT_PATH")"; fi; \
	EXE_DIR="$$(dirname "$(EXE)")"; CAP_DIR="$$EXE_DIR/live-capture"; \
	if command -v wslpath >/dev/null 2>&1; then CAP_WIN="$$(wslpath -w "$$CAP_DIR")"; \
	elif command -v cygpath >/dev/null 2>&1; then CAP_WIN="$$(cygpath -w "$$CAP_DIR")"; \
	else CAP_WIN="$$CAP_DIR"; fi; \
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$$SCRIPT_PATH" -Port $(LIVE_CDP_PORT) -ExePath "$$EXE_WIN" -CaptureDir "$$CAP_WIN" -DebugPolicyFallback; \
	deadline=$$((SECONDS + 120)); up=""; \
	while [ $$SECONDS -lt $$deadline ]; do \
	  if curl -fsS "http://localhost:$(LIVE_CDP_PORT)/json/version" >/dev/null 2>&1; then up=1; break; fi; \
	  sleep 2; \
	done; \
	if [ -z "$$up" ]; then \
	  printf "live-smoke: CDP endpoint never came up — the exe did not boot. Diagnostics:\n" >&2; \
	  powershell.exe -NoProfile -Command "Get-Process brawler* -ErrorAction SilentlyContinue | Format-Table Id,ProcessName,StartTime" >&2 || true; \
	  printf "WebView2 runtime version (empty = runtime absent):\n" >&2; \
	  powershell.exe -NoProfile -Command "(Get-ItemProperty 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' -ErrorAction SilentlyContinue).pv" >&2 || true; \
	  printf "msedgewebview2 child processes (empty = the webview never spawned):\n" >&2; \
	  powershell.exe -NoProfile -Command "Get-Process msedgewebview2* -ErrorAction SilentlyContinue | Format-Table Id,ProcessName,StartTime" >&2 || true; \
	  printf "msedgewebview2 command lines (is --remote-debugging-port present?):\n" >&2; \
	  powershell.exe -NoProfile -Command "Get-CimInstance Win32_Process | Where-Object Name -eq 'msedgewebview2.exe' | Select-Object -ExpandProperty CommandLine" >&2 || true; \
	  printf "Listeners on the CDP port:\n" >&2; \
	  powershell.exe -NoProfile -Command "Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue | Where-Object LocalPort -eq $(LIVE_CDP_PORT) | Format-Table LocalAddress,LocalPort,OwningProcess" >&2 || true; \
	  for f in "$$CAP_DIR"/*; do [ -s "$$f" ] && { printf -- "--- %s\n" "$$f" >&2; tail -40 "$$f" >&2; }; done || true; \
	  printf "WebView2 user-data traces near the exe (EBWebView/crash dumps):\n" >&2; \
	  find "$$EXE_DIR" -maxdepth 5 \( -name "EBWebView" -o -name "*.dmp" \) 2>/dev/null >&2 || true; \
	  for f in "$$EXE_DIR"/data/logs/*; do [ -f "$$f" ] && { printf -- "--- %s (tail)\n" "$$f" >&2; tail -60 "$$f" >&2; }; done || true; \
	  exit 1; \
	fi; \
	BRAWLER_CDP_URL="http://localhost:$(LIVE_CDP_PORT)" $$RUN npx playwright test --config playwright.live.config.ts tests/live/boot-smoke.live.spec.ts

flake-check:
	nix flake check --no-build

tauri-build:
	$(NIX) npm run tauri -- build $(RELEASE_FEATURE_FLAG)

package-linux-amd64:
	$(NIX) npm run tauri -- build --bundles deb,rpm $(RELEASE_FEATURE_FLAG)
	APPIMAGE_EXTRACT_AND_RUN=1 npm run tauri -- build --bundles appimage --verbose $(RELEASE_FEATURE_FLAG)
	$(NIX) scripts/release/collect-linux-artifacts.sh "$(APP_VERSION)" "$(RELEASE_OUT_DIR)"

package-windows-from-linux:
	$(NIX_WINDOWS) npm run tauri -- build --runner cargo-xwin --target $(WINDOWS_TARGET) --no-bundle $(RELEASE_FEATURE_FLAG)
	@if [ ! -f "$(WINDOWS_EXE)" ]; then \
		printf "Expected Windows executable not found: $(WINDOWS_EXE)\n"; \
		exit 1; \
	fi
	@if command -v powershell.exe >/dev/null 2>&1; then \
		powershell.exe -ExecutionPolicy Bypass -Command '$$ErrorActionPreference = "SilentlyContinue"; Get-Process | Where-Object { $$_.ProcessName -like "brawler*" } | Stop-Process -Force; exit 0'; \
	fi
	@mkdir -p "$(WINDOWS_OUT_DIR)"
	@rm -f "$(WINDOWS_OUT_DIR)/brawler.exe"
	@cp -f "$(WINDOWS_EXE)" "$(WINDOWS_ARTIFACT)"
	@printf "Copied portable Windows executable to %s\n" "$(WINDOWS_ARTIFACT)"
	@cp -f "$(WINDOWS_STDIO_EXE)" "$(WINDOWS_OUT_DIR)/brawler-mcp-stdio.exe"
	@printf "Copied MCP stdio adapter to %s\n" "$(WINDOWS_OUT_DIR)/brawler-mcp-stdio.exe"

package-windows-portable-zip:
	$(NIX_WINDOWS) npm run tauri -- build --runner cargo-xwin --target $(WINDOWS_TARGET) --no-bundle $(RELEASE_FEATURE_FLAG)
	$(NIX_WINDOWS) scripts/release/package-windows-portable-zip.sh "$(APP_VERSION)" "$(WINDOWS_EXE)" "$(RELEASE_OUT_DIR)"

# Build release artifacts. VERSION (optional) stamps the version into the
# manifests at build time before packaging (ADR 0090 §D pt 2) — no commit. The
# stamp runs first, then the artifact builds go through recursive `$(MAKE)` so
# the derived APP_VERSION (artifact filenames) is re-read from the just-stamped
# package.json. Without VERSION it builds at the current manifest version.
# The per-side targets exist so CI can build Linux and Windows IN PARALLEL on
# two runners (release.yml build-linux / build-windows); the composite target
# stays the sequential local-parity path.
package-release-linux:
	@if [ -n "$(VERSION)" ] && [ "$(VERSION)" != "$(APP_VERSION)" ]; then \
		$(NIX) node scripts/release/bump-version.mjs "$(VERSION)"; \
	fi
	$(MAKE) package-linux-amd64

package-release-windows:
	@if [ -n "$(VERSION)" ] && [ "$(VERSION)" != "$(APP_VERSION)" ]; then \
		$(NIX) node scripts/release/bump-version.mjs "$(VERSION)"; \
	fi
	$(MAKE) package-windows-portable-zip

package-release-artifacts:
	$(MAKE) package-release-linux VERSION=$(VERSION)
	$(MAKE) package-release-windows VERSION=$(VERSION)

package-windows-smoke-run:
	@if [ ! -f "$(WINDOWS_ARTIFACT)" ]; then \
		printf "Expected portable Windows executable not found: $(WINDOWS_ARTIFACT)\n"; \
		printf "Run 'make package-windows-from-linux' first.\n"; \
		exit 1; \
	fi
	@if command -v powershell.exe >/dev/null 2>&1; then \
		EXE_WIN="$$(wslpath -w "$(WINDOWS_ARTIFACT)")"; \
		DIR_WIN="$$(wslpath -w "$(WINDOWS_OUT_DIR)")"; \
		powershell.exe -ExecutionPolicy Bypass -Command "Start-Process -FilePath '$$EXE_WIN' -WorkingDirectory '$$DIR_WIN'" ; \
	else \
		printf "powershell.exe not found; cannot launch the Windows executable from this environment.\n"; \
		exit 1; \
	fi

windows-package:
	@if ! command -v powershell.exe >/dev/null 2>&1; then \
		printf "powershell.exe not found. This target is intended for WSL on Windows.\n"; \
		exit 1; \
	fi
	@SCRIPT="$$(wslpath -w scripts/windows/package.ps1)"; \
	ARGS=(); \
	if [ -n "$${BRAWLER_WINDOWS_REPO:-}" ]; then ARGS+=("-WindowsRepo" "$$(wslpath -w "$$BRAWLER_WINDOWS_REPO")"); fi; \
	if [ -n "$${BRAWLER_WINDOWS_OUT:-}" ]; then ARGS+=("-OutputDir" "$$(wslpath -w "$$BRAWLER_WINDOWS_OUT")"); fi; \
	powershell.exe -ExecutionPolicy Bypass -File "$$SCRIPT" "$${ARGS[@]}" -NoRun

windows-package-no-run:
	@if ! command -v powershell.exe >/dev/null 2>&1; then \
		printf "powershell.exe not found. This target is intended for WSL on Windows.\n"; \
		exit 1; \
	fi
	@SCRIPT="$$(wslpath -w scripts/windows/package.ps1)"; \
	ARGS=(); \
	if [ -n "$${BRAWLER_WINDOWS_REPO:-}" ]; then ARGS+=("-WindowsRepo" "$$(wslpath -w "$$BRAWLER_WINDOWS_REPO")"); fi; \
	if [ -n "$${BRAWLER_WINDOWS_OUT:-}" ]; then ARGS+=("-OutputDir" "$$(wslpath -w "$$BRAWLER_WINDOWS_OUT")"); fi; \
	powershell.exe -ExecutionPolicy Bypass -File "$$SCRIPT" "$${ARGS[@]}" -NoRun

windows-test-help:
	@printf "Windows hands-on sanity testing\n\n"
	@printf "WSL without GUI should run automated checks only:\n"
	@printf "  make check\n"
	@printf "  make build\n\n"
	@printf "For complete packaged app testing from WSL, use the default D:\\Brawler checkout and run:\n"
	@printf "  make package-windows-from-linux\n"
	@printf "  make package-windows-smoke-run\n\n"
	@printf "Fallback if cross-building does not work yet:\n"
	@printf "  make windows-package\n"
	@printf "  make windows-package-no-run\n\n"
	@printf "Override with BRAWLER_WINDOWS_REPO and BRAWLER_WINDOWS_OUT when needed.\n\n"
	@printf "For native dev-mode testing, use a Windows checkout or worktree and run:\n"
	@printf "  powershell -ExecutionPolicy Bypass -File scripts/windows/dev.ps1\n\n"
	@printf "Why: a Tauri build from WSL is a Linux app. It is not a Windows .exe, and mixing\n"
	@printf "Windows and Linux node_modules/target directories in one working tree causes churn.\n\n"
	@printf "For a quick non-native UI preview from WSL, run:\n"
	@printf "  make build\n"
	@printf "  make frontend-preview\n"
	@printf "Then open the printed localhost URL in Windows. Tauri APIs are not validated there.\n"

open-project-windows:
	@if command -v explorer.exe >/dev/null 2>&1; then explorer.exe .; else printf "explorer.exe not found. This target is intended for WSL on Windows.\n"; fi

open-dist-windows:
	@if [ -d dist ]; then \
		if command -v explorer.exe >/dev/null 2>&1; then explorer.exe dist; else printf "explorer.exe not found. This target is intended for WSL on Windows.\n"; fi; \
	else \
		printf "dist/ does not exist. Run 'make build' first.\n"; \
	fi
