# Brawler

Brawler is a local-first investor research workspace for company news, watchlists, notebooks, and source-backed review workflows.

## Portable Windows Candidate

M21 produces a portable Windows executable candidate.

Build from WSL:

```bash
make package-windows-from-linux
```

Run the copied candidate:

```bash
make package-windows-smoke-run
```

Default output:

```text
D:\Brawler\Builds\latest\brawler-<version>-windows-x64-portable.exe
```

Portable runtime data is stored in a `data` folder next to the executable.

Notes:

- Windows 10/11 are the target platforms for this candidate.
- The executable is unsigned and may show Windows trust prompts.
- The executable relies on the system WebView2 runtime.
- Secrets stay in the OS keychain and may need to be entered again after moving the portable folder to another machine.

See [Packaging Smoke Tests](docs/packaging-smoke-tests.md) for the manual validation checklist.
