# Experience Contract Template

Reusable source for the UX-first planning artifact required by [ADR 0081](../adr/0081-ux-quality-loop-v2.md)
before non-mechanical UI work (a new panel/screen, functional redesign, changed
cross-screen journey, or new primary user decision). Copy/token-only fixes,
primitive-preserving mechanical migrations, and exact regression repairs are exempt
unless they change a journey. See [ui-authoring.md](../ui-authoring.md) § Experience
contracts, storyboards & discoverability for the trigger/exemption/storage/approval
flow.

**How to use:** copy this file's 12 sections into the feature plan section for the
task (do not add or drop sections — this is the ADR 0081 required set). Fill every
field; any `N/A` requires a written reason, not a blank cell. Pair it with a copy of
[`docs/mockups/STORYBOARD-TEMPLATE.html`](../mockups/STORYBOARD-TEMPLATE.html) for the
visual storyboard. The textual contract lives in the plan; the storyboard lives under
`docs/mockups/` — never only in a session scratchpad or `test-results/`. Both need
explicit owner approval before they are normative (a copied template is a draft, not
an approval).

## 1. Card, plan section, owner, status, served journey, first red journey test

| Field | Value |
| --- | --- |
| Radicle card | `<hex7>` |
| Plan section | `docs/plans/<file>.md#<section>` |
| Owner | `<name>` |
| Status | draft / owner-review / approved |
| Served journey | `<Jn — name, docs/ux-journeys.md>` |
| First red journey test | `<path to spec + case that must fail before this ships>` |

## 2. User / context and trigger

`<Who is using this, in what situation, and what event/action puts them here?>`

## 3. Desired outcome and the decision being made

`<What does the user know or accomplish when this is done? What single decision are
they making?>`

## 4. Evidence required before that decision

`<What facts must be visible/verifiable before the user can make the decision in §3?>`

## 5. Information hierarchy

| Tier | Content |
| --- | --- |
| Must-see | `<always visible, no interaction needed>` |
| Secondary | `<visible but subordinate>` |
| Hidden until needed | `<behind an expand/drill-in/detail action>` |

## 6. Primary action

`<Exactly one primary action for this surface, or an explicit reason why no single
primary exists (e.g. a quiet/empty state has no artificial CTA).>`

Mark it in code with `data-ux-primary-action="true"` on the `Button` (never inferred
from `variant="primary"` or a CSS class — ADR 0081 Q4). Verify with
`expectPrimaryActionCount(surface, { max: 1 })`
(`tests/browser/helpers/interactionContracts.ts`). A surface needing more than one
marked primary action is a multi-primary exemption: `max > 1` with a non-empty
`reason` that must match this field's rationale — a bare code exemption with no
entry here is not a valid exemption.

## 7. Entry path, exit/next step, and recovery/undo path

| Path | Description |
| --- | --- |
| Entry | `<how the user arrives here>` |
| Exit / next step | `<where the primary action or "done" leads>` |
| Recovery / undo | `<what happens when it goes wrong, and how the user undoes/retries>` |

## 8. Done-well criteria

`<Success criteria distinct from "record saved" — e.g. time-to-signal, no silently
absent item, no duplicate/stale application. Must be checkable, not just asserted.>`

## 9. Assumptions and explicitly excluded redesign scope

| | |
| --- | --- |
| Assumptions | `<what this contract takes as given, e.g. existing data shape, existing nav>` |
| Excluded scope | `<what this contract deliberately does NOT redesign>` |

## 10. Storyboard frame table

Mirrors the frames in `STORYBOARD-TEMPLATE.html`; link the completed visual storyboard
here once committed under `docs/mockups/`.

| Frame | State named | Primary action | Feedback | Intended focus |
| --- | --- | --- | --- | --- |
| Entry | | | | |
| Before action | | | | |
| Loading / in-flight | | | | |
| Success | | | | |
| Error | | | | |
| Undo / recovery | | | | |
| Narrow pane | | | | |

Storyboard: `docs/mockups/<file>.html`

## 11. State matrix

Required rows: empty, loading, partial, success, error, stale, dense. Any `N/A`
requires a written reason in that cell.

| State | User sees | Primary action | Feedback/recovery | Automated proof | Human review |
| --- | --- | --- | --- | --- | --- |
| Empty | | | | | |
| Loading | | | | | |
| Partial | | | | | |
| Success | | | | | |
| Error | | | | | |
| Stale | | | | | |
| Dense | | | | | |

## 12. Test-layer plan (multi-slice tasks)

Assign every behavior to the **cheapest authoritative layer** ([ADR 0081](../adr/0081-ux-quality-loop-v2.md) Q8; the responsibility table lives in [testing.md § Frontend test responsibilities](../testing.md#frontend-test-responsibilities)). For a single-slice task, mark `N/A — single slice` with a reason.

| Field | Value |
| --- | --- |
| Component proof (Vitest) | `<controlled state / forms / error / async-transition cases, per screen>` |
| Cross-screen browser proof (Playwright) | `<the tests/browser/*.spec.ts that own each cross-screen journey — named BEFORE the slices>` |
| Integration-seam proof | `<any last-intent/ordering/stale-response seam + its authoritative layer (often a controller renderHook, not the full app)>` |
| Consumer map | `<who consumes each new src/api export / read model — clear knip orphans before calling the UI done>` |
| Full-wave verification | `<the cross-cutting browser specs + full make check to run AFTER the last slice, before the final gate — not per-slice only>` |
