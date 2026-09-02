# ADR 0108: Retire the Docking Engine — No dockview, No Cockpit, No Named Views

Status: Accepted (2026-08-28)

Supersedes [ADR 0053](0053-dockview-layout-pilot.md) (dockview as the docking
shell) and [ADR 0057](0057-composable-views-and-curated-dashboard.md)
(composable views, the "+" new-view model). Amends [ADR 0054](0054-mode-based-thesis-centric-shell.md)
(dockview as the workspace engine — removed) and [ADR 0107](0107-company-view-paradigm.md)
decision 5 (the freeze resolves to removal). Closes the #414 study; #216
(dockview 6→7/8 migration) and #228 (cockpit shell v2) are superseded.

## Context

ADR 0107 froze the freeform cockpit "until the #414 engine decision" and
shipped the engine-free `Spółka` screen as production evidence. The study
(#414, method: derive requirements from the shipped model, score the field,
decide by evidence) closes on these facts:

- **Adoption** (owner's real database, 2026-08-28): `cockpit_layouts` holds
  the four auto-generated `dashboard:*` rows and **zero** user-created named
  views. The F3a study (§5) reads this as *pre-adoption* evidence, not proof
  that composition is unwanted — so the verdict below stands on the F3a
  findings (density, duplicated figures, navigation cost; study §4), and
  keeps a real re-entry path.
- **F3a in production** (v0.74–v0.75): every panel kind the cockpit hosted is
  a `Spółka` workshop tool or a standalone screen; one tool opens into the
  core at a time and the core collapses to a summary strip. The only cockpit
  capability with no replacement is the simultaneous linked-selection triad
  (feed → inspector → claims) — never used.
- **Field survey** (#414 comment, 2026-08-20): dockview is on v8 and went
  open-core (pinned/multi-row tabs, spatial keyboard docking, layout history
  behind the paid `dockview-enterprise`); 6→8 carries silent breaks
  (packaging split, `onDidActivePanelChange` payload). golden-layout and
  rc-dock are stalled/abandoned; react-mosaic has no a11y/pop-out story;
  Lumino needs a bespoke React adapter; flexlayout-react is credible but
  bus-factor-1 without dockview's a11y pack; react-resizable-panels is the
  best-maintained "less framework" primitive (panels only, no tabs/docking).
- **Cost of the freeze**: `dockview` 6.6.1 pinned behind a dependabot ignore,
  ~2 900 lines of cockpit source/tests, a 536-line theme override, four IPC
  commands, a table, and three issues blocked on this decision.

## Decision

1. **The shipped model needs no layout engine.** Requirements derived from
   the F3a experience contract, scored against "does this need an engine?":

   | Requirement (shipped) | Engine needed? | Served by |
   | --- | --- | --- |
   | Co-visible dense core (KPI table, feed, price, coverage, recommendations) | no | CSS grid (`.spolka-core`) |
   | One tool at a time, core collapses to a strip, untouched return | no | workshop bar + `ToolHost` |
   | Dirty seam on every unmount path incl. app close | no | `ToolHost { isDirty, discard }` |
   | Narrow window: stack, never clip; internal scroll | no | container queries, `.spolka-body-scroll` |
   | Keyboard + a11y (landmarks, one `aria-current`, palette) | no | ADR 0104 primitives, palette |
   | Theming (dark default, light variants) | no | tokens |
   | Bundle / maintenance / paid-tier exposure | — | zero engine = zero exposure |
   | Persistence of user layouts, pop-out, drag-docking, split/tab groups | **not required** | — |

   The last row is marked *not required*, not a "win" for the no-engine
   option: nothing in the shipped journeys (J1–J7) arranges panels.

2. **Field verdict**: with no requirement calling for an engine, every
   candidate is rejected on cost-vs-need. Eliminated on health alone:
   golden-layout, rc-dock. Eliminated on capability: react-mosaic (no
   a11y/pop-out), Lumino (adapter cost). Shortlisted and rejected:
   dockview 8 free tier (89 kB, single maintainer, open-core drift, a 6→8
   migration on a surface nobody uses) and flexlayout-react (same bus factor,
   no a11y pack). "Less framework" (react-resizable-panels + own tabs) is
   recorded as a **deferred hypothesis**, not a preselected answer.

3. **Remove, don't freeze**: dockview, the cockpit screen, the "Views"
   sidebar group, the `Open view:` palette entries, the `Section` variant, the
   four `cockpit_layouts` IPC commands and their store are deleted; the
   `cockpit_layouts` table is dropped by a forward, idempotent migration
   (0152 — the pre-migration snapshot is automatic). The shared company panels
   the cockpit hosted move to `src/screens/Spolka/panels/` (they are Spółka
   tools now); the standalone Decision Journal route keeps its global panel.
   Companyless research brief/digest search results open the standalone
   Research route.

4. **Re-entry rule**: a multi-pane layout returns only through a new ADR
   triggered by a **real journey** that needs two simultaneously visible,
   independently scrollable/resizable surfaces which fixed CSS plus workshop
   switching cannot serve — proven on real usage, with a fresh evaluation of
   the field at that time. ADR 0052 requires report diff to be *reachable*
   from the company surface (it is: the `diff` tool), not side-by-side; it
   does not trigger this rule by itself.

## Consequences

- #414 closed by this ADR; #216 and #228 closed as superseded (#133, the
  activity center, re-parents to #410 first); #197 is re-scoped to the
  keyboard model of the Spółka workshop and the palette.
- `docs/retired-surface.json` gains the retired tokens (`dockview`,
  `cockpit_layouts` and its four commands, "Research cockpit",
  `.cockpit-pane`, `cockpit-palette`) so no live doc re-specifies them.
- Retreat ledger: [bad-ideas.md](../bad-ideas.md) ("app-wide docking shell").
- Data: the four `dashboard:*` rows are auto-generated and unrecoverable by
  design; nothing user-authored lives in `cockpit_layouts`.

## Rejected

- **Keep the freeze** — dead weight, a permanent dependabot ignore, and a
  navigation surface ("Views") that duplicates Spółka for four companies.
- **Migrate to dockview 8** (#216) — an epic-grade migration with silent
  breaks, on a surface with zero adoption, into an open-core dependency.
- **Switch to flexlayout-react** — pays the same bus-factor risk for a
  capability no requirement asks for.
- **Pre-commit to "less framework" now** — building resizable splits without
  a journey that needs them repeats the cockpit's mistake.

## Amendment (2026-09-02, #410 F4c S2)

Decision 3's "the standalone Decision Journal route keeps its global panel" is reversed: the **global Journal route and the global Notebooks screen are retired** (owner decision 2026-09-02 on the evidence: 7 notes across 6 companies and 2 journal entries in three months — the cross-company review surfaces had nothing to review). The per-company `dziennik` and `notatnik` Spółka tools are the only note/journal surfaces; every deep link that landed on the global Notebooks screen (Inbox feed-item draft, research evidence, global search, transcript) opens the company's `notatnik` tool through a typed route intent (`{ t: "notatnik"; entryId?; draft? }`, `navigateToCompanyNotebook`). Cross-company reads stay on the MCP port. Retired tokens: `docs/retired-surface.json`; ledger row: [bad-ideas.md](../bad-ideas.md).
