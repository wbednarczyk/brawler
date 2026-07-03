# The research workspace (modes, Today, and Focus)

Brawler is organized around the **jobs you actually do** as an investor, not a
single screen of everything. The app shell has three parts:

- a **left sidebar** — your navigation spine,
- a **top bar** — global search/ask, source health, refresh, and theme,
- the **main area** — whichever mode you're in.

## The sidebar: modes, pinned companies, library

The sidebar is grouped so you always know where you are:

- **Modes** — the big destinations:
  - **🏠 Today** — your home and the default screen (see below).
  - **Companies** — browse, search, add companies, and open one to its
    dashboard (see below).
  - **Compare** — line companies up side by side *(coming with the valuation
    work)*.
  - Below the built-in modes: your own **saved views** and a **"+ New view"**
    entry to build one — see
    [Composable cockpit views](cockpit-views.md).
- **Pinned companies** — your favorites for one-click access. Pin a company from
  its dashboard or the Companies list; it then appears here with a small
  status dot. Unpin from the sidebar (hover the row) or the dashboard.
- **Library** — the reference surfaces: **Inbox** (the full feed), **Watchlists**,
  **Transcripts**, **Sources**.
- **Utilities** — **Settings** (and **Diagnostics** in developer mode).

## Today — what needs your attention

**Today** answers one question: *what should I look at?* It is an attention
digest, not a wall of feed items:

- **What changed** — the freshest report disclosures, newest first. Click
  **Review** to jump to that company.
- **Autopilot** — a card per automated run for a company you've opted into
  **assist** or **autopilot** mode, with a summary, a **Structure changed**
  note when the report's layout shifted, and **Review**/**Dismiss** (plus
  **Undo** for autopilot-mode runs). See [Autopilot](autopilot.md).
- **To verify** — management claims that are due or overdue for the companies you
  have **pinned**. (Pin companies to populate this.)
- **Upcoming reports** — the next report dates on the calendar.
- **Conviction** — a watchlist-level overview. A per-company conviction status is
  coming with the valuation and thesis work; today this is a placeholder.
- **Recent activity** — a compact peek at the latest feed items, with a shortcut
  to the full **Inbox**.

## Opening a company: the curated dashboard

Open a company (from Companies, a pinned row, a feed item, or a Today
**Review** button) to land its **dashboard** — a curated
[cockpit view](cockpit-views.md) scoped to that company, opening with a calm
starting set of panels (Fundamentals, Feed, Claims, Quality, Report
documents, Notebook). It's the one place that shows you everything about a
company at once, and it stays composable — add, remove, or move panels, then
**Save dashboard** to keep the arrangement for next time.

Pin the company to the sidebar from here or from the Companies list. Company
metadata (exchange, ticker, ISIN, and other identifiers) lives in the
Companies list, not on the dashboard.

## Focus mode — distraction-free reading and writing

Some work wants the whole screen. **Focus** is a full-screen, distraction-free
surface you open from where the content lives, and leave with **Esc**:

- **Reader** — open a long report-over-report **comparison** full-screen to read
  it without the surrounding panels.
- **Writer** — open a **notebook note** full-screen to write at length.

## A note on conviction and advice

The conviction status is **decision support** — a transparent roll-up of facts
(claims, what changed, and later valuation/quality). It is never a buy/sell/hold
rating. Your data stays local on your machine.
