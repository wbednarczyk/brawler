# Ownership: who holds the company, and how that changes

Track a company and Brawler reads the **shareholders tables out of the periodic
reports it already stores** — no manual entry, no cloud. The Basic info panel
grows an **Ownership** section: a donut of the current structure grouped by
holder type, each holder's stake over time, and a derived free float. All
local, all decision support only.

## What you see

- **Current structure donut** — holders grouped by type (founder/insider,
  pension funds, mutual funds, State Treasury, treasury shares…), with the
  **free float** as a hatched, neutral slice. Free float is *derived*
  (100% − disclosed stakes) and always carries the `*` note: stakes below the
  5% disclosure threshold hide inside it.
- **Capital % and votes % separately** — preferred-vote shares are common on
  the GPW, and the gap between capital and votes is itself a signal (e.g. a
  founder foundation holding 31% of capital but 61% of votes).
- **Stakes over time** — one trend line per major holder, with ESPI
  threshold-notification events marked.
- **Holder type chips** — classified automatically from a built-in dictionary
  of Polish TFI/OFE/state entities. Unknown holders go to AI **only as a
  proposal**: you confirm or reject; nothing is applied silently. You can
  re-type any holder manually, and your choice always wins.

## Where the data comes from

1. **BiznesRadar "Akcjonariat" pages** — the automatic everyday source: every
   tracked GPW company gets its shareholder table read on the daily refresh
   (clearly labeled as BiznesRadar data). This is what guarantees coverage for
   your whole watchlist.
2. **Periodic reports** (already stored): the mandatory "shareholders holding
   ≥5% of votes" table, parsed deterministically — adds document provenance,
   the capital-vs-votes gap, and history where reports exist. Reports whose
   table is an image or an unreadable font surface as residuals — those
   results always wait for your confirmation.
3. **ESPI threshold notifications** (art. 69): formulaic filings update stakes
   between full pictures — only when the numbers parse unambiguously; anything
   ambiguous is flagged in Diagnostics instead of guessed.

The sources also check each other: whenever your reports/ESPI picture and the
BiznesRadar table disagree above the disclosure threshold, the divergence is
flagged in Diagnostics.

## Current state vs history

A holder who drops below 5% simply vanishes from disclosures (nobody files
"0%"), so the *current* view is scoped to the newest full picture — the latest
report **or** BiznesRadar table, whichever is fresher — plus later
notifications. The full history — including vanished holders — stays on the
timeline. History is append-only: re-reading a report never rewrites what was
recorded.

## Insiders: who inside the company is buying or selling

Below the ownership structure sits an **Insiders** block, built from two
official, deterministic sources — no AI, no guessing:

- **MAR art. 19 notifications** (managers' and closely-associated persons'
  transactions), parsed from the filings Brawler already stores into *who*
  bought or sold, in what *role*, and — where the notification document spells
  it out — *how much*, at what *price*, and *when*.
- The mandatory **management-holdings table** from periodic reports: each
  board/supervisory person's shareholding, parsed the same way.

What you see:

- **A transaction timeline** (newest first). Figures the notification's cover
  note omits stay blank rather than invented — Brawler fetches the notification
  **attachment PDF** to fill in volume, price, and date where that document
  parses cleanly.
- **Rolling net direction** over the **last 90 days** and **last 12 months** —
  buys minus sells, with a volume readout where volumes are known. An aggregate
  only appears once there are **at least 2 transactions** in the window; below
  that the timeline still lists them but shows no net (too little to summarise).
  Transactions with no direction count as *undetermined*, never in the net.
- **A "skin in the game" badge** on holders in the structure above who are
  corroborated as insiders — a management-holdings row or a MAR transaction ties
  the stake to a named person. It also fires when a founder holds **through a
  vehicle** (a foundation or holdco); hover the badge to see who, and via what.

Counts, volumes, and names only — never a verdict.

## Tables Brawler can't parse

Some reports publish the shareholders table as an **image** or with a broken
text layer. Since v0.59 those land as a **residual with a clear warning** — an
honest gap, never a silent one and never a guess (the in-app OCR/AI reading is
retired). Enter the stakes manually from the filing, or leave the residual as
the marker of what the automaton could not read.

## Getting started

Nothing to configure. Track a company → the section appears (empty at first,
with a one-click "Extract from reports" backfill for companies tracked before
v0.56). Fund exits and founder moves feed the alerting rules and the
[red-flags panel](red-flags.md).
