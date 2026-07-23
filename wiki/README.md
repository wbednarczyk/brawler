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
- **[Fundamentals coverage and history backfill](fundamentals-coverage.md)** —
  the Coverage map (what has data, what's missing, per period), one-click
  automatic arrival of core figures (BiznesRadar-primary + issuer filings), the
  queue, and the AI spend budget.
- **[Price context, sectors, and company basics](price-context.md)** —
  automatic daily prices, the 52-week range, market cap and level-0 valuation
  ratios, the candlestick chart, sector classification with manual override,
  and the read-only Basic info panel.
- **[Ownership](ownership.md)** — who holds each company and how that changes:
  the current-structure donut with derived free float, capital % vs votes %
  kept separate, stakes over time with ESPI threshold events, automatic holder
  classification with confirm-only AI, the BiznesRadar witness, the **Insiders**
  block (MAR art. 19 transactions, management holdings, skin-in-the-game badge),
  and how unreadable shareholder tables surface as honest gaps.
- **[Composable cockpit views](cockpit-views.md)** — build your own
  multi-panel dashboards from a grid and a panel picker, save them by name,
  and switch between them from the sidebar.
- **[Per-company settings](company-settings.md)** — quick single-company
  controls (autopilot mode, IR reports URL) and the bulk **Manage settings**
  surface for changing several companies, or a whole watchlist, at once.
- **[Quality frameworks](quality-frameworks.md)** — score a company against your
  own quality checklists: quantitative criteria computed from reported
  fundamentals, plus qualitative criteria (moat, capital allocation…) assessed
  by an AI agent from your stored evidence, with citations. Also covers the
  published **health scores** (Piotroski F, Altman Z″) shown in the Quality tab.
- **[Red flags](red-flags.md)** — the per-company panel that surfaces auditor
  concerns, report delays, fund exits, score deteriorations, and short-selling
  spikes automatically, each with evidence, an acknowledge action, and alert
  wiring.
- **[Analyst recommendations](analyst-recommendations.md)** — broker ratings and
  target prices tracked as attributed third-party opinions: verbatim ratings
  with upgrade/downgrade markers, an append-only local history, a
  recommendation-change signal for alerts, and a "vs target" readout beside the
  price — never advice.
- **[DSL reference](dsl-reference.md)** — the small expression language used to
  write quality-framework criteria (e.g. `roic >= 15%`,
  `net_debt_to_ebitda < 2.5 AND fcf > 0`).
- **[Report comparison](report-comparison.md)** — see what changed between a
  company's two most recent financial statements of the same kind, section by
  section — fully local, deterministic, no AI.
- **[AI in Brawler: transcripts only (BYOA)](ai-provider-presets.md)**
  — route each AI capability to its own provider with a failover pool, and add
  free/self-hosted open-model hosts (Groq, OpenRouter, local Ollama, and more)
  alongside Gemini, Claude, and OpenAI.
- **[The decision journal and pre-report expectations](decision-journal.md)**
  — capture your buy/pass/keep-watching decisions in an append-only,
  evidence-linked journal, and write down pre-report expectations that freeze
  when the real numbers arrive — a factual mirror, never a grade.
- **[Attention alerts and the morning briefing](attention-and-briefing.md)** —
  set up alert rules over signals, autopilot runs, and price conditions; read
  the daily "what changed + what needs doing" briefing at the top of Today; and
  find fired alerts as pop-ups and in the Today attention list.
- **[Source reliability & disclosure signals](source-reliability-and-disclosure-signals.md)** —
  the ESPI witness that audits your official-report feed and warns when the
  primary source missed a disclosure; the KNF short-selling register as a
  signal + dashboard panel; and the auditor-opinion red-flag signal.
- **[The MCP server](mcp-server.md)** — let an AI assistant (Claude Code,
  Claude Desktop, …) work with your research through a localhost-only connector:
  read the whole workspace, and — when you allow writes — record notes, claims,
  facts, and verdicts, always with a source. Reference: enabling, security, and
  troubleshooting.
- **[Connecting an AI agent to Brawler](mcp-agent-guide.md)** — the hands-on
  how-to: connect Claude Code/Desktop step by step, turn on write tools, the
  per-write citation rules, the full tool catalog, and example workflows.

## A note on what Brawler is — and isn't

Brawler is **decision support**, not advice. It surfaces sourced facts and
computed analysis to help *you* decide. It never tells you to buy, sell, or
hold, and the quality frameworks cannot be made to output that either — a
criterion can measure `roic >= 15%`, but there is no "score → buy" step.

Your data — watchlists, frameworks, facts, and results — stays local on your
machine.
