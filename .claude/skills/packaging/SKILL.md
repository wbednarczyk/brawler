---
name: packaging
description: Build and smoke-test Brawler release artifacts — portable Windows .exe from Linux/WSL via cargo-xwin, Linux .deb/.rpm/AppImage, native Windows fallback. Use for packaging, cross-build verification, or Windows hands-on testing paths.
---

# Brawler Packaging

Reference for building and smoke-testing Brawler release artifacts. See [engineering-workflow.md](../../../docs/engineering-workflow.md) for the WSL/Windows runtime split this sits inside, and the `brawler-release` skill for when packaging runs as part of a release.

## Windows-from-Linux portable

The primary Windows sanity path — builds the versioned portable Windows executable from the Linux/WSL Nix environment, no native Windows tooling required:

- `make package-windows-from-linux`: builds the versioned portable Windows `.exe` and copies it to a Windows test directory.
- `make package-windows-smoke-run`: launches the latest copied portable executable through `powershell.exe` for manual smoke testing (only entry point that launches it).
- `make package-windows-portable-zip`: uses the same Linux/WSL `cargo-xwin` path to produce a versioned portable zip under `release-artifacts`, containing `brawler.exe` and `README-portable-windows.txt`.
- `make package-release-artifacts`: builds the Linux artifacts and Windows portable zip together for release publication.

Implementation direction:

- Use the dedicated Nix shell `windows-cross` (`devShells.windows-cross` in `flake.nix`).
- Includes the Rust `x86_64-pc-windows-msvc` target, `cargo-xwin`, NSIS, LLVM/LLD, Clang, Node, npm, and Tauri CLI prerequisites.
- Runs the Tauri build from Linux with a Windows target and `--no-bundle`.
- Copies the resulting portable executable to `D:\Brawler\Builds\latest` with a versioned name such as `brawler-0.21.0-windows-x64-portable.exe`.
- Stops already-running copied `brawler*` processes before replacing the portable artifact.
- Windows installer generation is a later target; the current loop validates the runnable `.exe` only.
- Makefile targets that enter `nix develop` clear inherited `LD_LIBRARY_PATH` before launching Nix, so stale libraries from an outer shell don't break the Nix executable before the intended dev shell is created.

Do not routinely run Windows npm/Rust builds inside the same working tree used by WSL/Nix — mixing Windows and Linux `node_modules` and Rust `target` artifacts in one tree creates slow, confusing, noisy changes. Prefer `package-windows-from-linux`; if native Windows packaging is needed, use a separate Windows checkout/worktree.

### Data policy (portable Windows)

- Windows portable release executables store runtime data in `data/` next to the executable.
- Development builds keep using the OS app-data directory.
- Runtime secrets continue to use the OS keychain and may need re-entry when a portable folder moves to another machine/user profile.
- The portable executable relies on the system WebView2 runtime. Bundling a fixed WebView2 runtime or producing an installer is deferred.

## Linux artifacts

- `make package-linux-amd64`: produces versioned `.deb`, `.rpm`, and `.AppImage` files under `release-artifacts` (e.g. `brawler-0.28.0-linux-amd64.deb`).
- The target intentionally uses a **split packaging path**: `.deb`/`.rpm` build through the Nix-wrapped Tauri command, while AppImage builds through the host Ubuntu toolchain — `linuxdeploy` dependency discovery does not reliably resolve WebKitGTK from the Nix store.
- Linux runtime startup has a WSL-only WebKitGTK compatibility fallback for WSLg/EGL startup failures, keeping `.deb`, `.rpm`, AppImage, desktop launch, and terminal launch behavior consistent without changing native Linux defaults.

Implementation direction:

- Set `APPIMAGE_EXTRACT_AND_RUN=1` for AppImage packaging (including GitHub Actions) so downloaded linuxdeploy AppImages self-extract instead of relying only on FUSE availability on the runner.
- Install host AppImage runtime tools in GitHub Actions: `libfuse2t64`, `librsvg2-dev`, `squashfs-tools`, `desktop-file-utils`, `appstream` — the Tauri AppImage bundler downloads/executes linuxdeploy AppImages at packaging time and the GTK linuxdeploy plugin needs `librsvg-2.0.pc`.
- GitHub release packaging caches npm package data, Cargo registry/git data, `src-tauri/target`, and `.xwin-cache`, with lockfile-driven cache keys to avoid stale dependency reuse.
- Treat AppImage as the Arch-friendly artifact until native Pacman packaging is explicitly designed.
- Linux release builds store runtime data under `~/.brawler`; installed Linux packages must not write runtime data beside package-managed executable paths.

## Native Windows fallback

Requires a native Windows checkout with Node/Rust/MSVC tooling — use only when the Linux/WSL cross-build path doesn't apply.

- `powershell -ExecutionPolicy Bypass -File scripts/windows/dev.ps1`: start native Tauri dev mode from a Windows checkout.
- `powershell -ExecutionPolicy Bypass -File scripts/windows/dev.ps1 -Check`: run checks before native dev mode.
- `powershell -ExecutionPolicy Bypass -File scripts/windows/dev.ps1 -Build`: create a native Windows Tauri build.
- `powershell -ExecutionPolicy Bypass -File scripts/windows/package.ps1 -NoRun`: build and copy the packaged Windows executable.
- `make windows-package`: WSL-side fallback that calls Windows PowerShell to build and copy the native packaged Windows app from the default `D:\Brawler` checkout; requires Windows Node/Rust/MSVC tooling.
- `make windows-package-no-run`: compatibility alias for the same build-and-copy behavior.
- `make windows-test-help`: prints the Windows hands-on testing path.

`scripts/windows/package.ps1` accepts:

- `-WindowsRepo`: native Windows checkout path; defaults to `BRAWLER_WINDOWS_REPO` or `D:\Brawler`
- `-OutputDir`: copied artifact directory; defaults to `BRAWLER_WINDOWS_OUT` or `D:\Brawler\Builds\latest`
- `-NoRun`: copy the executable without launching it
- `-OpenOutput`: open the artifact directory in Explorer
- `-SkipInstall`: skip `npm ci`

When using `make windows-package` from WSL, `BRAWLER_WINDOWS_REPO` and `BRAWLER_WINDOWS_OUT` may use WSL-style `/mnt/c/...` paths — the Makefile converts them before invoking PowerShell.

## Cross-build dependency constraint (pure-Rust deps)

**Runtime engine dependencies shipped in the packaged app must be pure-Rust (no transitive native/C deps).** The `cargo-xwin` Linux→Windows path compiles C/asm sources with `clang-cl` against the xwin SDK, which fails for many native crates. Adding a dependency that transitively pulls a C/native crate — `ring`, `*-sys` bindings (`onig-sys`), `openssl-sys`, etc. — silently breaks `make package-windows-from-linux` even when the host/Nix build is green.

This bit the `v0.45.0` embedding engine: `hf-hub`→`ureq`→`rustls`→`ring` and `tokenizers`'s default `onig` both failed the cross-build and were replaced with the existing `reqwest` (native-tls/SChannel, already cross-compiles) and `tokenizers`' pure-Rust `fancy-regex` backend.

Rule: when adding a runtime dependency that will ship in the packaged app (especially under the interpretative-layer / engine boundaries), prefer pure-Rust crates and verify with `make package-windows-from-linux` before relying on it; reuse the already-cross-compiling stack (`reqwest`, `rustls`-free TLS) rather than introducing a parallel one. A host or Nix `cargo build` passing is **not** evidence the cross-build works.
