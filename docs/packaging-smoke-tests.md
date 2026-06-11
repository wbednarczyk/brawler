# Packaging Smoke Tests

Use this checklist for public release artifact candidates.

## Build

Build all current release artifacts from WSL/Linux:

```bash
make package-release-artifacts
```

The Linux packaging path requires both the Nix environment and host Ubuntu
Tauri prerequisites. `.deb` and `.rpm` are built through Nix; AppImage is built
through the host Ubuntu toolchain because the AppImage bundler must discover
runtime WebKitGTK libraries. AppImage packaging sets `APPIMAGE_EXTRACT_AND_RUN=1`
so downloaded linuxdeploy AppImages can self-extract, and the GitHub workflow
installs `libfuse2t64`, `librsvg2-dev`, `squashfs-tools`, `desktop-file-utils`,
and `appstream` for linuxdeploy/runtime compatibility.
The GitHub release workflow also caches npm package data, Cargo registry/git
data, `src-tauri/target`, and `.xwin-cache` to keep repeated release packaging
runs practical.

Expected files under `release-artifacts`:

- `brawler-<version>-linux-amd64.deb`
- `brawler-<version>-linux-amd64.rpm`
- `brawler-<version>-linux-amd64.AppImage`
- `brawler-<version>-windows-x64-portable.zip`

Build only Linux artifacts:

```bash
make package-linux-amd64
```

Build only the Windows portable zip:

```bash
make package-windows-portable-zip
```

The older Windows copy-and-run smoke path remains available:

```bash
make package-windows-from-linux
make package-windows-smoke-run
```

## Linux AppImage

- Mark the AppImage executable if needed: `chmod +x release-artifacts/brawler-<version>-linux-amd64.AppImage`.
- Start the AppImage in a Linux desktop environment.
- Confirm the app window opens and normal open-core navigation is available.
- Confirm `~/.brawler` is created after startup.
- Confirm `~/.brawler/brawler.sqlite3` is created after startup.
- Create or import a small company/watchlist/notebook sample.
- Close and reopen the AppImage.
- Confirm the data is still present.

## Linux Deb

- Inspect package metadata before installation:

  ```bash
  dpkg-deb --info release-artifacts/brawler-<version>-linux-amd64.deb
  dpkg-deb --contents release-artifacts/brawler-<version>-linux-amd64.deb
  ```

- Confirm the package metadata `Version` matches the artifact file version.
- If testing on a disposable Debian/Ubuntu environment, install the package and start Brawler from the desktop menu or command line.
- Confirm runtime data is written under `~/.brawler`, not under a package-managed install path.
- On WSL/WSLg, command-line startup should not abort with `Could not create default EGL display: EGL_BAD_PARAMETER`. Brawler applies a WSL-only WebKitGTK compatibility fallback before startup; if startup still fails, capture the terminal output and confirm whether `WEBKIT_DISABLE_DMABUF_RENDERER=1 brawler` changes the behavior.

## Linux Rpm

- Inspect package metadata before installation:

  ```bash
  rpm -qip release-artifacts/brawler-<version>-linux-amd64.rpm
  rpm -qlp release-artifacts/brawler-<version>-linux-amd64.rpm
  ```

- Confirm the package metadata version matches the artifact file version.
- If testing on a disposable RPM-based environment, install the package and start Brawler from the desktop menu or command line.
- Confirm runtime data is written under `~/.brawler`, not under a package-managed install path.

## Windows Portable Zip

- Extract `brawler-<version>-windows-x64-portable.zip` into a writable folder.
- Confirm the zip contains only:
  - `brawler.exe`
  - `README-portable-windows.txt`
- Start `brawler.exe` from the extracted folder.
- Confirm a `data` folder is created next to `brawler.exe`.
- Confirm `data/brawler.sqlite3` is created after startup.
- Create or import a small company/watchlist/notebook sample.
- Close the app.
- Start the same executable again from the same folder.
- Confirm the data is still present.

## Primary Workflow

- Open Inbox, Companies, Watchlists, Notebooks, Events, Sources, Research, and Settings.
- Add a tracked company.
- Create a watchlist and add the company to it.
- Create a notebook entry.
- Export research data and settings.
- Import the exported research data into a fresh data folder.
- Confirm source refresh commands still return a visible status or recoverable error.

## Known Candidate Limitations

- Artifacts are unsigned and may show operating-system trust warnings.
- Windows portable builds rely on the system WebView2 runtime.
- Linux packages depend on the system WebKitGTK stack expected by Tauri.
- The Windows portable folder and Linux `~/.brawler` directory must be writable.
- Secrets remain in the OS keychain and may need to be re-entered after moving a portable folder to another Windows user profile or machine.
