# ADR 0107: Company View Paradigm — Engine-Free Main Surface

Status: Accepted (2026-08-25, owner approval of the F3a experience contract after
a 7-round adversarial review; epic #410/F3a #429)

Deciders: maintainer. Area: frontend, IA, cockpit, market data.

## Context

The F3a Phase-A study (real base, 52 companies + contact-sheet vision pass +
journey cost mapping) found the panel-grid dashboard serves ~⅓ of its content:
only glanceable row-shaped surfaces earn co-visible slots, workflow surfaces are
50–85% chrome/empty in a slot, the grid multiplies duplicated figures, and the
freeform compose-a-view flow has zero real adoption while its persistence is
broken (filling a fresh named view is never saved). The owner's product goal:
*everything about a company from one place, easily*. Founding complaint (#410):
four vocabularies over an unguessable preset↔save↔reset model.

## Decisions

1. **The company surface is the `Spółka` screen — no docking engine.** Three
   zones: glance bar (identity + attention counters with typed drill targets),
   core (CSS grid of co-visible dense surfaces: annual KPI table, company feed,
   price chart, report coverage, analyst recommendations), workshop (bottom
   tool bar; a tool opens INTO the core zone, the core collapses to a summary
   strip). It replaces the curated per-company dashboard path (amends
   [ADR 0057](0057-composable-views-and-curated-dashboard.md)); the mode nav
   becomes Dziś / Inbox / Spółka.
2. **Routes are a discriminated union** — `{kind:"company", companyId, tool?}`
   | `{kind:"namedView", layoutId}`, with `tool` a closed per-variant-payload
   union (15 variants). Invalid combinations are unrepresentable; every
   glance-counter and deep-link intent maps to a typed destination.
3. **One composed read model `get_company_view`** feeds the whole screen (one
   invocation, one pending state, closed per-section error map — the F2
   pattern); counter semantics are normative in contracts.md.
4. **Price charts are daily OHLC candlesticks on a logarithmic axis** (house
   standard; ~3M window on the company card; series reused from
   `compute_price_context`, rendered by the shared `CandlestickChart`
   primitive extended with an opt-in log scale).
5. **Freeform layout structure is FROZEN until the #414 engine decision** (resolved 2026-08-28 by [ADR 0108](0108-retire-docking-engine.md): removed): view
   creation, panel add/close/drag and preset application are removed; existing
   named views and the four legacy `dashboard:*` rows stay reachable read-only
   ("Dawny dashboard · TICKER"), with domain editing inside panels fully
   writable. Rationale: zero adoption + a proven data-loss path; building
   layout persistence for a surface whose engine is under review is waste.
   dockview remains ONLY behind these frozen views; #414 decides stay/replace/
   remove with F3a as production evidence that the main surface needs no
   docking engine.
6. **Workshop tools own a dirty seam** (`ToolHost { isDirty, discard }`)
   enforced on every unmount path including app close; switching the company
   closes the open tool (stay/discard when dirty) — tools never silently
   retarget across companies.

## Consequences

- J4/J6/J7 journeys are redefined off the frozen creation flow (Report Season
  screen; Spółka→Dziennik; Events→Watchlists→Research→Spółka) with re-based
  budget floors measured at implementation.
- The palette collapses to the ADR 0104 dictionary with `{actionKey, verb}`
  metadata and a copy gate; the frozen cockpit's palette is navigation-only.
- The `PriceContextSection` chart migrates to the log axis (decision 4 is
  global for price charts).
- **Amendment (2026-09-03, F3c #197):** the workshop bar is an APG toolbar (one
  Tab stop, arrow traversal), `Ctrl+.` focuses it, `H`/`L` cycle tools, `Escape`
  returns to Overview through the dirty seam (dec. 6), and adjacent-company
  shortcuts close the tool (dec. 6, never retarget). Keyboard contract:
  `docs/plans/frontend-v2-f3c.md`; ADR 0076 dec. 9 amendment carries the
  focus-ring rule.
- Details, slices, state matrix and proofs: `docs/plans/frontend-v2-f3a.md`
  (approved experience contract); mockups:
  `docs/mockups/frontend-v2-widok-spolki/`.

## Rejected

- **Fixing freeform persistence now** — work on a surface whose engine fate is
  undecided (#414); freezing preserves the data and the optionality at ~zero
  cost.
- **Feed-item popup in Spółka** — a third surface competing with Inbox; the
  workshop zone hosts the shared per-kind detail instead (Inbox keeps
  cross-company triage; read state is shared).
- **Master-detail everywhere** — earnings deep-dive genuinely needs 2–4
  co-visible panes; the two-tier model keeps them without a docking engine.
