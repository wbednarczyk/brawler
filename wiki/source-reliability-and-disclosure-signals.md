# Source reliability & disclosure signals

Brawler v0.55 makes the feed **harder to fool and harder to starve**: a second
official channel now audits what your primary source delivers, and two new
high-signal disclosure categories land in your feed and alerts. Everything here
is decision support only — facts with sources, never buy/sell/hold advice.

## The ESPI witness — a second pair of eyes on official reports

Official company reports enter Brawler through one primary channel (Bankier).
Since v0.55 the **official GPW ESPI/EBI listing runs as a witness**: after each
refresh it compares its list of disclosures against what the primary channel
delivered for your tracked companies.

- **Agreement** is recorded quietly — you see nothing, which is the goal.
- **A report the primary channel missed** lands in the Dziś stream (and the
  morning briefing) as "an official report your feed didn't catch", with the
  report's title; **Review opens the report itself** at the GPW source. No rule
  setup needed — it's on by default.
- The witness **never adds items to your feed** — no duplicates, ever.

In **Sources** the witness carries a "Witness" badge (it's a health mechanism,
not a feed). The full comparison ledger — matched / primary-only / witness-only
pairs — lives in **Diagnostics → Source reconciliation** (developer mode).

## KNF short-selling register

The public KNF register of net short positions (≥ 0.5% of a company's capital)
is now a source. Daily, for your tracked companies:

- a fund **entering, increasing, decreasing, or exiting** a short position
  becomes a feed item and a **"Short position" signal** (badge + filter),
- you can attach an **alert rule** to it like to any signal category,
- the company dashboard offers a **"Krótka sprzedaż (KNF)" panel** (add it from
  the panel palette): current holders and sizes, a 30-day change, and the full
  history of moves. Most companies show the calm empty state — no registered
  positions.

## Auditor-opinion signal

Filings whose titles carry auditor red flags — **qualified opinion, disclaimer
of opinion, negative opinion, going-concern emphasis** (Polish phrasings like
"opinia z zastrzeżeniem", "odmowa wyrażenia opinii", "kontynuacja
działalności") — are classified into a dedicated **"Auditor opinion"** category
with a danger badge. These are among the strongest public warning shots a
company can receive; the category is alert-rule-capable and will feed the
red-flags panel planned for v0.57.
