# ADR 0019: Portable Windows Executable Candidate

## Status

Accepted for M21 implementation.

## Context

M21 was narrowed from a broader packaging and release-hardening milestone to the smallest useful personal-use Windows candidate: a portable executable. The project owner wants a simple artifact that can be copied and run, without installer work, GitHub release automation, code signing, or a larger backup/restore feature in this milestone.

The app is built with Tauri. On Windows, Tauri uses WebView2 from the operating system by default rather than bundling a browser engine into the executable. A fully self-contained single-file executable with the webview runtime cooked in is therefore not the M21 target. Windows 10 and Windows 11 are the intended candidate platforms, with any WebView2 prerequisite documented if it appears during smoke testing.

The portable app should keep its data with the executable so the candidate behaves like a portable application instead of an installed desktop app.

## Decision

- M21 produces a raw versioned Windows executable artifact, named with app version and target, for example `brawler-0.21.0-windows-x64-portable.exe`.
- The primary packaging path is `make package-windows-from-linux` from WSL/Nix using the existing Windows cross-build shell.
- The native Windows PowerShell packaging path remains a fallback and copies the same versioned portable artifact shape.
- Packaging and launching are separate commands. `make package-windows-from-linux` builds and copies the artifact; `make package-windows-smoke-run` launches the copied artifact for manual smoke testing.
- The M21 executable is unsigned. Windows SmartScreen or unknown-publisher prompts are accepted known limitations for this candidate.
- The M21 executable relies on the system WebView2 runtime. Bundling a fixed WebView2 runtime or producing an installer is deferred.
- Windows release builds use a portable data directory next to the executable: `data/`.
- Development builds and future non-portable distribution paths may continue to use the OS app-data directory through the same data-directory boundary.
- Runtime secrets remain in the OS keychain. Moving the portable folder preserves portable data such as the database and logs, but secrets may need to be re-entered on another machine.
- GitHub Actions packaging, installer packaging, code signing, release automation, tags, changelog, richer About/version UI, and full backup/restore are deferred.

## Consequences

- The first personal-use candidate stays simple to build, copy, and test.
- The artifact is not a polished public release. It may trigger Windows trust warnings until code signing is added later.
- The executable may not run on Windows machines missing a suitable WebView2 runtime. If that becomes a real external-testing blocker, a later release-packaging milestone should choose between fixed WebView2 bundling and installer packaging.
- Keeping portable data next to the executable makes backup and movement more understandable for the candidate, but the folder must remain writable.
- Future installer packaging must explicitly choose whether to use OS app-data or portable data mode; the code now has a boundary for that choice.
