# Attention: the Today day queue, alerts, and fired events

Brawler tells you **what deserves a look** instead of leaving you to re-scan
everything. Since F2 (Dziś v2, `#422`) the **Today screen is a per-day
decision queue anchored to your last visit**: a delta header answers "what
arrived since I was last here", and the rows below are grouped DZIŚ (today) /
WCZORAJ (yesterday) / earlier days, newest first. Everything here is
**decision support only** — facts with links to evidence, never buy/sell/hold.

## The delta header leads

The header states, in one sentence, what arrived since your last visit
(reports/filings, with a media count noted separately) and carries the
screen's **one** filled call-to-action — the single most urgent thing in the
queue: an unseen urgent alert first, then an unread report, then a missed
report (`NIE WPŁYNĄŁ`), then whatever is newest and still unseen. A clean
morning shows no CTA at all — that absence is deliberate, not a loading state.

## The day queue

Below the header, each day with anything in it gets its own section: a mono
label (DZIŚ / WCZORAJ / the date), a count, and its rows. Rows are typed —
report/filing (official, cyan provenance), media **clustered per company**
(magenta, even a single article), `NIE WPŁYNĄŁ` (an announced periodic report
past its date with no witnessing filing — disappears the moment the
`report_delay` red flag takes over), claims to verify (a separate "DO
WERYFIKACJI" section), autopilot runs, and fired alerts (attention events,
root-fed — shared with the sidebar badge/Alerts). Every row action **names
its destination and lands on the thing itself**: `Przeczytaj raport` → the
company workspace; `Otwórz komunikat`/`Otwórz artykuł` → the Inbox with
exactly that item selected (a narrow pane raises the detail as an overlay);
`Otwórz w Inbox` (a real media cluster) → the company-scoped Inbox; `Otwórz
tezę` → that claim highlighted in Claims; `Odśwież źródła` → a source refresh.

**A day collapses to one line** once every row in it is read/seen, or you
mark it reviewed by hand (**Oznacz dzień jako przejrzany** — undoable:
re-opening it with **Otwórz dzień** clears the manual mark too). A clean
morning (nothing new, nothing pending) renders three beats instead: the
headline, a reassurance line, and a quiet **Odśwież źródła**.

## Archiwum

A quiet **Archiwum** link in the footer opens the read-only list of dismissed
attention events, fetched on first open. Dismissing (**Odrzuć**, from an
active row) is a **confirmation, not a deletion** — nothing disappears.
Restoring (undoing a dismissal) is intentionally still not offered.

## App-level conditions get one banner

When something is wrong with the app itself — e.g. a source hasn't responded
for days — Today shows **one dismissible banner** above the queue (with a
Diagnostyka shortcut), instead of repeating the condition on every row.

## The morning briefing lives in MCP now

Dziś's delta header replaced the old **Poranny przegląd** strip (ADR 0068
amendment, F2). The deterministic composition (`gather_sources` +
`compose_briefing`) and its commands are unchanged — an AI agent connected
over MCP is the briefing's consumer now, not the Today screen.

## Alert rules

An **alert rule** says *what you want to be told about*. Set them up in
**Alerts** (left sidebar, Library group): a **trigger** (a signal category, an
autopilot run completing, a price entering your range or hitting a 52-week
low), a **scope** (company or watchlist), and **Pause**/**Resume** in place of
an on/off switch. Preset chips cover the common cases. A rule never fires
twice for the same thing and alerts at most once a day. **Fired alerts come
first** on the screen — they're why you open it day to day; your rules and the
composer that creates a new one follow. A fired row's **Open …** action names
its destination (the company, or Inbox for a workspace-wide event) and marks
it seen; **Dismiss** moves it to the Archive without opening it.

## When Brawler's own work fails

Brawler runs background tasks for you — fetching price history, sweeping report
history, extracting a shareholder table, composing the briefing. If one of them
**gives up** (it retried and still could not finish), it now appears in the
stream as **UWAGA** with the **Zadanie w tle** badge, naming the task and what
it choked on ("Uzupełnianie historii cen: niepowodzenie — HTTP 503 …"). Some
tasks report themselves somewhere better and stay out of the stream: a source
that fails shows on **Źródła** with its own error, and a failed autopilot run
shows on its run card — so one failure is never announced twice. A task that is
still retrying stays quiet until it truly gives up. Tasks that concern one
company appear under its ticker; workspace-wide ones (like the briefing) appear
without a ticker.

## No pop-ups — the badge on Dziś is your signal

The stream is the system of record, and nothing interrupts you to say so:
**attention events never appear as corner pop-ups**. While you work elsewhere,
the sidebar **Dziś** entry carries a small count of important events you have
not looked at yet; open Dziś and the count clears — everything you saw there
counts as noticed (rows stay until you dismiss them, and dismissed events keep
living in the Archive). The only pop-ups left are brief confirmations of your
own actions ("Usunięto — Cofnij", "Źródła odświeżone") that fade on their own
and never stack up over the interface.

## A note on scope

Attention routing is **in-app only** for now: alerts live inside Brawler, not
as operating-system notifications. Everything stays **local**, and every alert
is a **fact with a link** — never advice.
