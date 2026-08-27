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
card or a tool has a lot of content. Opening a tool replaces the core with
the tool (the core collapses to a one-line summary strip so you never lose
the ticker/counters context) — closing it restores the core exactly as you
left it, scroll position and selection included. Every open tool carries a
leading **Overview** button (and the summary strip's ticker does the same) so
you're never more than one click from the untouched core.

| Tool | Hosts |
|---|---|
| **Claims** | management claims to verify, with evidence |
| **Notebook** | this company's notes |
| **Decision journal** | your buy/pass/keep-watching entries for this company |
| **Quality** | the quality scorecard |
| **Report diff** | report-over-report comparison |
| **Research** | the research review queue/questions/reminders |
| **Ownership** | holder structure + short positions |
| **Signals** | red flags, with acknowledge/history |
| **Documents** | the company's report documents |

**Fundamentals**, **Coverage**, and **Recommendations** are reachable the
same way from their core card's own button — see [The core](#the-core).

Opening a feed item from the Inbox ("Open company") lands you straight on
that item's detail, with the rest of the feed reachable below it.

## Unsaved work: stay or discard

If a tool has a draft in progress (an unsaved note, an open composer) and you
try to leave it — closing the tool, opening a different one, switching to
another company, navigating away, or **closing the app window** — Brawler
asks: **"Unsaved changes in this tool"**, with **Stay** (keep the draft) or
**Discard** (drop it and continue). Nothing is silently lost.

## What moved from the old dashboard

The freeform, per-company dashboard (build-your-own panel grid) is frozen —
see [Composable cockpit views](cockpit-views.md) for what that means for any
dashboards you already built. Every capability it hosted is still here, just
fixed in place instead of arranged by you:

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
