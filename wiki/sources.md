# Sources

Every fact in Brawler traces back to a source — an official filing feed, a
market-data provider, a company directory. This screen is where you see what
those sources are, whether they're healthy, and when they last ran. It's a
diagnostics/status surface, not a feed — you don't act on it day to day.

## The screen

- **Header**: `Refresh sources` — runs every enabled source once, right now.
- **Groups**: sources are grouped by role — official reports, calendar and
  events, public media, and the company directory. Each group header shows
  its member count.
- **Each row**: the source's name, health (a colored dot), the schedule
  summary (how often it runs automatically, and when it next will), and the
  result of its last fetch. Rows with an unhealthy source sort first within
  their group.
- **Open a source** to see its full detail: last attempt/success/error
  timestamps, what triggered the last run, and `Open source page` to view
  the source's own website.
- The **company directory** sources (GPW, NewConnect) additionally offer
  `Refresh company directory` and a searchable `Companies` list with `Add`
  for any company you don't track yet.
- A source you're allowed to turn off shows a switch on its row; the
  company directory and a few required sources can't be disabled.

## Everyday moves

- **Check something looks stale** — open the source, read `Last success` /
  `Last error`.
- **Force a fetch now** — `Refresh sources` (all) or open a source and use
  its own refresh action.
- **Add a company you're missing** — open the GPW or NewConnect directory
  source, search, `Add`.
- **Turn a source off** — its switch, if the source allows it.

## Developer mode

With Developer mode on, a source refresh also shows a detail summary
(items fetched/created/matched/unmatched, detail pages stored/failed) below
the source list.
