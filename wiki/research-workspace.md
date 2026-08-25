# The research workspace (modes, Today, and Focus)

Brawler is organized around the **jobs you actually do** as an investor, not a
single screen of everything. The app shell has three parts:

- a **left sidebar** — your navigation spine,
- a **top bar** — global search/ask, source health, refresh, and theme,
- the **main area** — whichever mode you're in.

The app uses its own bundled typefaces — Schibsted Grotesk for the
interface and JetBrains Mono for figures and identifiers — on a flat dark (or
light) background. Nothing is fetched from the internet for this; the fonts
ship inside the app.

## The sidebar: modes, pinned companies, library

The sidebar is grouped so you always know where you are:

- **Modes** — the big destinations:
  - **🏠 Today (Dziś)** — your home and the default screen: a morning queue broken
    into days. The header tells you what arrived **since your last visit** (reports,
    filings, media — media come pre-grouped per company); each row's button says
    exactly where it takes you ("Otwórz komunikat" opens that very filing in the
    Inbox, "Otwórz tezę" highlights that claim). An announced report that has not
    arrived shows as "NIE WPŁYNĄŁ" until the app's own delay alert takes over. Mark
    a day as reviewed and it folds to a single line; a quiet morning says so plainly.
  - **Companies** — browse, search, add companies, and open one to its
    dashboard (see below).
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

**Today** answers one question: *what should I look at?* At the top, a
**morning briefing** sums up what changed in your companies and what needs
doing; below it, a single **attention stream**, ordered by priority — not a wall
of feed items:

1. **Autopilot** — one row per automated run for a company you've opted into
   **assist** or **autopilot** mode, with a summary, a **Structure changed**
   note when the report's layout shifted, and **Review**/**Dismiss** (plus
   **Undo** for autopilot-mode runs, with the undo confirm inline in the row).
   See [Autopilot](autopilot.md).
2. **To verify** — management claims that are due or overdue for the companies
   you have **pinned**. (Pin companies to populate this.)
3. **Fresh disclosures** — the newest report publications, with **Review** to
   jump to that company.
4. **Upcoming reports** — the next report dates on the calendar.
5. **Fired alerts** — the alert rules you set up that just triggered, grouped by
   company, each with **Review** (jump to the evidence) and **Dismiss**.

The morning briefing and alert rules have their own guide:
[Attention alerts and the morning briefing](attention-and-briefing.md).

The **counter tiles** above the stream (Autopilot / To verify / Upcoming
reports) show how much of each is waiting — and clicking a tile **filters the
stream** to that category; click again to restore. The stream is fully
keyboard-friendly: **j / k** move focus between row actions. When nothing needs
you, Today says so and stays calm. **Open Inbox** takes you to the full feed.

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

## The command palette (Ctrl+K)

Press **Ctrl+K** anywhere to open the **command palette**: type to filter, hit
Enter to run. It's the fastest way to jump between screens or trigger an action
without reaching for the mouse. (Inside a cockpit view, the palette also lists
that view's panel commands.) All shortcuts are listed under
**Settings → Keyboard shortcuts** — shortcuts are ignored while you're typing
in a field or editor, so they never fight your text.

## Deleting things: undo, not "are you sure?"

Where Brawler can restore something faithfully after deletion (notes, research
questions, and similar), deleting shows a **toast with Undo** instead of a
blocking confirmation dialog — act first, change your mind within a few
seconds. Destructive actions that *can't* be fully restored keep an explicit
inline confirmation instead. Either way, focus lands somewhere sensible after a
row disappears, so keyboard flow isn't broken.

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
