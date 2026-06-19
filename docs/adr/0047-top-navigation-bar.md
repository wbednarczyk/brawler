# ADR 0047: Top Navigation Bar (move primary navigation from the left sidebar to the top)

Status: Accepted

## Context

Through v0.45.0 the app shell was a three-column grid: a resizable left
`aside.sidebar` (brand + the ten primary section buttons + version), an 8px
sidebar resizer, and a `main.workspace` whose top toolbar held search, source
status, refresh, and the theme switch. The left rail consumed 160–280px of every
screen's horizontal space — costly on the tall, narrow side-zone windows the app
must support (~960–1280px effective width; see `AGENTS.md` Testing
Expectations), where horizontal room is the scarce dimension.

The owner asked to move the primary menu out of the left rail and onto the top of
the window, reclaiming the full width for the workspace. Prototyping settled the
shape: keep the existing top toolbar as row 1 (brand + search + actions) and add
the primary navigation as a horizontal row 2 directly beneath it.

## Decisions

### 1. Navigation region: top bar, not a left sidebar

The app shell is a stacked three-row grid (`grid-template-rows: auto auto
minmax(0,1fr)`):

1. **Top toolbar** — brand (logo + "Brawler / Investor newsfeed" + the version
   label beside the title), global search, source-status pill, manual refresh,
   theme switch.
2. **Navigation bar** — the primary section buttons (icons + labels, active item
   badged).
3. **Workspace** — the current screen, full-width.

The left `aside.sidebar` and its drag-to-resize handle are removed entirely.

### 2. Overflow: wrap, never scroll or hide

The navigation bar is a horizontal flex row with `flex-wrap: wrap`. When the ten
sections do not fit on one line (narrow windows, ~1100px and below), items flow
onto a second line. We explicitly rejected a horizontal-scroll strip (hides items
off-edge, poor desktop discoverability) and an overflow "More ▾" menu (adds a
click and hidden state) — every section stays visible at every supported width.

### 3. Inbox workspace defaults to a 50/50 split, kept **horizontal** (side by side)

With the rail gone the Inbox feed list and detail pane share the full width
**50/50 by default, side by side**, replacing the previous fixed 360px detail rail. The divider
is a **fraction of the row** (`--detail-pane-width` is a percentage), draggable
between **25% and 75%** (`detailPaneMinFraction`/`detailPaneMaxFraction` in
`src/app/layout.ts`). The feed column is `minmax(0, 1fr)` and the detail column is
the percentage var; both panes already contain `min-width: 0` / ellipsis content,
so no fixed minimum is needed and there is no horizontal overflow. The earlier
~1120px track-shrink override (which existed only because the fixed-min two-column
grid could not fit beside the sidebar) is therefore removed; ≤980px still
collapses to a single column.

Detail-pane size stays in-memory (resets on launch), unchanged from before — no
persistence or contract change.

## Rejected alternative: stacking the Inbox panes vertically

We prototyped stacking the feed list over the detail pane (top/bottom, 50/50) and
**rejected it**. The Inbox feed pane carries ~260px of fixed chrome — the
All/Unread/Saved tabs, the stats row, "Clear filters", the six-control filter
toolbar, and the "Delete unsaved" footer — above its scrollable list. In a
half-height top pane that chrome crowds the list down to ~1–2 visible rows even on
a 900px-tall window, which defeats an inbox. Adding per-pane minimum heights only
pushed the problem into a scrolling workspace and let the full-width horizontal
resizer overlap the feed rows (clicks intercepted). **Rule: the Inbox feed list
must remain the dominant flexible scroll region; its pane's heavy fixed chrome
makes a side-by-side (horizontal) split the right layout. Do not stack it
vertically without first removing/collapsing that chrome.** The Playwright
pane-usability tests (`tests/browser/ui-layout.spec.ts`: "keeps notebook panes
independently usable", "keeps Inbox readable at a narrow desktop package window")
and the shortest viewport in the matrix (1366×768) are the automated backstop that
caught this — they must keep passing for any future workspace-layout change.

## Consequences

- Screens gain ~160–280px of width on every view; the side-zone window case
  improves materially.
- `--sidebar-width` and the sidebar resizer (state, handlers, ARIA, the one
  tolerated inline style) are deleted; the `aria-label="Primary navigation"`
  moves from the `aside` to the new `nav.navbar`, so existing selectors and tests
  keyed on that label keep working.
- Guardrails updated/added in `src/styles/layoutContracts.test.ts`: the app-chrome
  contract now asserts `.navbar` (`display: flex`, `flex-wrap: wrap`) instead of
  `.sidebar`, the `.content-grid` template asserts the 50/50 fractional columns,
  the removed ~1120px override is asserted gone, and a new guard locks the navbar
  wrap + the fractional default + the absence of an app-shell column track. This
  follows the [ADR 0038](0038-enforcement-as-guardrails.md) /
  [ADR 0045](0045-guardrail-harvest-loop.md) posture.
- Canonical docs updated in the same change: [ui-flows.md](../ui-flows.md) and
  [ui-information-architecture.md](../ui-information-architecture.md).
- UI authoring policy ([ADR 0037](0037-ui-component-framework-and-authoring-contract.md))
  is unaffected; the navigation bar remains app-shell chrome in `AppShell.tsx`.
