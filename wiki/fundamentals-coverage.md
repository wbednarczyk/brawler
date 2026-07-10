# Fundamentals coverage and history backfill

When you track a company, you want its reported numbers — revenue, profit, the
figures behind your analysis — actually *in* Brawler, period by period, without
hunting through PDFs and clicking "extract" on each one. This page covers the
tools that get you there: the **Coverage map**, one-click **history backfill**,
automatic extraction, the **Review queue** for anything that needs your eye, and
the **AI spend budget** that keeps it all under control.

It is all **local** and **decision support only** — Brawler reads what a company
reported; it never tells you to buy, sell, or hold.

## The Coverage map

Open a company's dashboard → the **Coverage** panel. It's a table with one row
per reporting period (newest first), and four columns:

- **Period** — the fiscal year and period (FY, H1, Q1, Q3…).
- **Report** — the canonical report for that period, with a chip for its kind
  (and an ESEF chip when it's a structured filing). If no report was found for
  the period, it says so plainly.
- **Data** — how many figures are recorded, split into **validated** and
  **still-to-review**. If a report is fetched but not yet read, this cell reads
  **"not processed → Extract"**. If the only filing is a link (metadata, no
  stored file), it reads **"link-only — no stored file"**.
- **To review** — a count of figures awaiting your confirmation. When it's
  above zero, click it to jump straight to the **Review queue** (below).

A period shows up if *any* of these name it — a report, a recorded figure, or a
pending proposal — so a gap is never silently missing. Clicking a row (anywhere
outside the *To review* cell) opens the company's **Report documents**.

## One-click backfill and extraction

At the bottom of the Coverage panel are two actions:

- **Backfill history** — fetches the company's **past** reports and filings
  (back as far as your configured depth, see Settings below), then
  **automatically extracts** the figures from them. One click: add a company,
  hit Backfill, and its fundamentals fill in — no per-document clicking.
- **Extract missing periods** — runs the extraction step **only**, over reports
  Brawler already has stored. Use this when the documents are already there and
  you just want the numbers read out.

A status line tracks the work as it runs — *backfilling → extracting N/M →
done* — and settles on the result. A company with **automation off** disables
both actions with an "automation off" hint rather than doing nothing silently
(turn automation on in the Fundamentals panel; see [Autopilot](autopilot.md)).
If a backfill reaches its page limit before your full depth, it warns you that
older filings may be missing rather than pretending it got everything.

## How figures get read

Most reports are read **deterministically** — straight out of the filing, with
no AI and no guessing. This now includes interim (quarterly and half-year)
reports that ship as web-page renderings, including ones whose layout only makes
sense read by column position. A clean deterministic read is the most reliable
path and needs no confirmation.

When a report **can't** be read that way, an optional **OCR** step (Mistral,
free tier) reads it instead. The **first** OCR read for a company lands as
**proposals** you confirm — and confirming the first one teaches Brawler that
company's report layout, so its later reports read straight through without
asking again. (For what "structured" vs. "AI" provenance means on a saved
figure, see [Autopilot → provenance](autopilot.md).)

## The Review queue

Anything that isn't a clean, validated read lands in the **Review queue** panel
instead of failing quietly. Reach it from the Coverage map's *To review* cell,
or add the **Review queue** panel from the panel picker.

It lists every pending figure, grouped by period, each row showing the proposed
metric and value, its source snippet and document, and a tag for where it came
from:

- **OCR bootstrap** — read from a not-yet-confirmed OCR layout. Confirming it
  also confirms the layout.
- **OCR · flagged** — a deterministic read that Brawler's accounting-identity
  check flagged for a second look (a caution, never a block).
- **AI** — an older text-AI proposal.

**Confirm** records the value as a fact (a flagged read is confirmed with a
caution, not blocked); **Reject** discards it. If you confirm a value that
conflicts with one already recorded, Brawler tells you both values and records
nothing rather than overwriting. An empty queue means the deterministic pipeline
is writing figures directly, with nothing left for you to check.

## Settings

- **Backfill history depth** (Settings → Sources) — how many years back a
  backfill reaches. Clickable presets plus a slider; default **3**, up to
  **10**.
- **History-sweep AI budget** (Settings → AI) — caps how many OCR/AI calls a
  single history sweep may spend, so a big backfill can't run up your free-tier
  usage. Presets **0 / 10 / 30 / 100** (or type your own), default **30**, and
  **0 means no limit**. The Coverage footer shows the latest sweep's spend
  (e.g. *"AI: 2/30"*). The budget is snapshotted when a sweep starts, so
  changing it only affects future sweeps; a sweep that hits its cap marks the
  remaining periods **"Skipped — AI budget"** rather than dropping them
  silently — run another sweep (or raise the budget) to finish them.

## When something can't be read

Brawler tells you plainly instead of pretending:

- A backfill on an **unsupported market** stops immediately with a clear
  message rather than doing nothing.
- OCR/AI reads that **degrade** (a provider error, a report that isn't a PDF,
  an unconfirmed layout) leave a trail under **Diagnostics → Logs**.
- A report that genuinely can't be extracted is reported as such — never as an
  empty "success".
