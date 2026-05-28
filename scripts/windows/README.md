# Windows Desktop Sanity Testing

Use this directory for native Windows helpers that validate the actual desktop app experience.

The recommended path is a separate Windows checkout or Git worktree. Avoid running Windows npm/Rust builds in the same working tree used by WSL/Nix because `node_modules` and Rust `target` artifacts are platform-specific and can become noisy or stale.

## Dev Mode

From PowerShell in a native Windows checkout:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/windows/dev.ps1
```

Optional flags:

- `-Check`: run the project check suite before starting dev mode
- `-Build`: build a native Windows Tauri package instead of starting dev mode
- `-SkipInstall`: skip `npm ci`
- `-AllowWslPath`: allow running from a `\\wsl$` or `\\wsl.localhost` path for a deliberate one-off experiment

## Packaged App Sanity Test

The preferred experimental direction is to build the Windows executable from Linux/WSL:

```bash
make package-windows-from-linux
```

That target builds the portable Windows executable from the Linux/Nix toolchain, copies it to the output directory, and launches the copied executable by default. Installer generation is intentionally separate and deferred.

The scripts in this directory are the fallback native-Windows path. They require Windows Node/Rust/MSVC tooling.

Default paths:

- Windows checkout: `D:\Brawler`
- Copied executable output: `D:\Brawler\Builds\latest`

Optional environment variables from WSL:

- `BRAWLER_WINDOWS_REPO`: native Windows checkout path, for example `/mnt/d/Brawler`
- `BRAWLER_WINDOWS_OUT`: artifact output directory, for example `/mnt/d/Brawler/Builds/latest`

The Makefile converts those WSL-style paths before passing them to PowerShell.

Useful targets:

```bash
make package-windows-from-linux
make windows-package
make windows-package-no-run
```

Direct PowerShell usage:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/windows/package.ps1
powershell -ExecutionPolicy Bypass -File scripts/windows/package.ps1 -NoRun
powershell -ExecutionPolicy Bypass -File scripts/windows/package.ps1 -OpenOutput
```

## WSL Role

WSL remains the main automated development environment:

```bash
make check
make build
```

Those commands provide fast automated confidence. Native Windows dev mode provides the clickable app sanity check.
