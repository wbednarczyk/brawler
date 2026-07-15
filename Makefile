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
RELEASE_FILES := CHANGELOG.md docs/kanban-archive.md docs/kanban.md docs/roadmap.md package-lock.json package.json src-tauri/Cargo.lock src-tauri/Cargo.toml src-tauri/src/lib.rs src-tauri/tauri.conf.json

.PHONY: commit help install dev frontend-preview build check check-fast check-docs disk-clean disk-clean-deep coverage bench report-escaped-defects ux-contact-sheet visual-update mutants types types-check check-epic test ui-smoke ui-smoke-install typecheck frontend-check rust-check install-git-hooks commit-msg-check version-check changelog changelog-check release-notes release-check release-prepare release license-keygen-author license-author license-friend smoke-gemini-transcript smoke-gemini-analysis smoke-keyring live-drive live-up live-cycle flake-check tauri-build package-linux-amd64 package-windows-from-linux package-windows-portable-zip package-windows-smoke-run package-release-artifacts windows-package windows-package-no-run windows-test-help open-project-windows open-dist-windows

help:
	@printf "Brawler developer commands\n\n"
	@printf "  make install             Install npm dependencies inside nix develop\n"
	@printf "  make check               The single mandatory gate: all deterministic suites, hard-fail (pre-commit runs this)\n"
	@printf "  make check-fast          Fast inner-loop check (parallel core, no browser) — iteration only, NOT proof of done\n"
	@printf "  make check-docs          Docs-only gate: mandatory-read budgets + docs drift, no code suites (pre-commit uses this for docs-only commits)\n"
	@printf "  make disk-clean          Safe temp cleanup: caches, mutants artifacts, old nix generations, journal, fstrim\n"
	@printf "  make disk-clean-deep     disk-clean + cargo target dir (full rebuild next time) + full nix GC\n"
	@printf "  make check-epic          Closure suite: the full gate + heavy periodic suites (coverage ratchet)\n"
	@printf "  make report-escaped-defects\n"
	@printf "                            Advisory escaped-defect taxonomy trend report (ADR 0081 Q7), never part of check\n"
	@printf "  make ux-contact-sheet SCREENS=\"a,b\" (or CHANGED=1)\n"
	@printf "                            Assemble a local HTML contact sheet from the existing visual scenarios (ADR 0081 Q5)\n"
	@printf "  make visual-update SCREEN=<name> REASON=\"why\"\n"
	@printf "                            Deliberate Playwright baseline update — refuses without both\n"
	@printf "  make test                Run frontend tests inside nix develop\n"
	@printf "  make ui-smoke-install    Download Chromium for opt-in Playwright smoke tests\n"
	@printf "  make ui-smoke            Run opt-in Playwright browser UI smoke tests\n"
	@printf "  make build               Build the frontend inside nix develop\n"
	@printf "  make install-git-hooks   Install repo-local commit message hooks\n"
	@printf "  make release-check       Validate release workflow guardrails\n"
	@printf "  make changelog           Generate future changelog entries with git-cliff\n"
	@printf "  make changelog-check     Validate git-cliff changelog generation\n"
	@printf "  make release-notes TAG=vX.Y.Z\n"
	@printf "                            Print the changelog entry used for GitHub Release notes\n"
	@printf "  make release-prepare VERSION=X.Y.Z\n"
	@printf "                            Bump version + generate the changelog scaffold, then stop for curation\n"
	@printf "  make release VERSION=X.Y.Z\n"
	@printf "                            Finalize: validate, commit, tag, and push (bumps + generates changelog if not prepared)\n"
	@printf "  make dev                 Start Tauri dev mode inside nix develop, requires Linux GUI/WSLg\n"
	@printf "  make frontend-preview    Serve built frontend preview to Windows browser, not native Tauri\n"
	@printf "  make license-keygen-author\n"
	@printf "                            Generate the external author Ed25519 key if missing\n"
	@printf "  make license-author      Generate an author license token under private/licenses\n"
	@printf "  make license-friend HOLDER=\"Friend Name\"\n"
	@printf "                            Generate a friend-test license token under private/licenses\n"
	@printf "  make smoke-gemini-transcript\n"
	@printf "                            Opt-in live Gemini YouTube transcript smoke test\n"
	@printf "  make smoke-gemini-analysis\n"
	@printf "                            Opt-in live Gemini feed-item analysis smoke test\n"
	@printf "  make smoke-keyring        Opt-in live OS keyring persistence smoke test\n"
	@printf "  make live-up             Rebuild the portable exe, launch it on Windows with CDP open, wait until ready (ADR 0066)\n"
	@printf "  make live-drive          Drive the real running Windows app via WebView2 CDP (ADR 0066), needs a live app\n"
	@printf "  make live-cycle          live-up + live-drive: one command from WSL to rebuild, launch, and test live\n"
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

# The SINGLE mandatory gate (ADR 0062). Every deterministic/hermetic suite runs
# here as a hard-fail step — nothing exit-ignored — and `.githooks/pre-commit`
# runs this whole target before every commit, so a green commit is proof the gate
# passed. This is a deliberate promotion (ADR 0048 Decision 6) of the browser
# suite + knip + ts-rs drift guard from the old closure-only cadence into the
# per-commit gate, because a suite that is not a hard-fail step of the one gate
# ROTS (the browser suite went 28-red for two sessions in `check-epic`, masked by
# `-`-prefixed steps). Suites deliberately EXCLUDED stay periodic, each for a
# reason that disqualifies it from a per-commit hard gate: `coverage` (slow
# instrumented build), `mutants` (30m–2h), `bench` (machine-dependent wall-clock),
# the live Gemini/keyring smokes (credentials/network/OS), `live-drive` (ADR 0066
# — needs a real running Windows app with remote debugging enabled, unavailable
# in default/CI environments), and packaging (OS/toolchain). gate-integrity fails
# the gate if any mandatory suite is
# dropped or any step is `-`-prefixed (silent red); docs-drift (ADR 0065, last
# step) fails it if contracts.md/ui-information-architecture.md/data-model.md
# drift from the code, or ADR hygiene (Status: lines, INDEX.md) rots.
check:
	@node scripts/check/disk-guard.mjs
	$(NIX) npm run check
	$(NIX) npm run knip
	$(MAKE) types-check
	$(NIX) npm run test:browser:install
	$(NIX) npm run test:browser
	$(NIX) node scripts/check/gate-integrity.mjs
	$(NIX) node scripts/check/docs-drift.mjs

# Docs-only pre-commit gate (ADR 0062): the meta-guards a *documentation* change
# can actually break — the mandatory-read byte budgets + parity (gate-integrity,
# ADR 0063) and cross-doc drift — WITHOUT the code suites (types/lint/test/build/
# browser/knip/ts-rs) that a docs-only change cannot affect. The pre-commit hook
# runs this instead of the full `make check` when the staged changeset is
# docs-only; any code/config change still runs the full gate. Seconds, not minutes.
check-docs:
	@node scripts/check/disk-guard.mjs
	$(NIX) node scripts/check/gate-integrity.mjs
	$(NIX) node scripts/check/docs-drift.mjs

# Staged concurrent check (ADR 0048): fast-fail static stage, then the heavy
# suites (Rust clippy+nextest+doc, Vitest, build) concurrently — overlaps the
# Rust compile with the JS suites. Inner-loop iteration ONLY — it omits knip, the
# ts-rs drift guard, the browser suite, and gate-integrity, so it is NEVER proof
# of done; `make check` is the gate.
check-fast:
	@node scripts/check/disk-guard.mjs
	$(NIX) npm run check:parallel

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

# Coverage measurement + ratchet (ADR 0048): frontend (Vitest v8) + Rust
# (cargo-llvm-cov) line coverage, then fail if either drops below the committed
# floor in coverage-baseline.json. Periodic (slow instrumented Rust build), not
# part of `make check`.
# Rust coverage runs under NEXTEST (process-per-test), not plain `cargo test`
# (threads-in-one-process): env-mutating hermetic tests (credential scrubs,
# BRAWLER_MCP_TOKEN) and the loopback-socket test group rely on process
# isolation + .config/nextest.toml — under threaded cargo-test they race each
# other (5 tests reddened the v0.52 closure run exactly this way, 2026-07-12).
coverage:
	$(NIX) npm run test:coverage
	$(NIX) bash -c 'cd src-tauri && cargo llvm-cov nextest --summary-only --json --output-path ../coverage/rust-summary.json'
	$(NIX) npm run coverage:ratchet

# Periodic micro-benchmarks of the hot data-transform kernels (ADR 0049): the
# similarity scan, RSS parse, and formula parse. Runs criterion, then the
# bench-ratchet flags any kernel that regressed beyond tolerance against
# bench-baseline.json. Machine-dependent and slow — NEVER part of `make check`;
# run on the reference machine and update the baseline deliberately.
bench:
	$(NIX) bash -c 'cd src-tauri && cargo bench --bench transforms'
	$(NIX) npm run bench:ratchet

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
# Uses nextest for a fast per-mutant test pass. Periodic/manual — slow (rebuilds
# + runs the suite per mutant), never in `make check`; run at epic/milestone
# closure cadence and triage every survivor by adding an assertion.
# BRAWLER_FIDELITY_CORPUS: cargo-mutants copies only src-tauri/ into a scratch
# tree, so the mock-fidelity test's corpus path (one level above the workspace)
# doesn't exist there. Export an absolute path to the real file so build.rs's
# fallback (see src-tauri/build.rs) picks it up in every mutant/baseline build.
# Resource caps (harvested 2026-07-03: an uncapped sweep saturated WSL — every
# mutant rebuild used all cores and multi-GB rustc peaks, freezing the desktop).
# Low-priority + bounded build/test parallelism keeps the machine usable; the
# sweep takes longer but never needs killing. Build jobs: MUTANTS_BUILD_JOBS
# (below). Test threads: the `mutants` profile in src-tauri/.config/nextest.toml
# (an env NEXTEST_TEST_THREADS would leak into cargo-mutants' internal
# `nextest --no-run` build call, which rejects it). Override for a dedicated
# box: make mutants MUTANTS_BUILD_JOBS=8 (+ edit the nextest profile).
# MUTANTS_SHARD (e.g. 1/8) runs one shard — split a sweep across quiet moments.
# MUTANTS_MEMORY_MAX: hard memory jail via a systemd user scope — when the
# sweep's builds spike past it, the OOM killer takes the SCOPE, not the whole
# WSL VM (two full-WSL freezes on 2026-07-03 were memory, which nice/job caps
# cannot bound). Requires systemd (WSL: systemctl is-system-running → running).
# MUTANTS_JAIL controls whether the systemd-run jail is used: `auto` (default)
# cheaply probes `systemd-run --user --scope true` and jails only if that
# succeeds — WSL has a user systemd manager, so this is a no-op behavior
# change there; `off` skips the jail unconditionally (a GitHub-hosted
# ubuntu-latest runner has no user systemd manager, so the manual mutants.yml
# workflow passes MUTANTS_JAIL=off instead of relying on the probe to fail).
MUTANTS_BUILD_JOBS ?= 2
MUTANTS_MEMORY_MAX ?= 11G
MUTANTS_SHARD ?=
MUTANTS_JAIL ?= auto
mutants:
	@jail_cmd=""; \
	case "$(MUTANTS_JAIL)" in \
	  off) jail_cmd="" ;; \
	  auto) if systemd-run --user --scope true >/dev/null 2>&1; then \
	          jail_cmd="systemd-run --user --scope -p MemoryMax=$(MUTANTS_MEMORY_MAX) -p MemorySwapMax=1G -p OOMPolicy=continue --same-dir"; \
	        fi ;; \
	  *) echo "MUTANTS_JAIL must be 'auto' or 'off' (got: $(MUTANTS_JAIL))" >&2; exit 1 ;; \
	esac; \
	$$jail_cmd $(NIX) bash -c "export BRAWLER_FIDELITY_CORPUS='$(CURDIR)/src/test/scenarios/fidelity-corpus.json'; \
	  export CARGO_BUILD_JOBS=$(MUTANTS_BUILD_JOBS) NEXTEST_PROFILE=mutants; \
	  cd src-tauri && nice -n 19 cargo mutants --test-tool nextest --jobs 1 \
	  $(if $(MUTANTS_SHARD),--shard $(MUTANTS_SHARD)) \
	  -f 'src/fundamentals/expr/**' \
	  -f 'src/storage/migrations.rs' \
	  -f 'src/storage/feed_matching.rs' \
	  -f 'src/source_adapters/parsing.rs' \
	  -f 'src/entity_resolution.rs'"

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

# Epic/milestone-closure suite (ADR 0062): the full mandatory gate, then the
# heavy periodic deterministic suite too slow for per-commit — the coverage
# ratchet (slow instrumented build). Every step HARD-FAILS (no `-` prefix): a
# closure gate that ignores exit codes is exactly how the browser suite rotted
# silently red. `make mutants` (30m–2h) and `make bench` (machine-dependent) stay
# separate closure-cadence targets (see docs/engineering-workflow.md §I); the
# live/keyring/packaging smokes stay opt-in.
check-epic:
	$(MAKE) check
	$(MAKE) coverage
	@printf "\ncheck-epic complete: full gate + coverage ratchet green. Run \`make mutants\` and (if a hot kernel changed) \`make bench\` for the remaining closure-cadence suites.\n"

test:
	$(NIX) npm run test

ui-smoke-install:
	$(NIX) npm run test:browser:install

ui-smoke:
	$(NIX) npm run test:browser

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

changelog:
	$(NIX) git-cliff --config cliff.toml --unreleased --tag "v$(APP_VERSION)" --prepend CHANGELOG.md

changelog-check:
	$(NIX) npm run release:changelog-check

# Validate the commit message BEFORE the expensive pre-commit gate runs (the
# commit-msg hook fires after make check by git design, so a rejected subject
# wastes a full gate run). Stages nothing: git add what you want first.
# Optional BODY adds a second -m paragraph.
commit:
	@test -n "$(MSG)" || { printf 'Usage: make commit MSG="type(scope): subject (<=72 chars after colon)" [BODY="..."]\n' >&2; exit 64; }
	@scripts/release/validate-commit-message.sh --message "$(MSG)"
	@if [ -n "$(BODY)" ]; then git commit -m "$(MSG)" -m "$(BODY)"; else git commit -m "$(MSG)"; fi

release-notes:
	@test -n "$(TAG)" || { printf "Usage: make release-notes TAG=vX.Y.Z\n" >&2; exit 64; }
	@scripts/release/extract-changelog-entry.sh "$(TAG)" CHANGELOG.md

release-check:
	$(NIX) npm run release:check

# Prepare a release for curation: bump the version and generate the changelog
# scaffold, then STOP. This is the seam the one-shot `release` lacks — it lets the
# `## vX.Y.Z` section be curated into real release notes *before* the release
# commit, so the curated notes land in the tagged commit (the GitHub release notes
# are sliced from CHANGELOG.md at the tag). After curating, run `make release`.
release-prepare:
	@test -n "$(VERSION)" || { printf "Usage: make release-prepare VERSION=X.Y.Z\n" >&2; exit 64; }
	@test "$$(git branch --show-current)" = "master" || { printf "Release target must run from master.\n" >&2; exit 1; }
	@if git rev-parse -q --verify "refs/tags/v$(VERSION)" >/dev/null; then \
		printf "Tag v$(VERSION) already exists.\n" >&2; \
		exit 1; \
	fi
	$(NIX) node scripts/release/assert-release-worktree.mjs
	@if [ "$(VERSION)" != "$(APP_VERSION)" ]; then \
		$(NIX) node scripts/release/bump-version.mjs "$(VERSION)"; \
	else printf "Version already at $(VERSION); skipping bump.\n"; fi
	@if grep -q "^## v$(VERSION) " CHANGELOG.md; then \
		printf "CHANGELOG.md already has v$(VERSION); skipping generation.\n"; \
	else $(MAKE) changelog; fi
	@printf "\nPrepared v$(VERSION). Curate the '## v$(VERSION)' section in CHANGELOG.md into\nuser-facing release notes, then run:  make release VERSION=$(VERSION)\n"

# Finalize a release: validate, make the single chore(release) commit, tag, and
# push. Works whether or not `release-prepare` ran first — the bump and changelog
# steps are skip-if-already-done (so a prepared+curated changelog is preserved),
# and run inline for a one-shot `make release` (which then commits the terse
# scaffold, same as before).
release:
	@test -n "$(VERSION)" || { printf "Usage: make release VERSION=X.Y.Z\n" >&2; exit 64; }
	@test "$$(git branch --show-current)" = "master" || { printf "Release target must run from master.\n" >&2; exit 1; }
	@if git rev-parse -q --verify "refs/tags/v$(VERSION)" >/dev/null; then \
		printf "Tag v$(VERSION) already exists.\n" >&2; \
		exit 1; \
	fi
	$(NIX) node scripts/release/assert-release-worktree.mjs
	@if [ "$(VERSION)" != "$(APP_VERSION)" ]; then \
		$(NIX) node scripts/release/bump-version.mjs "$(VERSION)"; \
	else printf "Version already bumped to $(VERSION) (prepared); skipping bump.\n"; fi
	@if grep -q "^## v$(VERSION) " CHANGELOG.md; then \
		printf "CHANGELOG.md already has v$(VERSION) (prepared/curated); skipping generation.\n"; \
	else $(MAKE) changelog; fi
	$(MAKE) release-check
	$(MAKE) check
	git add $(RELEASE_FILES)
	git commit -m "chore(release): bump version to $(VERSION)"
	git tag -a "v$(VERSION)" -m "v$(VERSION)"
	git push origin master
	git push origin "v$(VERSION)"
	git push rad master
	git push rad "v$(VERSION)"

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

smoke-gemini-analysis:
	@if [ -z "$${GEMINI_API_KEY:-}" ]; then \
		printf "GEMINI_API_KEY is required for the live Gemini analysis smoke test.\n"; \
		exit 1; \
	fi
	@if [ -z "$${BRAWLER_GEMINI_ANALYSIS_SMOKE_SOURCE_URL:-}" ]; then \
		printf "BRAWLER_GEMINI_ANALYSIS_SMOKE_SOURCE_URL is required for the live Gemini analysis smoke test.\n"; \
		exit 1; \
	fi
	@if [ -z "$${BRAWLER_GEMINI_ANALYSIS_SMOKE_TITLE:-}" ]; then \
		printf "BRAWLER_GEMINI_ANALYSIS_SMOKE_TITLE is required for the live Gemini analysis smoke test.\n"; \
		exit 1; \
	fi
	@if [ -z "$${BRAWLER_GEMINI_ANALYSIS_SMOKE_BODY:-}" ]; then \
		printf "BRAWLER_GEMINI_ANALYSIS_SMOKE_BODY is required for the live Gemini analysis smoke test.\n"; \
		exit 1; \
	fi
	$(NIX) cargo test --manifest-path src-tauri/Cargo.toml live_gemini_analyzes_feed_item -- --ignored --nocapture

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

# T0.1 recall/precision ratchet (guardrail G-3, ADR 0077; docs/testing.md
# § Ground-truth metrics): grades the deterministic pipeline's emitted fact
# VALUES against the hand-labeled ground truth in
# private/realdata/t7-cbf/ground_truth/*.json (double-pass: agent proposes,
# owner verifies) and asserts recall/precision against the pinned floors in
# src-tauri/src/storage/tests/extraction_metrics.rs. Inert without the corpus
# or the ground-truth dir. Floors only move deliberately (ratchet).
realdata-extraction-metrics:
	$(NIX) bash -c 'cd src-tauri && BRAWLER_REAL_DB=$(abspath $(REALDATA_DB)) BRAWLER_REAL_DATA_DIR=$(abspath $(REALDATA_DIR)) cargo test extraction_metrics -- --ignored --nocapture'

# Live-drive (ADR 0066, docs/testing.md § Live drive): drives the REAL packaged
# Windows app — real backend, real local SQLite DB — via WebView2's Chrome
# DevTools Protocol, replacing most manual click-through testing. Requires a live
# app already running with remote debugging enabled (`make live-up`, or
# `scripts/windows/dev-live.ps1` directly on Windows). Deliberately excluded from
# `make check`/`check-epic`: it needs a live GUI app reachable over CDP, which no
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
# The wait loop probes localhost first, then the Windows-host IP from
# /etc/resolv.conf's nameserver line — the same resolution order as
# tests/live/helpers/liveConnect.ts.
live-up:
	$(MAKE) package-windows-from-linux
	@if ! command -v powershell.exe >/dev/null 2>&1; then \
		printf "powershell.exe not found. live-up is intended for WSL on Windows.\n"; \
		exit 1; \
	fi
	@SCRIPT="$$(wslpath -w scripts/windows/dev-live.ps1)"; \
	OUT_WIN="$$(wslpath -w "$(WINDOWS_OUT_DIR)")"; \
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$$SCRIPT" -Port $(LIVE_CDP_PORT) -OutputDir "$$OUT_WIN"
	@rm -f "$(LIVE_CDP_URL_FILE)"; \
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
		printf "✖ live-up: no CDP endpoint reachable on port $(LIVE_CDP_PORT) after 90s.\n"; \
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

package-release-artifacts: package-linux-amd64 package-windows-portable-zip

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
