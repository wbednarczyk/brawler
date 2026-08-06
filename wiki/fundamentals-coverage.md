# Fundamentals: how figures arrive, and the Coverage panel

Since v0.59 Brawler is a **full automaton** for core financial figures: numbers
simply arrive, honestly labeled with where they came from. There is no review
queue and nothing to confirm — reviewing every figure by hand kills the app's
usability, so the app never asks you to.

It is all **local** and **decision support only** — Brawler reads what a company
reported; it never tells you to buy, sell, or hold.

## Where the numbers come from

Four readers, highest trust first. When two cover the same figure, the more
trusted one owns the slot:

1. **Your own edits** — untouchable. No automatic path ever overwrites a value
   you typed in. If a source later disagrees with you, Brawler records an
   informational note (see *Flagged periods*), never a change.
2. **ESEF annual reports** — the issuer's own iXBRL-tagged filing. The only
   source for figures the aggregator doesn't publish (cash, EPS, long-term
   debt, total liabilities, group equity).
3. **Structured/positional xHTML** filings and the **ESPI "Wybrane dane
   finansowe" cover table** from interim komunikaty (parsed the moment the
   feed item arrives — the figures outlive the carrier item).
4. **BiznesRadar — the primary source for core KPIs.** Once a day Brawler
   politely reads three public report pages per tracked company (income
   statement, balance sheet, cash flow) and ingests **every period column**
   they carry — a newly tracked company gets its whole reported history on day
   one. Values are attributed to the page; empty or zero cells are never
   treated as data.

**PDFs are for you, not for the machine.** Brawler no longer parses financial
figures out of PDF statements (every issuer's layout was a new fight and a new
source of silent errors). A PDF report's Today card says plainly: *"PDF report
— core figures arrive from the aggregator source"*. Reading the filing yourself
and adding a figure by hand (or via an MCP agent, later) covers the long tail.

## Every value carries its origin

Click any figure in **Fundamentals → Financial facts** to see its provenance:
the source tier (issuer filing / cover note / aggregator page), the extraction
method, and a citation naming the exact row and page or document it was read
from. Origin is a **label, never a to-do**.

## Cross-checking (who watches whom)

- Where the issuer's filing holds a figure and BiznesRadar disagrees, the
  disagreement is recorded as an informational entry — the filing always wins.
- The same for your manual entries: the aggregator's dissent is noted so you
  learn of it, and your value stays.
- **Agreement is recorded too.** When BiznesRadar reads the same figure the
  filing reports, that confirmation is stamped on the value (the figure seen,
  the page it came from, and when) and the figure is marked **confirmed by a
  second source** — so "nobody has checked this" and "two independent sources
  agree" stop looking the same. A value you typed yourself gets the note beside
  it, but keeps your grading: the app never re-labels your own entry.
- Re-reading a report that yields a **different figure than the one already on
  file** never overwrites it — the disagreement is recorded, with both numbers
  shown side by side.
- If one metric starts disagreeing across many companies at once, Brawler
  raises a *mapping suspect* warning — that pattern means a source row is
  being misread, not that ten companies restated at once.

## The Coverage panel

**Coverage** (company workspace) shows, per reporting period: the canonical
report document, how many facts landed and how many were cross-validated, and
**Flagged periods** — an informational list of periods where a reader ran and
refused to emit (a failed consistency check, a source disagreement), each with
a plain-language reason and a **Try again** action. Below it, **Flagged
figures** lists the opposite case: values that *did* land but could not be fully
verified, each with its period, metric, amount, the reader that produced it and
the source label it was read from. Both are information, not homework: nothing
waits for your click.

Some cells are honestly empty: BiznesRadar's pages simply don't publish every
line (no cash, no total liabilities, equity only for parent shareholders), so
those figures exist only for periods with an ESEF filing.

## Fixing and adding figures

- **Edit or add a KPI value manually** in Fundamentals — your value takes the
  top of the trust ladder.
- **Undo an autopilot run** from its Today card (two-step confirm) to revert
  exactly the facts that run produced.
