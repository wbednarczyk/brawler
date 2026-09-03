# The Spółka screen

**Spółka** is the one place that shows you everything about a company at
once — no assembly required. It replaced the old freeform company dashboard
(F3a, ADR 0107): instead of a grid of panels you pick and arrange, every
company opens straight into the same fixed layout — a glance bar, a
co-visible core, and a workshop of tools one click away.

## Getting there

- Click a company row in **Companies**, a **pinned** company in the sidebar,
  or a company mentioned from Today/Inbox.
- Command palette (`Ctrl+K`) → **Open company: TICKER**.
- Or switch right from the screen: a **company picker** in the header lets you
  jump straight to any tracked company without leaving Spółka.

The screen always opens scoped to one company (`data-company-id` if you're
ever inspecting the DOM) — no blank state.

## The glance bar

Identity (ticker, name) plus four counters, laid out as a row of tiles (a
figure, a label, and a detail line each), each a one-click drill into the
matching workshop tool:

| Counter | Drills into |
|---|---|
| **Signals** | the **Signals** tool (red flags), broken down by category |
| **Claims** | the **Claims** tool, with the nearest due date shown |
| **Shorts** | the **Ownership** tool, scrolled to the short-positions section |
| **Events** | the **Events** tool (this company's upcoming dates, next 30 days) |

A counter past 99 reads "99+" — the exact figure is one click away.

## The core

Always visible at rest, no panel picking required — the screen fills the
panel height with no page scroll; a card with more content than fits scrolls
its own body instead:

- **KPI table** — the annual figures, with a **Fundamentals** button for the
  full facts matrix.
- **Feed** — the newest items (capped); **Feed** button for the full list.
- **Price chart** — 3 months of daily candles, log scale, YTD/1M deltas.
- **Report coverage** — the 8 newest periods; **Coverage** button for the
  full Coverage screen (a tracked company can carry 30+ periods).
- **Recommendations** — the latest few; **Recommendations** button for the
  full history.

Nothing here is a buy/sell signal — it's the state of your research, at a
glance.

## The workshop

A bar of tools fixed along the bottom — it never scrolls away, even when a
card or a tool has a lot of content. The bar **leads with Overview** — the
core itself, active whenever no tool is open — followed by every one of the
screen's 14 tools, in this order. Opening a tool replaces the core with the
tool (the core collapses to a one-line summary strip so you never lose the
ticker/counters context) — closing it restores the core exactly as you left
it, scroll position and selection included. The active entry (Overview or the
open tool) is visibly marked; the **Overview** tab (or the summary strip's
ticker, or the tool's ✕) brings back the untouched core in one click.

| Bar entry | Hosts |
|---|---|
| **Overview** | the core itself (KPI table, feed, price chart, coverage, recommendations) |
| **Fundamentals** | the full facts matrix — also reachable from the KPI card's own button |
| **Feed** | the full company feed — also reachable from the feed card's own button |
| **Coverage** | the full Coverage screen — also reachable from the coverage card's own button |
| **Recommendations** | the full recommendations history — also reachable from the recommendations card's own button |
| **Claims** | management claims to verify, with evidence |
| **Notebook** | this company's notes |
| **Decision journal** | your buy/pass/keep-watching entries for this company |
| **Quality** | the quality scorecard |
| **Report diff** | report-over-report comparison |
| **Research** | the research review queue/questions/reminders |
| **Ownership** | holder structure + short positions |
| **Signals** | red flags, with acknowledge/history |
| **Documents** | the company's report documents |
| **Events** | this company's upcoming dates — also reachable from the Events glance counter |

Opening a feed item from the Inbox ("Open company") lands you straight on
that item's detail, with the rest of the feed reachable below it.

## Keyboard

The whole screen works without the mouse. Every binding below is rebindable
in Settings → Keyboard shortcuts.

| Keys | Does |
|---|---|
| `Ctrl+.` | Jump to the workshop bar (focus lands on the entry that last had focus — at first, the open one). Does nothing while a dialog is open. |
| `←` `→` · `Home` `End` | Move along the bar (wraps); `Enter` or `Space` opens the focused entry. The bar is one Tab stop — `Tab` leaves it. |
| `H` / `L` | Previous / next workshop tool, cycling through Overview. |
| `Shift+J` / `Shift+K` | Next / previous company (the open tool closes first — a draft asks stay/discard). |
| `Esc` (inside a tool) | Back to Overview; focus returns to that tool's bar entry. A draft in progress asks stay/discard first. In a search box, the first `Esc` clears the text; in a drop-down it closes the list. |
| `Ctrl+K` | Command palette: type, `↑`/`↓`/`Home`/`End`, `Enter`. `Esc` closes it and puts focus back where it was. |

When a tool opens, focus lands on its heading; the cyan outline always shows
where focus is.

## Unsaved work: stay or discard

If a tool has a draft in progress (an unsaved note, an open composer) and you
try to leave it — closing the tool, opening a different one, switching to
another company, navigating away, or **closing the app window** — Brawler
asks: **"Unsaved changes in this tool"**, with **Stay** (keep the draft) or
**Discard** (drop it and continue). Nothing is silently lost.

## What moved from the old dashboard

The freeform, per-company dashboard (build-your-own panel grid) was removed.
Every capability it hosted is still here, just fixed in place instead of
arranged by you:

| Old dashboard panel | Now |
|---|---|
| Fundamentals | Core KPI card's **Fundamentals** button (100% of the old panel) |
| Coverage | Core coverage card's **Coverage** button |
| Short positions | **Ownership** tool (shorts counter drills straight to the section) |
| Red flags | **Signals** workshop tool (new home; signals counter drills here) |
| Recommendations | Core card's **Recommendations** button |
| Claims / Quality / Notebook / Journal / Report diff / Research / Documents | Same panels, now workshop tools |
| Company feed | Core feed card (capped) + **Feed** workshop button for the full list |
| Basic info (ISIN, exchange…) | Glance bar (identity) + **Ownership** tool (the rest) |
