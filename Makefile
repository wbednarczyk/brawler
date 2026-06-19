SHELL := /usr/bin/env bash
.DEFAULT_GOAL := help

NIX := env -u LD_LIBRARY_PATH nix develop -c
NIX_WINDOWS := env -u LD_LIBRARY_PATH nix develop .\#windows-cross -c
WINDOWS_TARGET := x86_64-pc-windows-msvc
# Cargo features compiled into shipped/packaged builds. The on-device embedding
# model (ADR 0035) is off by default for fast/offline dev + `make check`, but the
# packaged app enables it. Override with `make ... RELEASE_FEATURES=` to omit it.
RELEASE_FEATURES ?= embedding-model
RELEASE_FEATURE_FLAG := $(if $(RELEASE_FEATURES),--features $(RELEASE_FEATURES))
RELEASE_OUT_DIR ?= release-artifacts
WINDOWS_OUT_DIR ?= /mnt/d/Brawler/Builds/latest
WINDOWS_EXE := src-tauri/target/$(WINDOWS_TARGET)/release/brawler.exe
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

.PHONY: help install dev frontend-preview build check check-epic test ui-smoke ui-smoke-install typecheck frontend-check rust-check install-git-hooks commit-msg-check version-check changelog changelog-check release-notes release-check release-prepare release license-keygen-author license-author license-friend smoke-gemini-transcript smoke-gemini-analysis smoke-keyring flake-check tauri-build package-linux-amd64 package-windows-from-linux package-windows-portable-zip package-windows-smoke-run package-release-artifacts windows-package windows-package-no-run windows-test-help open-project-windows open-dist-windows

help:
	@printf "Brawler developer commands\n\n"
	@printf "  make install             Install npm dependencies inside nix develop\n"
	@printf "  make check               Run the full local automated check suite inside nix develop\n"
	@printf "  make check-epic          Epic-closure suite: gate + knip + Playwright smoke (run-and-triage)\n"
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

check:
	$(NIX) npm run check

# Staged concurrent check (ADR 0048): fast-fail static stage, then the heavy
# suites (Rust clippy+nextest+doc, Vitest, build) concurrently — overlaps the
# Rust compile with the JS suites. Opt-in until a measured win promotes it to
# the default; `make check` stays the sequential release-gate parity path.
check-fast:
	$(NIX) npm run check:parallel

# Full epic/milestone-closure suite: the hard gate first, then the opt-in/periodic
# suites (knip dead-code audit, Playwright browser UI smoke) that are NOT in
# `make check` and otherwise rot unrun (see docs/engineering-workflow.md and
# ADR 0045). The gate is hard (aborts on failure); knip and the browser smoke are
# run-and-report (leading `-`) so you always see every result — triage each
# failure (fix it, or file a tracked Radicle issue) before signing off on closure.
# Scoped to the epic boundary, not per-change: ~1–2 min beyond `make check`.
check-epic:
	$(NIX) npm run check
	-$(NIX) npm run knip
	-$(NIX) npm run test:browser:install
	-$(NIX) npm run test:browser
	@printf "\ncheck-epic complete. Triage any knip/browser-smoke findings above (fix or file an issue) before closing the epic.\n"

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
