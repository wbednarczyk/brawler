---
name: investing-domain
description: Domain lens for designing/building Brawler — how an equity investor actually reads companies (fundamental analysis, quality assessment, GPW specifics). Load when designing UI hierarchies, panels, journeys, or analytics for company data.
---

# Investing domain lens

How the app's user thinks. Design decisions (what is co-visible, what is
glanceable, what is a deep tool) must derive from this practice, not from
generic UX instinct. Owner-reviewed; extend with the owner's own style.

## The investor's recurring questions (each maps to a surface)

1. **What changed?** — new filings/news since I last looked, and is any of it
   thesis-relevant. (Daily; seconds per company. → glance layer.)
2. **Is the thesis intact?** — what did I claim/believe, what did management
   promise, did reality deliver. (Weekly/eventful. → claims + journal + notes.)
3. **What's the quality?** — durable returns on capital, balance-sheet
   strength, moat, management credibility. (Quarterly, slow-moving. → scores
   as badges, full framework as a workshop.)
4. **What do the latest numbers say vs expectations?** — report season: actuals
   vs my expectations vs guidance. (Bursty, calendar-driven.)

## Numbers that are read TOGETHER (co-visibility rules)

- Revenue + operating margin + cash conversion — growth without cash is a
  warning, never show revenue alone as "performance".
- ROIC/ROE vs leverage — returns bought with debt are a different animal;
  quality panels must pair them.
- Guidance/claims vs delivered KPIs — management credibility is a LEDGER:
  promise, date, outcome. This is Brawler's differentiator (claims tracking).
- Report actuals vs prior period vs same-quarter-last-year — PL reporting is
  seasonal; QoQ alone misleads (yoy is the default comparison).
- Short interest + free float — pressure indicators only meaningful together.
- Price context is background, not signal — Brawler is not a trading tool
  (ADR/product intent); quotes support "did the market react", nothing more.

## Cadence shapes the layout

- **Daily scan** (seconds): deltas only — new filings, fired signals, upcoming
  events. Counts and one-line headlines; anything requiring scroll is too much.
- **Report season** (hours, calendar-driven): who reports when, my prep state,
  expectations written BEFORE the report (bias control), then actuals diff.
- **Deep-dive** (open-ended): all tools on one company — filings reader,
  fundamentals tables, notes, claims ledger, quality framework. Needs 2–4
  co-visible panes; this is the one genuinely multi-pane task.

## Quality assessment practice (what "ocena jakości spółki" means)

- Piotroski F-score, Altman Z — screening heuristics, not verdicts; show as
  scores with drill-down, never as standalone panels.
- Moat/durability judgments are QUALITATIVE — checklist verdicts (pass/partial/
  fail/insufficient evidence) with cited evidence, never a bare number.
- Management credibility = claims ledger over time (promises kept ratio beats
  any single metric for GPW small/mid caps).

## GPW/Poland specifics

- Filings: ESPI (current reports — material events) vs EBI (governance);
  periodic reports Q1/H1/Q3/annual with statutory deadlines — non-arrival of an
  expected periodic report is itself a signal (report_delay).
- Ownership: 5% threshold disclosures, management deals (MAR), free float from
  stakes — ownership changes are events, not static facts.
- Short positions: public KNF registry above 0.5% — sparse but high-signal
  when present.
- Language: sources are Polish; numbers use spaces as thousands separators and
  comma decimals in UI (locale), but data pipeline is normalized.

## Design implications (the contract with UI work)

- Provenance is non-negotiable: every displayed figure traces to its source
  document (ADR 0104 provenance thread) — an untraceable number is a defect.
- Glance → core → work: counts/deltas first, dense co-visible tables second,
  workshops (authoring, frameworks, readers) on demand — matching cadence.
- Decision support, never advice: no buy/sell/hold phrasing anywhere.
- Empty states in sparse domains (claims, shorts, notes) are invitations with
  the action to start the ledger — sparseness is expected early, not failure.
