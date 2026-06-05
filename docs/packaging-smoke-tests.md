# Packaging Smoke Tests

Use this checklist for M21 portable Windows executable candidates.

## Build

From WSL:

```bash
make package-windows-from-linux
```

Expected result:

- a versioned executable is copied to `D:\Brawler\Builds\latest` by default
- the artifact name follows `brawler-<version>-windows-x64-portable.exe`
- packaging does not launch the app automatically

Launch the copied artifact:

```bash
make package-windows-smoke-run
```

## Portable Data

- Start the executable from its output folder.
- Confirm a `data` folder is created next to the executable.
- Confirm `data/brawler.sqlite3` is created after startup.
- Create or import a small company/watchlist/notebook sample.
- Close the app.
- Start the same executable again from the same folder.
- Confirm the data is still present.

## License Gate

- Start with no accepted license on a fresh machine or fresh user profile.
- Confirm normal navigation is gated.
- Enter a valid author license and confirm the app unlocks.
- Clear or replace the license through Settings when testing alternate states.
- Enter a valid friend-test license and confirm the app unlocks.
- Try an obviously invalid token and confirm the error is clear and recoverable.

## Primary Workflow

- Open Inbox, Companies, Watchlists, Notebooks, Events, Sources, and Settings.
- Add a tracked company.
- Create a watchlist and add the company to it.
- Create a notebook entry.
- Export research data and settings.
- Import the exported research data into a fresh portable data folder.
- Confirm source refresh commands still return a visible status or recoverable error.

## Known Candidate Limitations

- The executable is unsigned and may show Windows unknown-publisher or SmartScreen warnings.
- The executable relies on the system WebView2 runtime.
- The portable data folder must be writable.
- Secrets remain in the OS keychain and may need to be re-entered after moving the portable folder to another Windows user profile or machine.
