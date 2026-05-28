SHELL := /usr/bin/env bash
.DEFAULT_GOAL := help

NIX := nix develop -c

.PHONY: help install dev frontend-preview build check test typecheck frontend-check rust-check flake-check tauri-build package-windows-from-linux windows-package windows-package-no-run windows-test-help open-project-windows open-dist-windows

help:
	@printf "Brawler developer commands\n\n"
	@printf "  make install             Install npm dependencies inside nix develop\n"
	@printf "  make check               Run the full local automated check suite inside nix develop\n"
	@printf "  make test                Run frontend tests inside nix develop\n"
	@printf "  make build               Build the frontend inside nix develop\n"
	@printf "  make dev                 Start Tauri dev mode inside nix develop, requires Linux GUI/WSLg\n"
	@printf "  make frontend-preview    Serve built frontend preview to Windows browser, not native Tauri\n"
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

flake-check:
	nix flake check --no-build

tauri-build:
	$(NIX) npm run tauri -- build

package-windows-from-linux:
	@printf "package-windows-from-linux is the planned Windows cross-build target.\n"
	@printf "It will use a dedicated Nix shell once the cargo-xwin/NSIS spike is implemented.\n"
	@printf "Current fallback target: make windows-package, which requires Windows-native tooling.\n"
	@exit 2

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
