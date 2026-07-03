# ADR 0024: Cross-Platform Release Artifacts

Status: Accepted for Milestone 28 implementation.

## Context

Brawler is now public on GitHub while Radicle remains the canonical forge for source, issues, and patches. Public users need practical binary downloads without building the app themselves.

The project already has a Windows portable executable path built from Linux/WSL with `cargo-xwin`. The next distribution step should add Linux `amd64` packages and publish release binaries without requiring paid Windows or macOS CI runners.

## Decision

- GitHub Releases are the public binary mirror for release artifacts.
- Radicle remains the canonical project forge and source-of-truth for issues, patches, and source refs.
- Pushing a `v*` tag to GitHub starts release artifact packaging on standard `ubuntu-latest`.
- The release workflow publishes:
  - Linux `amd64` `.deb`
  - Linux `amd64` `.rpm`
  - Linux `amd64` `.AppImage`
  - Windows `x64` portable `.zip`
- Windows portable artifacts are built from Linux with `cargo-xwin`, not with a Windows runner, unless the Linux cross-build path proves unreliable after real implementation attempts.
- Windows portable zip contents are intentionally small: `brawler.exe` plus a portable-readme file. It must not include user data, secrets, logs, or sample databases.
- `.deb` and `.rpm` packaging use the Nix-wrapped Tauri build path. AppImage packaging uses the host Ubuntu toolchain because the AppImage bundler depends on `linuxdeploy` runtime dependency discovery, which does not reliably resolve WebKitGTK libraries from the Nix store.
- Windows portable release builds keep using `data/` next to the executable.
- Linux release builds use `~/.brawler` for local runtime data. Installed Linux packages must not try to write user data beside `/usr/bin`, `/opt`, or another package-managed location.
- Linux runtime startup sets a WSL-only WebKitGTK compatibility fallback before the webview starts: when WSL is detected and the user has not already configured `WEBKIT_DISABLE_DMABUF_RENDERER`, Brawler sets it to `1`. This avoids known WSLg/EGL startup failures while preserving default WebKitGTK behavior on normal Linux desktops and preserving explicit user overrides.
- AppImage is the Arch-friendly artifact for now. Native Pacman packaging is deferred until there is enough demand to justify a separate package adapter.
- macOS and macOS arm64 packaging remain out of scope, but the workflow should keep platform jobs separable so a later macOS job can be added without changing artifact semantics.

## Consequences

- Public users get normal downloadable release assets from GitHub while project governance remains Radicle-first.
- GitHub Actions usage stays on standard public-repo Linux runners and avoids paid Windows/macOS runners.
- Linux packaging now has a real installable package path, while Arch users can use AppImage until native Pacman packaging is designed.
- Linux and Windows data-location policies intentionally differ because installed packages and portable executables have different filesystem constraints.
- WSL can run the Linux packages for smoke testing, but it remains a compatibility environment. Native Linux desktops are still the main target for Linux GUI behavior, and Windows remains the primary hands-on runtime target for the project owner.
- Future code signing, Windows installers, package repositories, native Pacman packaging, and macOS distribution remain separate decisions.
