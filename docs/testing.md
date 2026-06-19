# Testing

The single home for Brawler's testing strategy, layers, and the manual/live/packaging smoke procedures. The day-to-day build/validation discipline and the Definition of Done live in [Engineering Workflow](engineering-workflow.md); this doc is the detailed testing reference it points at.

Use [Project Brief](project-brief.md) for the full documentation map. Related: [Engineering Workflow](engineering-workflow.md), [ADR 0007: GitHub Build and Lean Testing](adr/0007-github-build-and-lean-testing.md), [ADR 0021: Browser UI Regression Testing](adr/0021-browser-ui-regression-testing.md).

## Strategy

**Aim for automated coverage of every behavior.** Every command/contract, read model, UI workflow, migration, source adapter, provider mapping, job, and fixed regression should have a test that fails when that behavior breaks. "It's hard to test" or "it's only a small thing" is not a reason to skip coverage — find the cheapest layer that exercises it. The goal is *full behavior coverage*, not partial.

The constraint is that the suite stays **lean and fast** — coverage of everything must never mean a bloated suite or a gate that takes hours. Two rules hold both at once:

1. **Test behavior and contracts, not implementation details.** One clear test per behavior; assert the observable result (the command's output, the rendered state, the stored row), not internal mechanics. **Delete tests that no longer protect behavior, and never add a redundant or brittle test "to be safe"** (especially screenshot-diff tests) — that bloat is exactly what makes a suite slow, flaky, and ignored. More tests is not the goal; covering every behavior *once, well* is.
2. **Keep the bulk in the fast layers, and keep the slow ones out of the per-change gate.** `make check` must stay in the seconds-to-low-minutes range. The slow, flaky, or credentialed layers (Playwright browser smoke, live provider smoke, packaging smoke) are **opt-in/periodic** (`make check-epic`), never in `make check` — so "test everything" never means "every push takes hours." Default CI/local checks stay deterministic and secret-free; anything needing credentials/network/external services is manual or opt-in.

**The layers (push coverage down to the cheapest layer that proves the behavior):**

- many **Rust unit/contract tests** — domain logic, command contracts, parsing, dedupe, migrations, provider mapping, jobs; the bulk of coverage, milliseconds each;
- **frontend component/workflow tests** (Vitest) for every UI state and workflow, not just critical ones;
- **test-sample-backed integration tests** for source adapters and migrations (parsing, dedupe keys, company matching, error handling, migration safety, data persistence);
- **browser UI smoke** (Playwright) for layout/scroll/overflow regressions Vitest/jsdom cannot catch — periodic;
- a few **desktop / packaging / live-provider smoke** checks for what only the real runtime, package, or provider can prove — periodic/manual.

## Per-area minimum gates

- **Ordinary change:** docs/contracts updated when behavior changes; relevant Rust + frontend tests pass; formatting/lint pass; dependency additions justified and license-reviewed when they affect runtime.
- **Source adapter:** test-sample parse/dedupe/matching/error-handling tests pass; source policy documented. Normal CI must not depend on GPW/Gemini/SEC/Nasdaq/media reachability; sample refresh is deliberate and reviewable.
- **AI provider:** provider contract mapping + prompt/result shape tested with samples, no live calls; normal CI requires no API keys; live checks manual/local-only.
- **Packaging:** app starts; Rust command boundary works; local SQLite opens; primary screen renders; packaged builds keep open-core navigation and preserve optional entitlement workflows.

## Browser UI regression smoke (Playwright)

A small Playwright browser-smoke layer catches UI/layout regressions Vitest/jsdom cannot (overflow, scroll-ownership, clipping, fixed chrome). It targets the Vite preview app in Chromium with deterministic mock data — it does **not** read live sources or the user's local database. Opt-in/periodic (not in `make check`). Policy: [ADR 0021](adr/0021-browser-ui-regression-testing.md).

Setup and run:

- `make ui-smoke-install` — first-time Chromium download into the local Playwright cache.
- `make ui-smoke` — run the suite. (`npm run test:browser:install` / `npm run test:browser` are the direct equivalents.)
- Run a subset: `npx playwright test journeys smoke-walk`.

Use it for repeated layout risks: fixed app chrome + no global scrollbar; independently scrollable panels (Companies, Watchlists, Notebooks, Inbox, Events, Sources); dense row/category sizing; and **viewport regressions across the matrix in `playwright.config.ts`** — compact (1366×768), wide (1920×1080), and the tall/narrow quarter-ultrawide at 100% (1280×1440) and 125% (1024×1152) scaling, per the UI scaling requirement in [AGENTS.md](../AGENTS.md).

How it's wired (assertion-driven, not screenshot-only):

- a shared harness (`tests/browser/helpers/harness.ts`) gives an auto console-error gate (any `console.error`/uncaught error fails the test) and reusable invariants (`expectNoPageOverflow`, a deep `expectNoHorizontalOverflow` scan, `expectInternalScroll`);
- `tests/browser/journeys.spec.ts` asserts full flows end to end; `tests/browser/smoke-walk.spec.ts` walks every primary screen asserting no page-level horizontal overflow and deep-scans the detail rail where `overflow:hidden` hides the symptom;
- mock data lives in `src/test/browserSmokeRuntime.ts`, which mocks Tauri `invoke` (its `switch` default throws — add a case when a screen calls a new command). `?locale=pl` previews the app in Polish for screenshot specs.

**The harness also renders any screen for visual review** — drive a throwaway spec to the screen and `await page.screenshot(...)`. "No GUI in WSL" is not a reason to skip looking at a UI change (see [Engineering Workflow → Definition of Done](engineering-workflow.md#definition-of-done-the-handover-gate)).

Evidence policy: DOM/layout assertions are the pass/fail signal; screenshots and traces are retained only on failure; visual snapshot comparison is deferred until a stable use case justifies it.

Do **not** use this layer for: live external source/API testing; real Tauri file dialogs/keychain/taskbar/packaging/WebView2; broad end-to-end coverage of every workflow; or screenshot comparison as pass/fail evidence — those are the live/packaging smoke and native Windows paths.

## Manual desktop smoke

When automated coverage can't realistically catch it, run the app in the normal desktop path and record pass/fail notes in the milestone review before closure. `make frontend-preview` is acceptable for browser-only layout review but does not validate Tauri commands, keychain, or native window behavior.

Representative manual sweep:

- **Settings:** open every section; subnavigation stays stable; controls readable; the active panel scrolls independently. **Database:** adjust pool values, confirm out-of-range clamps + reset-to-defaults + the "applied on next launch" note.
- **Appearance:** switch dark/light/system, then `night-neon` / `midnight-horizon`; palette changes tokens without unexpectedly changing brightness mode.
- **Notebooks:** select company/note, create + edit a long note, tag-filter and clear; company list, note list, and editor scroll independently.
- **Inbox:** scan rows, change/clear filters and search, open details; the destructive cleanup action is separated from routine controls.
- **Sources:** adapters grouped by purpose, disabled/review candidates distinct, expanded rows readable, registry search + clear work.
- **Companies:** create a watchlist, toggle membership on/off, verify feedback/selected states, clear form fields.
- **Global search:** top-toolbar search → ranked results grouped by content type with snippets → select navigates → field clear returns focus.
- **Backups/restore:** in Developer Diagnostics, verify status + list, create a backup, exercise restore (warns + applies on relaunch).
- **Polish locale:** switch to Polish and check labels in Settings, Sources, Notebooks, Companies, licensing.

## Live smoke tests

Live smoke tests validate real external providers/sources. They require credentials, network, and external availability, so they are **never** part of default local checks or CI. The API key must not be stored in the repo, Nix files, `.envrc`, logs, exported settings, or SQLite settings.

### OS keyring persistence — `make smoke-keyring`

Proves the runtime OS credential backend can persist the Gemini transcription key target. The ignored Rust test `live_keyring_persists_gemini_transcription_secret` writes a temp secret to the real OS store, reads it back, clears it, and restores any pre-existing key. **Run it on the OS whose persistence you're validating** — a WSL run validates the WSL/Linux keyring, not the packaged Windows app's Windows Credential Manager. If the packaged Windows app can't persist credentials while WSL passes, add a Windows-runtime keyring validation path before closure.

### Gemini YouTube transcription — `make smoke-gemini-transcript`

Proves the configured Gemini model transcribes a real public YouTube URL into segments (ignored test `live_gemini_transcribes_youtube_url`). Default model `gemini-2.5-flash`.

Env: `GEMINI_API_KEY`, `BRAWLER_GEMINI_SMOKE_YOUTUBE_URL`, optional `BRAWLER_GEMINI_SMOKE_MODEL`, optional `BRAWLER_GEMINI_REQUEST_TIMEOUT_SECONDS` (use `45` for fail-fast; keep the `300` app default for real conference videos). First-validation URL: `https://www.youtube.com/watch?v=9hE5-98ZeCg` (Google's own Gemini video-understanding example) — validate wiring with this short input before longer videos.

```bash
GEMINI_API_KEY=... \
BRAWLER_GEMINI_SMOKE_YOUTUBE_URL='https://www.youtube.com/watch?v=9hE5-98ZeCg' \
make smoke-gemini-transcript
```

Failure interpretation: missing credentials → set `GEMINI_API_KEY`; provider limit/unavailable → retry or a smaller model; network timeout → use the short URL first, then raise the app timeout for long videos rather than treating Gemini as broken; provider rejection → try another public URL, and if the default model rejects supported URLs, change the default to the cheapest model that passes; parse error → fix provider prompting/parsing. **M10 cannot close until this passes at least once on the milestone branch.**

### Gemini feed-item analysis — `make smoke-gemini-analysis`

Proves the configured general-analysis model returns the provider-neutral AI analysis shape for one real feed-item-sized sample (ignored test `live_gemini_analyzes_feed_item`). Default model `gemini-2.5-flash`.

Required env: `GEMINI_API_KEY`, `BRAWLER_GEMINI_ANALYSIS_SMOKE_SOURCE_URL`, `BRAWLER_GEMINI_ANALYSIS_SMOKE_TITLE`, `BRAWLER_GEMINI_ANALYSIS_SMOKE_BODY`. Optional: `BRAWLER_GEMINI_ANALYSIS_SMOKE_MODEL`, `BRAWLER_GEMINI_ANALYSIS_TIMEOUT_SECONDS` (`45` fail-fast; `90` app default), `BRAWLER_GEMINI_ANALYSIS_SMOKE_QUESTION` (custom-question path), and `..._SMOKE_{COMPANY,TYPE,SOURCE,LANGUAGE,ATTRIBUTION,SUMMARY,PROMPT_PRESET}`.

```bash
GEMINI_API_KEY=... \
BRAWLER_GEMINI_ANALYSIS_SMOKE_SOURCE_URL='https://example.com/report' \
BRAWLER_GEMINI_ANALYSIS_SMOKE_TITLE='Example company report' \
BRAWLER_GEMINI_ANALYSIS_SMOKE_BODY='Paste a short source excerpt here.' \
make smoke-gemini-analysis
```

Expect provider ID, model, `job_status=succeeded`, a non-empty summary, and ≥1 source reference. Failure interpretation mirrors the transcription smoke (set sample input explicitly — it does not read SQLite). **M13 cannot close until this passes once on the branch, or a documented provider outage/cost decision explicitly defers it.**

## Packaging smoke tests

Checklist for public release artifact candidates. Native Windows owns hands-on desktop/packaged-executable validation; a WSL Linux build does not validate Windows desktop behavior.

Build all artifacts from WSL/Linux with `make package-release-artifacts` (`.deb`/`.rpm` via Nix; AppImage via the host Ubuntu toolchain with `APPIMAGE_EXTRACT_AND_RUN=1` because the AppImage bundler must discover WebKitGTK; the GitHub workflow installs `libfuse2t64`, `librsvg2-dev`, `squashfs-tools`, `desktop-file-utils`, `appstream` and caches npm/Cargo/`src-tauri/target`/`.xwin-cache`). Subsets: `make package-linux-amd64`, `make package-windows-portable-zip`; the copy-and-run path is `make package-windows-from-linux` + `make package-windows-smoke-run`. Packaged builds enable shipped cargo features via `RELEASE_FEATURES` (see [Engineering Workflow](engineering-workflow.md)).

Expected files under `release-artifacts`: `brawler-<version>-linux-amd64.{deb,rpm,AppImage}` and `brawler-<version>-windows-x64-portable.zip`. Confirm each artifact's metadata version matches its filename.

Per-artifact smoke (each: start → window opens → open-core nav available → data dir + `brawler.sqlite3` created → create/import a small sample → close + reopen → data persists):

- **AppImage:** `chmod +x` if needed; data under `~/.brawler`.
- **.deb / .rpm:** inspect metadata (`dpkg-deb --info/--contents`, `rpm -qip/-qlp`); install on a disposable env; data under `~/.brawler`, not the install path. On WSL/WSLg, command-line startup must not abort with `Could not create default EGL display: EGL_BAD_PARAMETER` (Brawler applies a WSL-only WebKitGTK fallback; if it still fails, capture output and check whether `WEBKIT_DISABLE_DMABUF_RENDERER=1 brawler` changes behavior).
- **Windows portable zip:** extract; contains only `brawler.exe` + `README-portable-windows.txt`; a `data/` folder + `data/brawler.sqlite3` are created next to the exe.

Primary workflow check: open Inbox/Companies/Watchlists/Notebooks/Events/Sources/Research/Settings; add a company; create a watchlist + add the company; create a notebook entry; export research + settings; import into a fresh data folder; confirm source refresh returns a visible status or recoverable error.

Known candidate limitations: artifacts are unsigned (OS trust warnings possible); Windows portable relies on the system WebView2 runtime; Linux packages depend on the system WebKitGTK stack; the portable folder / `~/.brawler` must be writable; secrets stay in the OS keychain and may need re-entry after moving a portable folder to another profile/machine.
