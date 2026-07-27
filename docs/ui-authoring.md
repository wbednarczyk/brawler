# Brawler UI Authoring Guide

This is the canonical, **agent-facing** guide for building or editing any Brawler frontend UI. It exists because the recurring cause of incoherent views is hand-rolling markup that a shared primitive already provides. Read it before writing or editing components, screens, or styles.

Doc map: [CLAUDE.md](../CLAUDE.md) § Required Reading. Related: [Modularization Design](modularization-design.md) (where things live), [UI Flows](ui-flows.md) and [UI Information Architecture](ui-information-architecture.md) (UX/IA, not component authoring), and [ADR 0037](adr/0037-ui-component-framework-and-authoring-contract.md) (the policy decision).

## The one rule

**Compose from `src/ui` primitives. Do not hand-roll a control, section, badge, row, or layout that a primitive already provides.** `src/ui/index.ts` is the source of truth for what exists. If a primitive is missing for a genuinely recurring shape, add it to `src/ui` (or `src/shared/components` for domain-level reuse) and document it here — do not inline a bespoke version in a screen.

**See the primitives rendered:** `src/ui/PrimitiveGallery.tsx` is a live catalog of every common primitive and its variants. View it with `npm run dev:vite` then open `/gallery.html` (a dev-only entry — it is never shipped). The gallery is also the surface the `jest-axe` accessibility test and the Playwright overflow check run against, so adding a primitive there gives it coverage for free. Keep it in sync when you add or change a primitive.

## Mockup-first and no-spec-no-design (v0.50 U12, ADR 0045 harvest)

Two process rules for anything beyond a mechanical change:

- **Mockup-first.** A new panel/screen or a functional redesign starts as an HTML mockup the owner approves BEFORE code, and the approved mockup is **saved under `docs/mockups/`** (gitignored, local-only — owner decision 2026-07-15) — never left in a session scratchpad (the v0.50 U0 mockups were approved but lost with the scratchpad; the ADR prose had to stand in for them).
- **No design decision without a spec.** Implementers (human or agent) execute normative specs — ADR tables, density-contract rows, approved mockups. If a spec is missing, ambiguous, or two rules conflict, STOP and escalate with the conflict spelled out; do not pick a design silently. Harvested from v0.50: every fan-out defect traced back to a gap or contradiction in the task spec, not to implementer judgment being too weak.

## Experience contracts, storyboards & discoverability (ADR 0081, pilot-gated)

Canonical home for the UX decision-validation loop's authoring rules ([ADR 0081](adr/0081-ux-quality-loop-v2.md)). Non-mechanical UI work — a new panel/screen, functional redesign, changed cross-screen journey, or new primary user decision — is specified with an **experience contract** (trigger, outcome, decision, evidence, hierarchy, one primary action, entry/exit/recovery, done-well, storyboard, state matrix, journey mapping, first red journey test) **before** component work. The textual contract lives in the feature plan; the approved storyboard under `docs/mockups/`. Copy/token-only fixes, primitive-preserving migrations, and exact regression repairs are exempt unless they change a journey. Reusable templates + a worked example land with the pilot (plan Q1); enforcement is pilot-gated and becomes universal only after the J1/J2 verdict.

**Templates.** Copy [`docs/plans/EXPERIENCE-CONTRACT-TEMPLATE.md`](plans/EXPERIENCE-CONTRACT-TEMPLATE.md)'s 11 sections into the task's plan section for the textual contract, and [`docs/mockups/STORYBOARD-TEMPLATE.html`](mockups/STORYBOARD-TEMPLATE.html)'s 7 frames (entry, before action, loading/in-flight, success, error, undo/recovery, narrow pane) for the visual storyboard — no runtime dependency, opens standalone. Both are reusable sources, not approvals: a copied template is a draft.

**Trigger.** Any non-mechanical UI change per the definition above; see [ADR 0081](adr/0081-ux-quality-loop-v2.md) decision 1 for the exact boundary.

**Exemption.** Copy/token-only fixes, primitive-preserving mechanical migrations, and exact regression repairs — unless the change alters a journey, in which case the contract applies.

**Storage.** The filled-in textual contract lives in the feature plan (`docs/plans/`); the filled-in storyboard is saved under `docs/mockups/`. Both directories are **gitignored working artifacts** (owner decision 2026-07-15) but must live there, never only in a session scratchpad or `test-results/` — that is how the v0.50 U0 mockups were lost (§ Mockup-first above).

**Approval flow.** Draft both from the templates → owner reviews the completed contract + storyboard together → owner approval makes them normative for the task; component work does not start on an unapproved copy. The worked J1 current-state example in [`docs/plans/ux-quality-loop-v2.md`](plans/ux-quality-loop-v2.md) § Pilot appendix shows every field filled — it documents current intent, not redesign approval.

### Discoverability & interaction-hierarchy contracts (ADR 0081 Q4)

Scoped, explicit contracts for a decision surface — never a whole-page CTA scan and never a heuristic guess at which information "looks important" on a multi-pane workspace.

- **`Button` emits `data-ui-button-variant={variant}`** on the rendered element — a stable hook for scoped test helpers (e.g. finding primitive icon buttons), never a styling hook.
- **The primary action is explicit, not inferred.** A surface's experience-contract primary action (§ 6 of the template) is marked by the *caller* with `data-ux-primary-action="true"` on the `Button`. It is never derived from `variant="primary"` or any CSS class — a screen can use the primary variant for emphasis without it being *the* contracted primary action, and vice versa.
- **Icon-only buttons** keep the existing accessible-name requirement (axe is the general authority, `expectNoA11yViolations`). A **non-obvious** icon action additionally needs a visible explanation or a `title` — a human call at review time, not something a helper infers.
- **Helpers** (`tests/browser/helpers/interactionContracts.ts`): `expectPrimaryActionCount(surface, { max, reason? })`, `expectActionBeforeScroll(action, scrollOwner)`, `expectFocusOrder(page, locators)`, `expectNamedIconActions(surface)`, `expectNextStepVisible(locator)`. Every helper takes an explicit surface/action locator supplied by the caller — never scans the whole page.
- **Multi-primary exemption.** `expectPrimaryActionCount` with `max > 1` requires a non-empty `reason`, which must pair with a matching entry in that surface's experience contract (template § 6). Without a `reason`, the surface's primary-action count must match `max` exactly (so a dropped marker reddens as loudly as a doubled one).
- **Pilot adoption:** J1 (`tests/browser/journeys/j1-morning-review.spec.ts`) and J2 (`j2-company-published-a-report.spec.ts`) mark their real primary actions (Today row "Review"; J2 marks the Notebook "New note" since `v0.59.0`, the KPI-extraction launcher having been retired with the in-app AI layer — [ADR 0084](adr/0084-retire-in-app-ai-layer.md)) and assert the contract against them.

## Pre-write self-check (do this every time)

Before writing JSX for a piece of UI, ask:

1. **Have I opened the closest sibling screen and matched its scaffold?** This is step zero, not optional — building a screen from the catalog in the abstract is how views end up structurally "raw". Open an existing screen in the same shape (a full screen → `src/screens/Events/EventsScreen.tsx`; a panel → a `src/screens/Research/*Panel.tsx`) and copy its outer structure. See [Screen scaffold](#screen-scaffold).
2. **Is there a primitive for this?** Skim `src/ui/index.ts`. A titled section, a labelled input, a status badge, an error line, a list row, a modal, an empty state, a chart — there already is one.
3. **Is there a domain component for this data shape?** Skim `src/shared/components/`. A **qualified ticker → `TickerLabel`** (never render `qualifiedTicker`/`company` as plain text — it loses the exchange coloring every other screen shows), markdown note body → `MarkdownNoteBody`, a quarter/date field → `NotebookQuarterField`/`NotebookDateField`, etc. These live **outside** `src/ui` so they will not show up when you only skim the primitive barrel. See [Domain components](#domain-components).
4. **Am I about to write a raw `<input>`, `<select>`, `<textarea>`, a `*-header`/`*-toolbar` div, or `style={{…}}`?** That is almost always a primitive bypass. Stop and use the primitive.
5. **Is this shape new but recurring** (used or about to be used in ≥2 places)? Add a primitive, don't copy markup.
6. **Strings**: every user-visible string goes through `text("…")` (see [i18n](#i18n)) with an entry in **both** `en.ts` and `pl.ts`.

## Screen scaffold

A full-screen view has a fixed outer structure. Copy it from a sibling (e.g. `EventsScreen`) rather than re-deriving it — getting this wrong is what makes a view feel raw next to the others (headers flush to the border, no internal scroll, double-wrapped chrome):

```tsx
<section className="feed-panel" aria-labelledby="x-title">
  <PanelHeader title={…} description={…} titleId="x-title" actions={…} />
  {/* The padded, independently-scrolling body. Without it, content sits flush
     against the panel border. Match the .events-layout / .companies-layout idiom. */}
  <div className="x-layout">
    {/* sections, rows, empty states … */}
  </div>
</section>
```

Rules:

- The screen root is `feed-panel` (the panel chrome: border, radius, `overflow: hidden`, full height). Do **not** also wrap the body in the `Panel` primitive — that double-wraps the chrome. `Panel`/`PanelHeader` is for nested sub-panels inside a screen, not the screen shell.
- Put screen content in a **padded, `overflow:auto`, `flex:1; min-height:0`** body container (the `.events-layout`/`.companies-layout` pattern). The panel itself has no body padding by design.
- Rows that read as cards use the shared card idiom: a `var(--border)`-ish border, `border-radius: 8px`, and a `color-mix(... var(--surface-raised) …)` fill (see `.event-week-day`, `.report-season-row`). A selectable/expandable row gets a visible hover and active state; use the `ExpandableRow` primitive for expand-in-place — it renders its own rotating disclosure chevron (▸/▾), so consumers must not add one.
- **Focus after a destructive action** never falls back to `<body>` (ADR 0076 D9): when a list row is deleted/hidden, move focus to the next sibling row via the shared `useFocusAfterRemove(itemKeys, { rowSelector, focusSelector? })` hook (`src/shared/focus/focusAfterRemove.ts`). Attach its `listRef` to the list container; it auto-detects the removed row from the keys and only reclaims focus when the departed row (or the list) held it.

## Primitive catalog (what to use instead of hand-rolling)

| You are building… | Use | Do NOT hand-roll |
| --- | --- | --- |
| A titled section with optional count/meta + actions | `SectionHeader` | `<div className="*-toolbar/*-header"><h2/><p>count</p></div>` |
| A card in the feed/detail rail (needs `min-width:0` containment) | `DetailSection` | a `<section>` + manual `min-width:0` plumbing |
| A top-level screen panel with header | `Panel` + `PanelHeader` | a bespoke `*-panel` wrapper |
| A single-line text input | `TextField` | raw `<input>` (styled per-screen) |
| A select | `SelectField` | raw `<select>` |
| A multi-line input | `TextareaField` | raw `<textarea>` |
| A date input | `DateField` (or the `NotebookDateField` domain wrapper) | raw `<input type="date">` (renders unstyled native chrome off the dark design system) |
| A plain checkbox + label | `Checkbox` | `<label><input type="checkbox"/>text</label>` |
| A labelled field row | `FieldRow` | ad-hoc `<label>` grids |
| A search box | `SearchField` — **always pass a styling `className`** (base: `search-box`); the primitive carries structure, not skin, so a bare call site renders an unstyled native input (owner dogfooding, v0.52) — unless a container styles it contextually (e.g. inside `FilterToolbar`) | input + clear button assembled by hand; `<SearchField>` with no `className` outside `FilterToolbar` |
| A clear/reset affordance | `ClearButton` | a bare `<button>×</button>` |
| A button | `Button` (`variant`) | `<button className="compact-button">` |
| A **low-emphasis** tone-carrying status/metadata label (quiet, sits inline) | `StatusChip` | `<span className="…-status">` |
| A **high-emphasis** state badge, or a solid keyword **tag** (bold; tags go in `ChipList`) | `StatusPill` | bespoke tag/status spans |
| An inline error line | `ErrorText` | `<p className="error-text">` |
| Muted helper/hint text | `Hint` | `<p className="*-hint">` |
| A media/list row (icon + truncating title/link + trailing meta/badge/action) | `ListRow` | `<li>` + manual flex + manual truncation |
| A selectable dense row | `DenseRow` | bespoke selectable rows |
| A row that expands in place | `ExpandableRow` | manual open/close row markup |
| Empty state | `EmptyState` | `<p>Nothing here</p>` |
| Key/value metadata block | `InfoGrid` | ad-hoc definition grids |
| Inline confirm (delete etc.) | `InlineConfirm` | bespoke confirm toggles |
| A dialog | `Modal` | a hand-built overlay |
| A full-screen distraction-free surface (reader/writer Focus mode) | `FocusOverlay` | a hand-built full-screen overlay |
| Tabs / segmented views | `SegmentedControl` + `SegmentedControlOption` | bespoke tab bars |
| Sub-navigation | `Subnav` | bespoke nav rows |
| A filter bar | `FilterToolbar` | bespoke filter rows |
| A row of actions | `ActionRow` | a flex div of buttons |
| Transient async-action feedback (saved / refreshed / started / undo) **or** a persistent attention alert | `useToast()` from the app-shell `ToastProvider` — transient by default (`role="status"`, auto-dismiss); pass `persistent` for an attention event (explicit dismiss, `role="alert"`, optional evidence click-through) | a bespoke inline "Saved"/"Done" banner, a per-screen notification region, or a second toast mount |
| Sparkline / trend chart / scaled line chart / candlesticks | `Sparkline` (axis-less inline) / `TrendChart` (per-period bars) / `LineChart` (dense close-only series) / `CandlestickChart` (dense OHLC series) — the scaled charts carry a y-scale + date span; a value the reader must gauge is never plotted scale-less | a charting dependency or hand-drawn SVG |

## Domain components (`src/shared/components`)

`src/ui` holds generic primitives; `src/shared/components` holds **domain-level** shared components that render a specific Brawler data shape consistently across screens. They are **not** in the `src/ui/index.ts` barrel, so skimming the primitive catalog alone will miss them — check this folder for the data you are rendering.

| You are rendering… | Use | Do NOT |
| --- | --- | --- |
| A qualified ticker (`GPW:CDR`) | `TickerLabel` | render `qualifiedTicker`/`company` as plain text (loses the exchange coloring shown everywhere else). Passing it as a prop or building an `aria-label`/`title` string from it is fine; the **visible** label uses `TickerLabel`. A `<select>` `<option>` is the one place it stays plain text (options can't render styled spans). |
| A markdown note/claim body | `MarkdownNoteBody` | a hand-rolled markdown renderer |
| A fiscal quarter / date input | `NotebookQuarterField` / `NotebookDateField` | a bespoke period picker |
| A company claims / report-docs / KPI / backfill panel | the matching `Company*Panel` / `Feed*Panel` | a re-implementation |

## Decision rules where two primitives look similar

- **Chip vs pill (emphasis, not status-vs-tag):** the two badges differ by **visual weight**, and both carry a tone. `StatusChip` is the **quiet, low-emphasis** one (muted text, subtle surface; tones `neutral | accent | ok | warn | danger`) — use it for inline status/metadata that should not dominate the row: signal categories, "Saved", a fetch state. `StatusPill` is the **bold, high-emphasis** one (solid, primary-tinted, weight 700; tones `neutral | ok | warn | danger`) — use it for (a) a prominent process/job **state** that should stand out (transcript/analysis/KPI job status) and (b) solid keyword **tags**, normally inside `ChipList` (note tags, market codes, watchlist names). Pick by how much the badge should draw the eye; they are intentionally kept as two emphasis variants rather than merged.
- **Section containers:** `SectionHeader` for a titled section inside a screen/tab. `DetailSection` for cards in the fixed-width feed/detail rail (it bakes the containment contract so long content can't blow out the pane). `Panel`/`PanelHeader` for a top-level screen panel.
- **Rows:** `ListRow` for non-interactive media rows (a document, an attachment). `DenseRow` for selectable list rows. `ExpandableRow` when the row expands detail in place.
- **Toast vs inline feedback (ADR 0068 T6):** the **success/completion of an async action** (source refresh, import apply, an AI job kicking off) belongs in a **transient toast** (`useToast`), not a per-screen "Saved"/"Done" banner — the shared surface confirms the action even when its result lands off-screen. What stays **inline**: form-adjacent **validation errors** (next to the field), and a **persistent status pill/chip** that reflects ongoing state (a job's running/failed state, a "Protected" marker). Do not raise a toast *and* keep a redundant inline "Saved" line for the same event; a rich result summary the user reviews (e.g. the import result grid) may stay alongside a transient completion toast.

## Configuration is visual-first (maintainer UX preference)

When the user configures a value, dimension, or layout, **prefer direct manipulation and visual presets over bare entry** — this is a standing project preference, apply it everywhere a value is set:

1. **Quick visual presets first** — clickable graphical choices for the common cases (e.g. mini-grid icons `2×2 / 2×3 / 3×3` for a layout, swatches for a color, sized examples for spacing), so the frequent path is one click.
2. **Direct-manipulation + exact input, bidirectionally linked** — a **slider** (or drag handle) for the feel of the range, **two-way bound** to a precise numeric/text field: dragging updates the field, typing updates the slider. Neither is read-only.
3. **Live preview** — reflect the change as it happens (the grid redraws, the value updates), not only on commit.

A bare numeric/text field alone is the **fallback**, for values with no meaningful range or visual. The grid-size picker in the composable-views creator ([ADR 0057](adr/0057-composable-views-and-curated-dashboard.md)) is the reference implementation: presets + linked slider/input + live preview.

**Presets are for small option sets (≤ ~12).** A large taxonomy (dozens of values) rendered as an always-on chip wall is a UX defect, not visual-first: render **type-to-filter suggestions** instead — chips appear only once the typed value narrows the set, capped (~12), case-insensitive substring, nothing suggested on an exact match. The sector field (`CompanySectorField`) is the reference. Harvested 2026-07-14 from the ~90-chip sector-taxonomy wall.

**A free-text settings field must edit a local draft and commit on blur/submit — never call `update_settings` per keystroke.** A controlled `TextField` bound directly to the round-tripped settings object cannot be typed into: the async save means React reverts each keystroke to the last-persisted value, and backend validation rejects every partial value on the way (e.g. a base URL fails the `http(s)://` check at `"h"`, `"ht"`, … — an error per keystroke). Keep a `useState` draft seeded from settings, commit on blur or an explicit save, and only then surface a validation error (the credential-key forms' draft pattern is the reference). Harvested 2026-07-02 from the S6 base-URL/freeform-model fields, which shipped unusable-by-typing.

## Styling rules

- **No inline `style={{…}}`** in screens/components (lint-enforced). Exactly one case is tolerated — the dynamic `--sidebar-width` custom property in `AppShell` (a live px value a static stylesheet cannot express), carrying an inline disable with its reason; do not add more. Containment, truncation, and spacing belong in CSS or are baked into a primitive (e.g. `ListRow` truncates; `DetailSection` contains).
- New screen-specific selectors go under `src/styles/screens/`; cross-screen control/layout styling goes in the matching shared style module (`src/styles/ui.css`, `rows.css`, etc.). Use design tokens (`var(--border)`, `var(--muted)`, `var(--surface)`) — do not hardcode colors.
- Do not invent a new `*-panel` / `*-header` / `*-toolbar` class when a primitive renders that shape.
- **Small text colored `var(--primary)` on a tinted/raised surface fails WCAG AA in BOTH light palettes** (~4.2–4.4:1 vs the required 4.5; the axe gate reddens it) — accent-colored small text (chips, toast actions, bolded preview words) ships with a `:root[data-theme="light"] … { color: var(--primary-strong); }` override; dark themes keep `--primary` (there the lighter tint has the higher contrast). Recurred twice on 2026-07-15 (`.ui-toast-action`, `.alerts-trigger-chip-active`).
- **Repeated trailing elements (status chips, badges, actions) sit in fixed, column-aligned slots down a list — a chip must never migrate horizontally because a neighboring optional control is absent on that row** (owner rule, 2026-07-09, global — applies to every row idiom: `ListRow` trailing, table cells, card footers). Reserve the optional-action slot (fixed-width grid column or invisible placeholder) so the same chip lands at the same x on every row; a right-packed flex row that shifts chips into the freed space fails this rule.
- **Never `stopPropagation` on a whole sub-region of a clickable row/card — scope it to the actual interactive control.** A row that selects on click often nests a "context" block (chips, metadata, a delete button) with `onClick={(e) => e.stopPropagation()}` on the *wrapper* to stop those clicks from also selecting the row. That wrapper becomes a **dead click-zone**: when responsive CSS stacks the row to `flex-direction: column` at narrow widths, the wrapper occupies the row's lower half, so clicking there does nothing (the `.company-row-context` dead-zone bug — selection only worked on `.company-row-main`). Put `stopPropagation` on the specific buttons/links that must not bubble, never on a layout region that can grow to cover the row's click target.
- **Grid/flex containers that hold variable or unbreakable content must let their children shrink, or they force a horizontal scrollbar.** A CSS grid item defaults to `min-width: auto` (= min-content), and an *unbreakable* string — a long ESPI filename like `cyber_Folks_SA_30.06.2023_raport.pdf`, a `nowrap` heading — has a min-content as wide as the whole string. So a content-bearing grid container needs **both** `min-width: 0` **and** `grid-template-columns: minmax(0, 1fr)` (plain `1fr` keeps an implicit min-content floor); only then do `ListRow`/`.ui-list-row-title` truncate instead of blowing the track out. This must hold for **every** ancestor down the chain — one missing link re-introduces the scrollbar (the `v0.47.0` report-diff panel overflowed because `.company-list → .company-row-block → .company-workspace → .company-report-documents` each needed it). Guard a panel that renders such content with a narrow-window Playwright assertion (see [Testing](testing.md) → no-horizontal-scroll).
- **A grid track holding a variable-length chip/badge is `max-content`/`auto`, never a px cap chosen from the shortest label** (ADR 0045 harvest, bug 228762e). A fixed track like `minmax(72px, 92px)` sized for a short severity badge stretches a longer chip to the cap while its `nowrap` label paints out onto the next cell — invisible to a container `scrollWidth` check (the chip element box fits; only its *own* `scrollWidth > clientWidth` reveals it). When a row idiom is reused for a cell whose text length differs (e.g. the Diagnostics reconciliation rows reusing the event-log grid), give that reuse its own scoped `grid-template-columns` with the status track content-sized. Guard with a Playwright assertion on the chip's `scrollWidth ≤ clientWidth` **and** no box intersection with the neighboring cell.
- **Panel-internal horizontal scrollbars are gated (ADR 0045 harvest, v0.50).** `expectNoPageOverflow` (browser harness) fails any element that *actually scrolls horizontally* (computed `overflow-x: auto|scroll` with wider content) anywhere on the page — a dockview pane scrolls internally, so the old document-level check never saw these. The rules that keep it green:
  - **Row slots are single-line.** `ListRow` title *and* meta shrink + ellipsize by design; never put prose-length text (guidance, reasoning, descriptions) in a `meta`/`trailing` slot — prose belongs in a wrapping detail block (`ExpandableRow` detail, `.quality-reasoning`-style paragraph).
  - **Deliberate wide content gets its own bounded scroller marked `data-hscroll`** (the facts matrix, the events week calendar, horizontal card strips, `FilterToolbar`). Pair `overflow-x: auto` with `contain: inline-size` — in a column flex/grid chain a scroller still propagates its content's min-content width upward without it.
  - **A `<select>`'s min-content is its longest option** — `SelectField` is shrinkable (`min-width: 0` + capped select); don't undo it with fixed widths.
  - **Pane-width responsiveness uses container queries, not media queries.** A cockpit pane can be narrow while the window is wide, so `@media` cannot stack a pane's columns — set `container-type: inline-size` on the hosting panel and `@container (max-width: …)` to stack (see `.notebook-panel`/`.notebook-workspace`).

## i18n

Every user-visible string is `text("English text")` from `useLocale()`. The locale resources are typed: add the key to **both** `src/shared/locale/resources/en.ts` and `pl.ts`, or the build fails. See the en/pl maps and [ADR-tracked locale model]. Do not concatenate translated fragments where a single key reads better.

**KPI display names always go through `localizedKpiLabel` (`src/shared/locale/kpiLabels.ts`), never raw `def.label`.** Canonical KPI definitions are seeded with English labels; rendering `def.label` directly ships the English name into a Polish UI (the "Current assets" bug, card 5b2222d). Every place a metric name is shown — picker options, table row/column heads, section titles, chart aria — maps through `localizedKpiLabel(def, locale)` / `localizedKpiLabelForKey(key, locale)`. Guarded by pl-locale render tests (e.g. `CompareScreen.test.tsx` asserts Polish picker + Profil rows).

**Use product language, not implementation terms.** Normal user-facing copy must avoid `SQLite`, `Tauri`, `adapter`, `schema`, `database`, `module`, `collector`, and `local`/`Local` — say what the user gets, not how it's built. Developer-only Diagnostics may use implementation terms (it's gated on Developer mode). Source-provided content/URLs may contain anything, but test samples in normal UI tests should not accidentally include the forbidden terms.

**Backend-composed user-visible strings are forbidden — the backend writes typed codes, the frontend translates.** A persisted or wire-carried field the UI renders (a briefing item's `title`/`detail`, an attention label, a run summary) must hold verbatim source data or a typed code/token, never an English sentence the Rust side composed. The frontend maps the code to localized copy through `text()`, tolerating legacy prose rows verbatim. Precedent: the morning-briefing seam (`briefingItemText.ts` over `compose_briefing`, [ADR 0087](adr/0087-today-attention-home-v2.md) dec. 4). Any new backend-composed user-visible string is a defect of this class.

## Panel density contracts (ADR 0076 Decision 6)

Normative per-panel behavior at pane tiers — width **S** <420px · **M** 420–760px · **L** >760px,
height **short** <480px · **tall** ≥480px — via container queries on the panel root. Global rules
(apply to every panel; the tables list only panel-specific deltas): identity fields (ticker, date)
never truncate; prose wraps at ≤72ch; no in-panel H1 repeating the pane tab title (compact header —
mark the panel's leading `SectionHeader`/`PanelHeader` with `paneLead`; the shared `.cockpit-pane
.ui-pane-lead-header` rule in `ui.css` visually hides that title, keeping it in the accessible tree
via the `.visually-hidden` clip-path pattern so `aria-labelledby`/headings still resolve, and drops
its subtitle — a header whose title is *more specific* than its tab, e.g. adds the ticker, stays
visible and must not carry `paneLead`);
filter toolbars collapse to a single row with a "Filtry" disclosure at S; secondary sections fold
behind expansion when short; fixed-height artifacts (calendar, matrix) scroll inside bounded,
`data-hscroll`-marked wrappers and never exceed the pane. Enforced by the panel-width matrix test
(U7) + all-panels screenshot baseline (U11).

| Panel | S (<420) | M (420–760) | L (>760) | short (<480h) |
|---|---|---|---|---|
| Fundamentals | sections stack; facts matrix scrolls; Autopilot section = one row + expand | matrix + one form column | matrix + forms side-by-side | only matrix + section headers; forms fold |
| Feed (company) | item = badge+title+date, meta folds | + summary line | + detail split-pane | list only, detail on click |
| Claims | list only; composer behind "Dodaj obietnicę" button | list + inline composer | + verdict detail column | queue counts + top 3 due |
| Quality | scorecard chips + criteria list; expression folds into expansion | + expression column | + history side panel | chips + criteria; history folds |
| Report documents | grouped by period; kind label + filename (middle-ellipsis, full in tooltip) + date | + kind/status chips + extract-data action (icon) | extract action gains its label | list only (chips + action hidden) |
| Notebook | single column (list OR detail, toggled) | list + detail stacked | list ∥ detail (existing container query) | list only, editor on select |
| Research | tabs + timeline only; queue/questions/reminders fold to count chips | + review queue strip | + questions/reminders columns | summary counts + timeline |
| Events | list mode forced (no week grid) | week grid in bounded scroller | full week grid | list mode forced |
| Report Season | company rows: name+date+state chip | + prep checklist inline | + pre-report card column | rows only |
| Watchlists | names + counts | + membership editor | + company table | names + counts |
| Transcripts | list; player/segments fold | list + segments | list ∥ segments ∥ note | list only |
| Inbox | list only (detail = overlay/route) | list + detail stacked | list ∥ detail | fewer visible filters |
| Today (post U-Rb) | stream only; counters fold to a top strip | stream + counters column | same, wider stream | stream trimmed to actionable items |
| Sources | source rows + status chip | + schedule/settings inline | + diagnostics column | rows only |
| Settings | one section at a time (tab list collapses to select) | tab list + section | same | n/a (screen) |
| Diagnostics | log list; filters collapse | + module/severity columns | full table | list only |

Compare: live Modes destination (restored v0.61, [ADR 0089](adr/0089-cross-company-comparison-and-valuation-l1.md)). Density behaviour: the selection controls and result sections stack in a column; the comparison table is deliberate wide content that scrolls inside its own `data-hscroll` container (grid/flex chain kept `min-width:0`), so the panel never grows a horizontal scrollbar down to ~960px. Storyboard: `docs/mockups/v061-compare-storyboard.html` (frames 1–7).

### Implementing a contract row

The cross-cutting infrastructure (U7) is in place: `.cockpit-pane` and `.workspace` are named `pane` **size** containers (`container: pane / size`), so a panel's CSS reacts to the *pane's* size, not the window's. To implement a row:

1. **Write container queries against the named `pane` container** at the tier boundaries — `@container pane (max-width: 419px)` (S), `@container pane (max-width: 759px)` (below L), `@container pane (max-height: 479px)` (short). Never re-declare a local `container-type` on the panel root; the pane roots already own it. Values are fixed — do not invent breakpoints.
2. **Assert it in `tests/browser/density-matrix.spec.ts`** — append a `PANEL_CONTRACTS` entry: `open(page)` returns the pane `Locator` to size, and each `tiers` check runs after `setPaneSize` forces that pane to the tier size. Assert visibility/layout via `boundingBox` (e.g. two columns → detail to the right of the list; stacked → detail below, same left edge) and rely on the runner's `expectNoPageOverflow` per tier. The `FilterToolbar` S-tier "Filtry" disclosure and other toggle semantics are unit-tested in `src/ui/primitives.test.tsx` — jsdom has no container queries, so the *tier switch itself* is browser-only.
3. **`setPaneSize`/`resetPaneSize`** (`tests/browser/helpers/harness.ts`) force a pane's inline size directly (the sanctioned approach) so the query fires regardless of the real dock cell. A panel hosted in a multi-pane dashboard is not the first `.cockpit-pane` — return the specific pane (`page.locator(".cockpit-pane", { has: … })`) from `open`.

## When a native element is acceptable

Per [Modularization Design](modularization-design.md), a few natives may stay when a primitive would obscure semantics or accessibility: segmented controls, **toggle switches** (track + thumb, `role="switch"`) and **selectable list rows** that contain a checkbox (use the row primitive, not `Checkbox`), field-clear buttons, collapsible headers, suggestion rows, anchor links, and the inherently-native input types the lint rule exempts (`checkbox`/`radio`/`file`/`date`/`time`/…). For a **plain** checkbox-with-label, prefer `Checkbox`. "It was faster to write raw" is **not** an acceptable reason. When in doubt, use the primitive.

`TextField`, `SelectField`, `TextareaField`, and `Button` **forward refs**, so a control that needs imperative focus/blur (e.g. a registry-lookup form) is no longer a reason to drop to a raw element — pass the `ref` to the primitive.

**`Button` vs raw `<button>`:** use `Button` for a standard action button (it owns the `primary`/`secondary`/`minimal`/`icon`/`danger`/`action`/`ghost` variants). A raw `<button>` is the right choice — and intentionally **not** lint-banned — when the element is a *custom interactive widget* whose styling/semantics the variants don't model: a collapsible section header (`aria-expanded`), a selectable list row, a mode tab, a filter chip, or a picker cell. Reach for `SegmentedControl` for tab/segment groups and `ChipList` for tag groups before hand-rolling those.

## Enforcement

Primitive-first authoring is policy ([ADR 0037](adr/0037-ui-component-framework-and-authoring-contract.md)), not a style preference. Enforcement runs in the `check:frontend` gate (`typecheck → lint → stylelint → test → build`) and is deliberately **non-restrictive** — it catches regressions without blocking legitimate natives or legitimately-English strings:

- **Button variants must have CSS** — `src/styles/buttonVariantContracts.test.ts` fails if any `Button` variant maps to a class with no CSS (a CSS-less variant silently renders as a default grey button).
- **Polish translation ratchet** — `src/shared/locale/translationCompleteness.test.ts` fails when a **new** `text("…")` literal has no `plText` entry. The current backlog of intentionally/temporarily untranslated strings lives in `untranslated-baseline.json`; fix a new failure by translating it (preferred) or, for an intentionally-English string (acronym, brand), adding it to the baseline as a conscious choice. The baseline should shrink, never grow casually.
- **Pluralization** — counts render their noun through `pluralNoun(locale, n, forms)` (`src/shared/locale/plural.ts`); a `n === 1 ? a : b` ternary is wrong for Polish (3 forms).
- **Layout/containment** — `src/styles/layoutContracts.test.ts` and the browser-smoke viewport matrix in `playwright.config.ts`. `expectNoPageOverflow` additionally fails on any *panel-internal* horizontal scrollbar not marked `data-hscroll` (see Styling rules above). Add a sample there when introducing a new layout shape; sample data must include realistic long content (the Kroeze-length guidance seed exists precisely so the gate has something to bite on).
- **Primitive contracts + a11y** — `src/ui/primitives.test.tsx` covers each primitive's render/behavior contract (e.g. `SectionHeader`'s `level` prop renders the right heading, `Checkbox` fires `onChange`), and `src/ui/primitives.a11y.test.tsx` runs `jest-axe` over the whole `PrimitiveGallery` so the library keeps a clean accessibility baseline (this caught a real `aria-selected`-on-`<article>` bug in `DenseRow`). Add the primitive to the gallery + the contract test when you add one. `tests/browser/gallery.spec.ts` additionally checks the gallery for horizontal overflow across the viewport matrix.
- **Barrel imports** — `no-restricted-imports` (in `eslint.config.js`) requires consumers to import primitives from the `…/ui` barrel, never a deep `…/ui/Button` path, so the public surface stays in `src/ui/index.ts`.
- **CSS hygiene (`npm run stylelint`)** — bans hardcoded hex colors outside `tokens.css`/`themes.css` (use design tokens), and flags duplicate selectors, duplicate properties, and empty rules. Runs in the gate.
- **Dead code (`npm run knip`)** — finds unused files, exports, and dependencies. Kept as a periodic audit script rather than a gate step (its native `oxc-parser` binding makes a hard CI gate riskier), but it runs on the same nix Node 22 toolchain as everything else. The export/type surface of `src/api` and `src/ui` is intentionally excluded as public contract/library surface.
- **Raw-control / inline-style / error-line lint** — `eslint.config.js` (`npm run lint`) bans raw `<input>`/`<select>`/`<textarea>`, inline `style={{…}}`, and a raw element with `className="error-text"` (use `ErrorText`, or `ErrorText as="span"` inline) outside `src/ui/` (the `no-restricted-syntax` rule, an **error**). It is escapable so it never blocks a legitimate native: inherently-native `<input>` types (`checkbox`, `radio`, `file`, `date`, `time`, `datetime-local`, `month`, `week`, `range`, `color`) are exempt by the rule itself, and any other genuinely-native control (a ref-bound/keyboard-driven widget, a composite picker, the dynamic `--sidebar-width` style) carries an inline `// eslint-disable-next-line no-restricted-syntax -- <reason>` documenting why. `src/ui/**` and tests are out of scope. The same config also runs the standard `@typescript-eslint` recommended set and `react-hooks` rules as warnings. The initial backlog was driven to **zero** and the lint script runs with `--max-warnings 0`, so **a warning fails the gate** — there is no silent backlog. Fix a new warning, or for an intentional case (a lifecycle effect that must not re-run, a ref read in cleanup) add a reviewed `// eslint-disable-next-line <rule> -- <reason>`.

## Adding a new primitive

1. Confirm the shape recurs (≥2 real uses) and no existing primitive fits.
2. Add it under `src/ui` (generic) or `src/shared/components` (domain-level), export it from the barrel, give it CSS in the right module, and keep class semantics/accessibility.
3. Add a row to the catalog table above and, if it changes policy, note it in [ADR 0037](adr/0037-ui-component-framework-and-authoring-contract.md).
4. Retrofit the call sites that motivated it.
