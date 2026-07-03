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

- **[The research workspace](research-workspace.md)** — how the app is laid out:
  the sidebar modes, the **Today** attention home, pinned companies, opening a
  company's dashboard, and full-screen **Focus** reading/writing.
- **[Autopilot](autopilot.md)** — let Brawler detect, fetch, and extract a
  company's new report automatically, per company, with everything cited,
  flagged, and reversible.
- **[Composable cockpit views](cockpit-views.md)** — build your own
  multi-panel dashboards from a grid and a panel picker, save them by name,
  and switch between them from the sidebar.
- **[Per-company settings](company-settings.md)** — quick single-company
  controls (autopilot mode, IR reports URL) and the bulk **Manage settings**
  surface for changing several companies, or a whole watchlist, at once.
- **[Quality frameworks](quality-frameworks.md)** — score a company against your
  own quantitative quality checklists, built from the company's reported
  fundamentals.
- **[DSL reference](dsl-reference.md)** — the small expression language used to
  write quality-framework criteria (e.g. `roic >= 15%`,
  `net_debt_to_ebitda < 2.5 AND fcf > 0`).
- **[On-device semantic similarity](embedding-model.md)** — an optional, local,
  no-API-key embedding model that finds feed items similar *in meaning*, not just
  by keyword.
- **[Report comparison](report-comparison.md)** — see what changed between a
  company's two most recent financial statements of the same kind, section by
  section — fully local, deterministic, no AI.
- **[AI provider pools and the OpenAI-compatible provider](ai-provider-presets.md)**
  — route each AI capability to its own provider with a failover pool, and add
  free/self-hosted open-model hosts (Groq, OpenRouter, local Ollama, and more)
  alongside Gemini, Claude, and OpenAI.

## A note on what Brawler is — and isn't

Brawler is **decision support**, not advice. It surfaces sourced facts and
computed analysis to help *you* decide. It never tells you to buy, sell, or
hold, and the quality frameworks cannot be made to output that either — a
criterion can measure `roic >= 15%`, but there is no "score → buy" step.

Your data — watchlists, frameworks, facts, and results — stays local on your
machine.
