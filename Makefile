SHELL := /usr/bin/env bash
.DEFAULT_GOAL := help

NIX := nix develop -c
NIX_WINDOWS := nix develop .\#windows-cross -c
WINDOWS_TARGET := x86_64-pc-windows-msvc
WINDOWS_OUT_DIR ?= /mnt/d/Brawler/Builds/latest
WINDOWS_EXE := src-tauri/target/$(WINDOWS_TARGET)/release/brawler.exe

.PHONY: help install dev frontend-preview build check test typecheck frontend-check rust-check license-keygen-author license-author license-friend smoke-gemini-transcript smoke-gemini-analysis smoke-keyring flake-check tauri-build package-windows-from-linux windows-package windows-package-no-run windows-test-help open-project-windows open-dist-windows

help:
	@printf "Brawler developer commands\n\n"
	@printf "  make install             Install npm dependencies inside nix develop\n"
	@printf "  make check               Run the full local automated check suite inside nix develop\n"
	@printf "  make test                Run frontend tests inside nix develop\n"
	@printf "  make build               Build the frontend inside nix develop\n"
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
	@printf "  make package-windows-from-linux\n"
	@printf "                            Experimental: build Windows app from Linux/WSL\n"
	@printf "  make windows-package     Build, copy, and run the packaged Windows app via PowerShell\n"
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

test:
	$(NIX) npm run test

typecheck:
	$(NIX) npm run typecheck

frontend-check:
	$(NIX) npm run check:frontend

rust-check:
	$(NIX) npm run check:rust

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
	$(NIX) npm run tauri -- build

package-windows-from-linux:
	BRAWLER_DEVELOPER_UNLOCK_CODE="dup dup dupa" $(NIX_WINDOWS) npm run tauri -- build --runner cargo-xwin --target $(WINDOWS_TARGET) --no-bundle
	@if [ ! -f "$(WINDOWS_EXE)" ]; then \
		printf "Expected Windows executable not found: $(WINDOWS_EXE)\n"; \
		exit 1; \
	fi
	@if command -v powershell.exe >/dev/null 2>&1; then \
		powershell.exe -ExecutionPolicy Bypass -Command '$$ErrorActionPreference = "SilentlyContinue"; Stop-Process -Name brawler; exit 0'; \
	fi
	@mkdir -p "$(WINDOWS_OUT_DIR)"
	@cp -f "$(WINDOWS_EXE)" "$(WINDOWS_OUT_DIR)/brawler.exe"
	@printf "Copied Windows executable to %s\n" "$(WINDOWS_OUT_DIR)/brawler.exe"
	@if command -v powershell.exe >/dev/null 2>&1; then \
		EXE_WIN="$$(wslpath -w "$(WINDOWS_OUT_DIR)/brawler.exe")"; \
		DIR_WIN="$$(wslpath -w "$(WINDOWS_OUT_DIR)")"; \
		powershell.exe -ExecutionPolicy Bypass -Command "\$$env:BRAWLER_DEVELOPER_UNLOCK_CODE = 'dup dup dupa'; \
														 \$$env:BRAWLER_DEVELOPER_MODE = "1"; \
														 Start-Process -FilePath '$$EXE_WIN' -WorkingDirectory '$$DIR_WIN'" ; \
	else \
		printf "powershell.exe not found; copied artifact but did not launch it.\n"; \
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
	powershell.exe -ExecutionPolicy Bypass -File "$$SCRIPT" "$${ARGS[@]}"

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
	@printf "  make package-windows-from-linux\n\n"
	@printf "Fallback if cross-building does not work yet:\n"
	@printf "  make windows-package\n\n"
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
