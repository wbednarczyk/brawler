# ADR 0021: Browser UI Regression Testing

Status: accepted.

## Context

Brawler has repeated UI regressions that are difficult to catch with Vitest/jsdom alone: broken scroll ownership, accidental global app scrolling, too-small list panels, oversized Sources bars, Notebooks pane layout problems, and viewport-specific row sizing issues.

Vitest/jsdom remains valuable for fast component and workflow tests, but it does not perform real browser layout. Manual smoke testing catches many issues, but it depends on the reviewer noticing and rerunning the right screens after every layout change.

## Decision

Add a small Playwright-based browser UI regression smoke layer in M23.

The first implementation will:

- target the Vite preview app in Chromium
- run through an opt-in command, not default `make check` or default CI
- use deterministic frontend test data rather than live sources or the user's local database
- use DOM/layout assertions as the pass/fail signal
- retain screenshots and traces only on failure
- cover two desktop viewports: compact desktop and normal desktop
- keep tests under `tests/browser/`
- focus on known repeated regressions: fixed app chrome, no global app scrollbar, internal panel scrolling, Companies list height, Notebooks pane usability, Sources row/category sizing, Watchlists member scrolling, and a tiny navigation smoke

The first implementation will not:

- automate the real Tauri desktop runtime
- validate native Windows file dialogs, keychain, taskbar indicators, packaged executable behavior, or WebView2-specific behavior
- use visual screenshot comparisons as pass/fail evidence
- replace existing Vitest/jsdom component and workflow tests
- perform live source/API testing

## Consequences

The project gains a practical guardrail for layout regressions that have already consumed iteration time. The suite is intentionally narrow and opt-in so it does not slow the default development loop or add CI/browser-install complexity before it proves stable.

Native Windows desktop behavior remains a separate smoke-testing path. If the Playwright suite becomes stable and high-value, a later milestone may promote a small subset to default local checks or CI.
