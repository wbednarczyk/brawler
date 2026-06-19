# Brawler Wiki

This is the **user-facing guide** to Brawler — how to actually use the app's
features. It is written for you, the investor using Brawler, in plain language.

It is deliberately separate from the [`docs/`](../docs) folder, which holds the
project's canonical specifications, contracts, and architecture decisions (for
contributors and agents). If `docs/` says *what the app is*, this wiki says
*how to use it*.

This is the first of many pages — more will be added as features land. One page
per feature.

## Pages

- **[Quality frameworks](quality-frameworks.md)** — score a company against your
  own quantitative quality checklists, built from the company's reported
  fundamentals.
- **[DSL reference](dsl-reference.md)** — the small expression language used to
  write quality-framework criteria (e.g. `roic >= 15%`,
  `net_debt_to_ebitda < 2.5 AND fcf > 0`).
- **[On-device semantic similarity](embedding-model.md)** — an optional, local,
  no-API-key embedding model that finds feed items similar *in meaning*, not just
  by keyword.

## A note on what Brawler is — and isn't

Brawler is **decision support**, not advice. It surfaces sourced facts and
computed analysis to help *you* decide. It never tells you to buy, sell, or
hold, and the quality frameworks cannot be made to output that either — a
criterion can measure `roic >= 15%`, but there is no "score → buy" step.

Your data — watchlists, frameworks, facts, and results — stays local on your
machine.
