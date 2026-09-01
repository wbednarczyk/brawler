# Events

Dates that matter for the companies on your lists: report publications,
ex-dividend days, general meetings. A weekend ritual (or a mid-week glance):
"what happens this week?"

## The screen

- **Week** (default) — five working-day columns (Mon–Fri), plus a weekend
  column when it has events. A weekday card shows the ticker, a human type
  label ("Report for H1", "Ex-dividend day", "Extraordinary shareholder
  meeting"), the company name, and where the date came from (`GPW`,
  `Bankier · calendar`, or `Manual`). A day with nothing shows a quiet dashed
  column — no filler text.
- **List** — a flat, filterable list (Upcoming / Past / All) instead of the
  calendar grid; the default on narrow windows.
- Click a card to open its detail underneath: the full title, the date/time,
  the source, and — for a date **derived from a filing and not yet
  confirmed** — a "Confirm" / "Reject" choice.
- Filters: watchlist, company, type, status. `Clear filters` resets them.

## Everyday moves

- **Add event** — a manual date the sources missed (a meeting invite, a
  verbal guidance date). Company, type, date, optional time, title, `Save`.
- **Refresh calendar** — re-pulls the GPW and Bankier calendars for the
  displayed week.
- **Confirm / Reject** a proposed date — a date extracted from a filing
  (dividend, general meeting) starts as *proposed*: amber, marked "awaiting
  confirmation". `Confirm` adds it to the calendar for good; `Reject`
  discards it. Only proposals get this choice — everything else on the
  calendar is already certain.
- **Previous week / Next week / Current week** — move around; filters stay.

## When the week is empty

An empty week never dead-ends. It tells you when the next matching date is
and gets you there in one click:

- A later date exists (with your filters still applied) → "Nothing this
  week" + "Next date: {date} — {company}, {type}" → **Show next week with
  events** jumps straight there, filters kept.
- No later date matches your filters → "Later there are no events matching
  the filters" + `Clear filters`.
- No later date at all, no filters → an invitation to refresh the calendar
  (if it has never been refreshed) or add a manual event.

## What it isn't

No month view, no macro/holiday layers, no whole-market view — the "investor
week calendar" concept in [ADR 0058](../docs/adr/0058-investor-week-calendar.md)
describes those; this screen doesn't implement them.
