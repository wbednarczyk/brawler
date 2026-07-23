# Attention: the Today stream, alerts, and the morning briefing

Brawler tells you **what deserves a look** instead of leaving you to re-scan
everything. Since v0.60 the **Today screen is a single prioritized attention
stream**: everything that happened lands in one list, ordered by how much it
matters — with repeats folded together so ten similar events read as one line,
not ten. Everything here is **decision support only** — facts with links to
evidence, never buy/sell/hold.

## How the stream decides what leads

Every item carries one of three **importance levels**, assigned by the app
(you never configure this):

- **PILNE (urgent)** — leads the stream with a red edge: insider transactions,
  profit warnings, auditor concerns, a report your primary source missed.
- **UWAGA (notable)** — below urgent, amber edge: failed autopilot runs, fired
  price alerts, dividends/meetings you asked to be alerted about, and claims
  **overdue** for verification.
- **Routine** — dimmed, at the bottom: successful report processing, upcoming
  report dates.

**Urgency ages.** An urgent item you haven't acted on for **3 days** stops
shouting — it demotes to notable. Nothing is hidden or deleted; it just no
longer outranks today's news.

**Repeats fold together.** Events of the same kind collapse: several for one
company become one row with an **×N** count, and the same cause across many
companies becomes one row with **×N spółek** (urgent folds from 2 companies,
others from 4). Expand the row in place to see every member — each keeps its
own **Przejrzyj** (Review). Attention groups also carry **Odrzuć wszystkie**
(Dismiss all, with a confirm step) so a systemic burst clears in one action.

## Archiwum

W nagłówku Dziś przełącznik **Aktywne | Archiwum** otwiera drugi, **tylko do
odczytu** widok: **Archiwum** odrzuconych zdarzeń uwagi. Odrzucenie (**Odrzuć**)
jest **potwierdzeniem, nie usunięciem** — nic nie znika. Archiwum pokazuje te
zdarzenia od najnowszego, z tym samym układem wiersza (waga, spółka, tytuł,
znacznik reguły alertu), ale bez akcji odrzucania; **Przejrzyj** służy tylko do
przejścia do dowodu. Puste archiwum mówi wprost: „Archiwum jest puste." Na razie
(v0.60) archiwum obejmuje **tylko zdarzenia uwagi** (nie przebiegi autopilota);
przywracania (cofnięcia odrzucenia) celowo jeszcze nie ma.

## Counters and filters

The right-hand tiles — **Pilne / Autopilot / Do weryfikacji / Nadchodzące
raporty** — show live counts; click one to filter the stream to that category,
click again to clear.

## App-level conditions get one banner

When something is wrong with the app itself — e.g. a source hasn't responded
for days — Today shows **one dismissible banner** above the stream (with a
Diagnostyka shortcut), instead of repeating the condition on every row.

## The morning briefing

The **Poranny przegląd** strip sits above the stream: one line with a
timestamp and grouped counts of what changed since the last briefing (new
signals, autopilot runs, claims due, upcoming reports, fired alerts). Expand it
for the full list — every entry click-throughs to its evidence. It is
**deterministic** — composed from your data, no AI, no keys. One composes
automatically each day while the app is open; **Wygeneruj** recomposes it on
demand.

## Alert rules

An **alert rule** says *what you want to be told about*. Set them up in
**Alerts** (left sidebar, Library group): a **trigger** (a signal category, an
autopilot run completing, a price entering your range or hitting a 52-week
low), a **scope** (company or watchlist), and an on/off switch. Preset chips
cover the common cases. A rule never fires twice for the same thing and alerts
at most once a day.

## Pop-ups (toasts) are pointers, not the inbox

The stream is the system of record. A corner pop-up only announces that
something **urgent** just landed — click it to jump to the row, or dismiss it;
nothing is lost either way (dismissing the row also clears its pop-up and vice
versa). Notable events show a brief fading pop-up at most; routine events never
pop up. The stack is capped and never covers buttons you need.

## A note on scope

Attention routing is **in-app only** for now: alerts live inside Brawler, not
as operating-system notifications. Everything stays **local**, and every alert
is a **fact with a link** — never advice.
