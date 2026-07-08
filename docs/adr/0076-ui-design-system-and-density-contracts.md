# ADR 0076: UI Design System — Scales, Semantic Tokens, Format Rules, Density Contracts

Status: Accepted (v0.50.0 phase 2 — owner sign-off 2026-07-05)

Recurring UI defects (panel overflow, dual date formatters rendering the same timestamp two ways,
raw `1050000000` next to humanized values, ~20 ad-hoc font sizes and ~20 gap values, state chips on
never-assessed rows, native `confirm()` dialogs) share one root cause: **no normative design system
and no gates enforcing one**. Agents implementing UI have been making design decisions per panel;
each choice is locally reasonable and globally inconsistent. This ADR fixes the values and rules so
implementation tasks carry **zero design discretion**, and names the gate that enforces each rule
(ADR 0038/0045 posture). Execution: [docs/plans/v0.50-ux-overhaul.md](../plans/v0.50-ux-overhaul.md).

## Decision 1 — Spacing scale (4px grid)

`tokens.css` defines the only allowed spacing steps:

| Token | Value | Typical use |
|---|---|---|
| `--space-0` | 2px | hairline gaps inside chips |
| `--space-1` | 4px | icon↔label, chip padding-y |
| `--space-2` | 8px | row gaps, control gaps (today's most common: 8px×119) |
| `--space-3` | 12px | section-internal padding, toolbar gaps |
| `--space-4` | 16px | card padding, inter-group gaps |
| `--space-5` | 20px | panel padding |
| `--space-6` | 24px | inter-section rhythm |
| `--space-7` | 32px | screen-level margins |
| `--space-8` | 40px | hero/empty-state spacing |

Migration map (deterministic; the mechanical CSS migration uses exactly this): 2→0, 4→1, 5→1,
6→2, 7→2, 8→2, 10→3, 12→3, 14→4, 16→4, 18→5, 20→5, 24→6, 28→7, 32→7, 40→8; rem values convert at
1rem=16px then map. **Gate:** stylelint `declaration-property-value-allowed-list` for
`gap`/`padding`/`margin` (tokens + `0` + `auto` + `%` values). *(As delivered in U2: the migration
completed in one pass, so the planned per-file ratchet was never needed — the gate is an
unconditional allowlist from day one, which is stronger. Recorded at the v0.50.0 closure audit.)*

## Decision 2 — Type scale

| Token | Value | Use |
|---|---|---|
| `--font-2xs` | 10px | uppercase group labels ONLY (`.nav-group-label` idiom) |
| `--font-xs` | 11px | metadata, chips, table captions |
| `--font-sm` | 12px | secondary text, labels, dense rows (today's dominant size) |
| `--font-base` | 13px | body, list titles, controls |
| `--font-md` | 14px | panel/section headings |
| `--font-lg` | 16px | screen headings |
| `--font-xl` | 20px | display numbers (stat tiles) |

Migration map: 7→drop (raise to 2xs), 10→2xs, 11→xs, 12→sm, 13→base, 14→md, 15→md, 16→lg, 17→lg,
18→lg, 20→xl, 22→xl, 26→xl; rem→px equivalent then map. Line-height: 1.45 body, 1.25 headings,
1.2 numeric tiles (set once in `tokens.css`, not per rule). **Gate:** same stylelint allowlist,
`font-size` property.

## Decision 3 — Semantic color tokens

Semantic tokens are **aliases resolved per palette × mode in `themes.css`** — components never
reference raw palette colors or `--success/--warning/--danger` directly for meaning-bearing UI.
Anchors reuse the existing palette values (night-neon dark shown; every palette×mode block defines
all of them — light values reuse the palette's existing darker anchors):

| Token | Meaning | Anchor (nn-dark) |
|---|---|---|
| `--tone-positive` | verdict pass, delivered, success | `#4ade80` (= existing --success) |
| `--tone-caution` | verdict partial, pending, warning | `#fbbf24` (= existing --warning) |
| `--tone-negative` | verdict fail, missed, error | `#fb7185` (= existing --danger) |
| `--tone-neutral` | unavailable, not-assessed, empty | `--muted` |
| `--tone-agent` | AI/agent-derived content (assessments, briefs, digests) | `--accent` |
| `--tone-official` | official-source trust (ESPI/EBI, reports) | `--primary` |
| `--tone-media` | public-media trust | `--secondary` |
| `--tone-user` | user-authored (notes, claims-as-entered) | `--highlight` |

`StatusChip` tone vocabulary maps 1:1 (`ok→positive`, `warn→caution`, `danger→negative`,
`neutral→neutral`, `accent→agent`; add `official/media/user`). Chart/trend colors draw from the
same tokens. **Rules:** (a) **never color-only** — every colored state carries a text label or
icon (chips already do; charts get legends/labels); (b) contrast of token-on-surface pairs is
asserted by the real-browser axe gate (U9) in **both themes**; (c) the light theme joins the
Playwright matrix as one project (`chromium-compact-light`) — full matrix × light is combinatorial
explosion for little marginal signal; recorded here as the deliberate trade-off.

## Decision 4 — One formatting layer (dates, numbers)

`src/shared/format/` becomes the **only** formatting module; `src/shared/formatting/date.ts` and
the split brain it causes are removed (its `replace("T"," ")` fallback is the "oday 09:12" bug).

Dates/times (list contexts):
| Age | Render (pl) | Render (en) |
|---|---|---|
| today | `dziś 09:12` | `today 09:12` |
| yesterday | `wczoraj 17:40` | `yesterday 17:40` |
| <7 days | `wt 18:00` | `Tue 18:00` |
| same year | `2 lip, 09:12` | `Jul 2, 09:12` |
| older | `2 lip 2025` | `Jul 2, 2025` |

Seconds never render in lists (detail panes may show full ISO on demand/tooltip). Detail/audit
contexts (provenance, diagnostics) use full `YYYY-MM-DD HH:MM`. Non-ISO inputs render verbatim
(never string-surgery). Week ranges: `30 cze – 4 lip`.

Numbers: financial values always through `financialValue` — humanized by default
(`1,05 mld zł`, `12,4 mln`, `842 tys.`), full precision in tooltip/detail; Polish locale uses
NBSP thousand separators and comma decimals; percentages one decimal (`14,3%`); all numeric
columns/tiles use `font-variant-numeric: tabular-nums` (one utility class); counts stay plain
integers. **Gate:** a vitest contract test (pattern: `buttonVariantContracts`) fails on imports of
the removed modules and on direct `toLocaleString`/`toISOString`/`Intl.NumberFormat` calls in
`src/screens/**` + `src/shared/components/**` (the format layer is the only sanctioned caller).

## Decision 5 — Feedback policy (undo vs confirm)

| Action class | Treatment |
|---|---|
| Reversible destroy (note, reminder, criterion, watchlist entry, evaluation run…) | immediate + **toast with Cofnij** (restore via existing create/update APIs; `useUndoableDelete`) |
| Irreversible / cascading (company with data, framework with history, backup restore, data clear) | `InlineConfirm` (in-place) or modal for multi-consequence — **never native `window.confirm()`** |
| Long-running (jobs) | existing job-status surfaces; no blocking dialogs |

Toast primitive: bottom-left queue, auto-dismiss 6s (persist on hover), max 3 stacked, `Cofnij`
button, `role="status"`. **Gate:** lint ban on `window.confirm` outside `src/ui/`.

## Decision 6 — Panel density contracts

Every cockpit panel/screen defines behavior at three pane-width tiers — **S <420px, M 420–760px,
L >760px** — via container queries on the panel root (`container-type: inline-size`; the
`.notebook-panel` precedent), **and two height tiers — short <480px, tall ≥480px** (`container-type:
size` where height rules apply): the audit showed full-screen-designed panels colliding or
clipping to header-only strips in default 2×2 cockpit cells (Research rows overlapping the
reminder button; the Events week calendar reduced to day headers). A short panel drops
secondary sections behind expansion and never renders a fixed-height artifact (calendar, matrix)
taller than the pane. The per-panel contract tables live in `docs/ui-authoring.md` (authored in
U0d from audit screenshots; implementers execute them verbatim). Common rules: S stacks all
side-by-side layouts and hides tertiary metadata behind expansion; M allows two columns; L may
add optional context columns; **identity fields (ticker, date) are never truncated — layouts
give them fixed priority over decorative columns**; a cockpit-hosted panel does not repeat the
pane tab's title as an in-panel H1 (compact header). **Gates:** the panel-width matrix test mounts every
panel at S/M/L and asserts the contract's visibility rules + the existing no-hscroll gate; the
all-panels screenshot baseline (Decision 7) pins the rendered result.

## Decision 7 — Visual regression baseline (all panels)

Owner decision: **every panel/screen**, not a curated subset — nothing is left to implementer
taste. Matrix: each panel × S/M/L pane width × dark (night-neon) + one light pass at M. Determinism:
`prefers-reduced-motion` forced, mock-runtime `SAMPLE_NOW` timestamps, `maxDiffPixelRatio: 0.01`.
Baselines live in per-spec `*-snapshots/` directories under `tests/browser/visual/` (Playwright's
default layout; repo-committed PNGs, size watched at review — as delivered in U11 and documented
in engineering-workflow.md; this ADR originally named `tests/browser/__screenshots__/`).
Updating a baseline is a **deliberate act**: the PR/commit description names which screens changed
and why (procedure in engineering-workflow.md). A baseline update without a named reason is a
review rejection.

## Decision 8 — Bilingual template seeds + non-destructive top-up

`TemplateCriterion` carries `{pl, en}` for `label` and `assessment_guidance` (and template
name/description). Seed, `reset_framework_to_template`, and the new **top-up** resolve the locale
from the persisted app-locale setting (default `pl`). Top-up: at startup, an `app_template`-origin
framework with `version == 1` (never user-edited — every edit bumps version) receives missing
template criteria additively; edited frameworks are untouched (preserves the ADR 0046 no-overwrite
rule) — this closes the "new template criteria invisible without destructive reset" gap found
2026-07-05. Criterion ids derive from the localized label at insert time; top-up matches by the
template criterion's stable index in the constant, not by label. **Gate:** storage tests for
seed/reset/top-up in both locales + migration-safety corpus.

## Decision 9 — Interaction affordances

Expandable rows render a rotating chevron (`▸`/`▾`) — disclosure is never ARIA-only. Focus after a
destructive action moves to the next sibling row (shared helper). Everything clickable must look
clickable (cursor + hover state); the audit (U0c) enumerates violations. **Gate:** RTL keyboard
tests per pattern + primitives contract test for the chevron.

## Consequences

- Implementation tasks (U1–U11) execute tables from this ADR; a divergence is a bug, not a choice.
- New gates added by this ADR: stylelint value allowlists (spacing/type), format-layer contract
  test, `window.confirm` ban, panel-width matrix, all-panels screenshot baseline, axe-in-browser.
  Per ADR 0045 each must be repo-clean before it is enabled, and none may be weakened to pass.
- The repo gains its first container-query-based responsive layer and its first visual baseline;
  both documented in ui-authoring.md / engineering-workflow.md as part of the same tasks.

## Resolved at owner sign-off (2026-07-05)

- **Compare stub: hidden from primary nav until v0.53** (the mode returns when market data gives
  it content; an empty mode in nav is trust debt). Task U-Rc.
- **Cockpit company context: view-level selector with per-panel pin override.** A cockpit view
  carries one "view company"; company-scoped panels follow it by default and retarget in place on
  switch (layout preserved, pane titles drop the per-company prefix); a single panel may pin a
  different company (tab menu → pin); saved views persist the context. Task U-Ra.
- **Today: redesigned to journey J1 as a single prioritized "what changed" stream** (full ticker +
  type badge + full date + one action per row, j/k navigation) plus a narrow counters column
  (autopilot / to-verify / upcoming reports); the quiet state is the goal. J1's interaction budget
  is the acceptance bar. Task U-Rb.
