# Activity

Brawler works in the background all day — refreshing sources, fetching a
company's report history, reading a new report into numbers, reading
shareholder tables, pulling prices, fetching a transcript. **Activity** is the
one place that answers "what is the app doing right now, and what just
finished?" — and takes you to the result.

## Where it is

The **Activity icon** sits in the top bar, next to the Sources pill. It only
ever tells you about work in progress:

- a spinner with a number — that many tasks are running right now;
- a number without a spinner — tasks are waiting in the queue (nothing runs
  this second);
- a quiet icon — nothing in the background; hover for when the last task
  finished.

It never counts failures. A failed task is announced once, where it belongs
(the Today stream, a source's row on Sources); Activity shows its *state*.

Open it by clicking the icon or with `Ctrl+K → Open activity`.

## The panel

- **In progress** — every running, queued or stalled task, grouped per
  company (company-independent work sits under *Sources and system*). A
  history sweep shows its progress (`7/12`); a queued task shows when its next
  attempt is due.
- **Recent** — the last 7 days, newest first (up to 40 tasks). Each row says
  what the task was (*Report reading*, *Source refresh*, *History fetch*, …),
  on what (the document's title, the company's ticker, the source's name),
  how it ended (finished · failed · partial · interrupted) and when.
- **Expand a row** for the raw error, the attempt count, or a parent task's
  members.
- **One action per row** — `Open document`, `Open company`, `Open sources`,
  `Open Today`, `Open transcripts` — lands on the item itself (a failed report
  reading opens that document in the company's Documents tool) and closes
  the panel. `Escape` closes it and puts focus back where you were.

Honest states: a task in retry backoff reads *queued*, not running; a task
the app lost track of after a crash reads *stalled* or *interrupted* once the
app restarts — never a perpetual "in progress".

## What it is not

No cancel/retry here — retry lives where the result lives (a period's
`Try again` in Coverage, `Refresh sources` in the top bar). Developer-mode
Diagnostics remains the technical log; Activity is the product view.
