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

The screen always opens scoped to that one company (`data-company-id` if
you're ever inspecting the DOM) — no blank state, no picker.

## The glance bar

Identity (ticker, name) plus four counters, each a one-click drill into the
matching workshop tool:

| Counter | Drills into |
|---|---|
| **Signals** | the **Signals** tool (red flags), broken down by category |
| **Claims** | the **Claims** tool, with the nearest due date shown |
| **Shorts** | the **Ownership** tool, scrolled to the short-positions section |
| **Events** | the **Events** tool (this company's upcoming dates, next 30 days) |

A counter past 99 reads "99+" — the exact figure is one click away.

## The core

Always visible at rest, no panel picking required:

- **KPI table** — the annual figures, with an **Open fundamentals** button
  for the full facts matrix.
- **Feed** — the newest items (capped); **Open feed** for the full list.
- **Price chart** — 3 months of daily candles, log scale, YTD/1M deltas.
- **Report coverage** — per-period status; **Open coverage** for the full
  Coverage screen.
- **Recommendations** — the latest few; **Open recommendations** for the
  full history.

Nothing here is a buy/sell signal — it's the state of your research, at a
glance.

## The workshop

A bar of tools along the bottom, always reachable, one click each. Opening a
tool replaces the core with the tool (the core collapses to a one-line
summary strip so you never lose the ticker/counters context) — closing it
restores the core exactly as you left it, scroll position and selection
included.

| Tool | Hosts |
|---|---|
| **Open claims** | management claims to verify, with evidence |
| **Open notebook** | this company's notes |
| **Open decision journal** | your buy/pass/keep-watching entries for this company |
| **Open quality** | the quality scorecard |
| **Open report diff** | report-over-report comparison |
| **Open research** | the research review queue/questions/reminders |
| **Open ownership** | holder structure + short positions |
| **Open signals** | red flags, with acknowledge/history |
| **Open documents** | the company's report documents |
| **Open fundamentals** | the full financial facts matrix (also reachable from the KPI card) |
| **Open coverage** | the full Coverage screen (also reachable from the coverage card) |
| **Open recommendations** | the full analyst-recommendations history (also reachable from the card) |

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
| Fundamentals | **Open fundamentals** workshop tool (100% of the old panel) |
| Coverage | **Open coverage** workshop tool / core coverage card |
| Short positions | **Ownership** tool (shorts counter drills straight to the section) |
| Red flags | **Open signals** workshop tool (new home; signals counter drills here) |
| Recommendations | Core card + **Open recommendations** workshop tool |
| Claims / Quality / Notebook / Journal / Report diff / Research / Documents | Same panels, now workshop tools |
| Company feed | Core feed card (capped) + **Open feed** workshop tool for the full list |
| Basic info (ISIN, exchange…) | Glance bar (identity) + **Ownership** tool (the rest) |
