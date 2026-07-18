# Red flags: concerns surfaced before you read the report

The **Red flags** panel is a per-company watch-list of *things worth a second
look* — collected automatically, each stated as a fact with a link to its
evidence. It never says "sell" or "avoid"; it points at what changed and lets
you judge. Add it to any cockpit view from the panel picker; it's part of the
default company dashboard.

## The five flag types

| Flag | Raised when | Severity |
|------|-------------|----------|
| **Auditor red flag** | The auditor's opinion is qualified or raises going-concern doubt | High |
| **Report delay** | A periodic report's expected date has passed (plus a short grace) with no filing ingested | High |
| **Fund exit** | A disclosed holder has vanished from the newest ownership picture | Medium |
| **Score deterioration** | Piotroski F dropped by ≥ 2 versus the prior year, or the Altman Z″ band was downgraded | Medium |
| **Short-selling spike** | The KNF short position jumped sharply (a large 30-day increase) | Medium |

Each row shows the flag, its **severity chip** (High / Medium, fixed per type),
when it was raised, and a link to the underlying evidence — the filing, the
ownership change, the auditor signal, or the short-position record.

## Acknowledging a flag

When you've looked at a flag, **Acknowledge** it (an inline confirm on the row).
It moves to a collapsed **acknowledged** history and, crucially, **never
re-raises for the same evidence** — so a concern you've already dealt with stays
quiet instead of nagging every refresh. The history stays available if you want
to revisit what you cleared.

A company with nothing outstanding shows a calm **"no active flags"** state —
that's the healthy default, not a blank panel.

## Flags feed your alerts

Newly raised flags don't just sit in the panel: each writes a typed signal, so
your existing [alert rules](attention-and-briefing.md) and the morning briefing
pick them up like any other signal. Point an alert rule at the relevant signal
category and a fresh red flag reaches you as a notification, not only when you
happen to open the company.
