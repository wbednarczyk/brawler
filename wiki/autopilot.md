# Autopilot (the autonomous report pipeline)

**Autopilot** closes the loop on a periodic report: when a company you track
publishes a new report, Brawler can detect it, fetch it, read out the figures
(where the format allows a deterministic read), work out what changed, and
check it against your open claims and research — with no manual steps. The
result shows up as one notification on **Today**.

It's opt-in **per company**, and it's still decision support only: the result
tells you *what changed* and *what to verify*, never buy/sell/hold.

## Turning it on

Autopilot is **off by default** for every company. Open the company (Spółka screen) → **Open fundamentals**
→ the **Fundamentals** panel to set its mode for that one company, or use
**Companies → Manage settings** to set the same mode across several companies
at once (see [Per-company settings](company-settings.md)).

There are three modes:

- **Off** — nothing automatic for this company's reports. (Core KPIs still
  arrive from the daily aggregator pull — that source is company-independent.)
- **Assist** — on detecting a new report, Brawler fetches it, diffs it against
  the previous one, and runs the deterministic extraction.
- **Autopilot** — the same full loop, plus claims/research cross-references.

Since v0.59 facts are **review-free in every mode**: whatever a deterministic
reader emits is saved immediately, fully cited, and labeled with its origin —
there is no pending queue and nothing to confirm. The difference between the
modes is how much of the loop runs, not whether you must click through the
results. Switching a company's mode never changes facts or runs it already
produced.

## Where results show up

Every run appears as an **Autopilot card** on **Today**: a summary of the
report and a **Review** button that opens the company. If a run couldn't
finish, the card says so honestly. For a **PDF report** the card says *"PDF
report — core figures arrive from the aggregator source"* — that's the normal,
by-design state (Brawler no longer machine-reads figures out of PDFs), not a
failure.

### Undo

**Undo** on a run's card reverts exactly the facts that run produced — nothing
from any other run or manual entry. It's a two-step confirm to avoid a stray
click undoing real work; once done, the card shows **"Reverted N facts."**
Undo is idempotent and appears on any run that actually produced facts.

## What "provenance" means on a fact

Every fact carries a label saying **how it got into Brawler**: the issuer's
ESEF filing, a structured/positional xHTML read, the ESPI cover-note table, or
the BiznesRadar page it was read from — with a citation down to the row. See
[Fundamentals & Coverage](fundamentals-coverage.md) for the full trust ladder
and cross-checking rules.

## What Autopilot needs

- **The app open.** Autopilot runs while Brawler is running — it does not
  fetch or analyze anything while the app is closed.
- No AI and no API keys: the whole pipeline is deterministic. (Intelligence —
  summaries, judgment, narratives — comes from your own agent over the MCP
  port; see [MCP server](mcp-server.md).)

## A note on trust and advice

Autopilot never tells you to buy, sell, or hold. It tells you what a new
report changed and what's worth double-checking — sourced, labeled, and
always reversible.
