# ADR 0066: Live-Drive — Remote-Debugging-Based Verification of the Real Running App

Status: Accepted

Relates to: [ADR 0021](0021-browser-ui-regression-testing.md) (mocked Playwright browser smoke), [docs/testing.md](../testing.md) (Manual desktop smoke, Packaging smoke), [docs/engineering-workflow.md](../engineering-workflow.md) (Definition of Done — "you rendered the changed screen and looked at it").

## Context

Brawler's automated Playwright suite (`playwright.config.ts`) drives a mocked Vite preview — no real Tauri commands, no real SQLite DB. Proving a change works against the **real** desktop app (real backend, real local DB, real WebView2 chrome) currently requires a human at a Windows machine following the manual smoke checklist in `docs/testing.md`. That is slow, easy to skip under time pressure, and not something an agent working from WSL can do at all — WSL has no GUI (per CLAUDE.md), so an agent cannot look at, click through, or verify the real packaged app itself.

WebView2 (the Chromium runtime Tauri uses on Windows) is a Chromium build and therefore speaks the **Chrome DevTools Protocol (CDP)**. Setting `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>` before launch opens a local CDP endpoint with zero app-code changes. Playwright's `chromium.connectOverCDP()` can then attach to that endpoint and drive the real running window like any other browser page — including reading real DOM state, clicking through real flows, and screenshotting the real app.

## Decision

**"Live-drive": an opt-in, dev-only ritual that lets an agent (or a human) on WSL drive the real running Windows app over CDP, replacing most manual smoke testing with something an agent can execute directly.**

1. **Launcher (`scripts/windows/dev-live.ps1`, run on Windows):** launches the portable packaged `.exe` (the same artifact `make package-windows-from-linux` produces) with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=<port>"` (default `9222`, overridable). Prints the check URL (`http://localhost:<port>/json/version`) and next steps. It does **not** open a firewall rule or bind any interface beyond what Chromium's remote-debugging server already binds (`127.0.0.1`/localhost) — the port is reachable only from the same machine.
2. **Connection (`tests/live/helpers/liveConnect.ts`, run from WSL):** `chromium.connectOverCDP(BRAWLER_CDP_URL ?? "http://localhost:<port>")`, attaching to WebView2's single existing browser context/page rather than creating a new one. Because WSL2 without mirrored networking cannot always reach a Windows-host `localhost` port, the helper falls back to the Windows host IP read from the `nameserver` line in `/etc/resolv.conf` (the standard WSL2-NAT route to the host) when the plain `localhost` URL is unreachable and `BRAWLER_CDP_URL` was not set explicitly.
3. **Test layer (`tests/live/`, `playwright.live.config.ts`):** a separate Playwright config and test directory from the mocked suite — no `webServer` (the app is launched independently), generous timeouts, single worker. `make live-drive` runs it. This is **never** part of `make check`/`check-epic` (no default/CI environment has a live GUI app with an open debug port), and is documented as such at its Makefile exclusion comment per the anti-rot convention ([ADR 0038](0038-enforcement-as-guardrails.md)). A full-cycle target, `make live-cycle` (= `make live-up` + `make live-drive`), makes the loop one WSL command: rebuild the portable exe, launch it on Windows via `powershell.exe` + the launcher, poll the CDP endpoint until ready (localhost → resolv.conf-nameserver fallback, 90s timeout), hand the verified URL to the suite via `/tmp/brawler-live-cdp-url`, and run it.
4. **First spec (`tests/live/smoke.live.spec.ts`):** non-destructive proof — connect, assert the sidebar/app shell rendered, screenshot to `test-results/live/`, log the app version. Future live specs extend this directory as more real-runtime verification is needed (e.g. real source refresh, real keychain-backed flows).

### Security posture

An open remote-debugging port grants **full control of the app's UI and DOM** (arbitrary JS execution, reading whatever is on screen) to anything on the machine that can reach it. This is acceptable only because: it is **opt-in** (never on by default, never in a shipped build), **localhost-only** (no firewall rule, no non-loopback bind), and scoped to **an active testing session** the operator starts and stops deliberately. It must never be enabled in a build a user runs normally, and never in CI.

## Alternatives considered

- **VNC / screen-recording + human review of screenshots:** still requires a human loop for anything beyond "does it look right"; cannot assert DOM state, and doesn't let an agent act (click, fill, navigate) — only observe.
- **A custom in-app test/automation API** (a Tauri command or debug endpoint exposing app state/actions to a test harness): rejected — it means shipping test-only surface area in the real app (or a feature-gated build divergence from what users run), maintaining a bespoke protocol, and reimplementing what CDP already gives for free. CDP needs **zero app code** — it is a property of WebView2/Chromium, not something Brawler has to build or maintain.

## Consequences

- An agent on WSL can now drive real Tauri commands, the real local DB, and real WebView2 chrome directly, closing the "no GUI in WSL" verification gap the Definition of Done already calls out.
- The manual desktop smoke checklist in `docs/testing.md` remains the fallback for native-only concerns (real file dialogs, OS keychain prompts, taskbar behavior) live-drive cannot reach, but most layout/workflow verification can move to live-drive.
- New files: `scripts/windows/dev-live.ps1`, `playwright.live.config.ts`, `tests/live/` (helpers + specs), `make live-drive`. None of these are picked up by `make check`/`check-epic`.
