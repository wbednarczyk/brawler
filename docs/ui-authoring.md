# Brawler UI Authoring Guide

This is the canonical, **agent-facing** guide for building or editing any Brawler frontend UI. It exists because the recurring cause of incoherent views is hand-rolling markup that a shared primitive already provides. Read it before writing or editing components, screens, or styles.

Use [Project Brief](project-brief.md) for the full documentation map. Related: [Modularization Design](modularization-design.md) (where things live), [UI Flows](ui-flows.md) and [UI Information Architecture](ui-information-architecture.md) (UX/IA, not component authoring), and [ADR 0037](adr/0037-ui-component-framework-and-authoring-contract.md) (the policy decision).

## The one rule

**Compose from `src/ui` primitives. Do not hand-roll a control, section, badge, row, or layout that a primitive already provides.** `src/ui/index.ts` is the source of truth for what exists. If a primitive is missing for a genuinely recurring shape, add it to `src/ui` (or `src/shared/components` for domain-level reuse) and document it here — do not inline a bespoke version in a screen.

**See the primitives rendered:** `src/ui/PrimitiveGallery.tsx` is a live catalog of every common primitive and its variants. View it with `npm run dev:vite` then open `/gallery.html` (a dev-only entry — it is never shipped). The gallery is also the surface the `jest-axe` accessibility test and the Playwright overflow check run against, so adding a primitive there gives it coverage for free. Keep it in sync when you add or change a primitive.

## Pre-write self-check (do this every time)

Before writing JSX for a piece of UI, ask:

1. **Is there a primitive for this?** Skim `src/ui/index.ts`. A titled section, a labelled input, a status badge, an error line, a list row, a modal, an empty state, a chart — there already is one.
2. **Am I about to write a raw `<input>`, `<select>`, `<textarea>`, a `*-header`/`*-toolbar` div, or `style={{…}}`?** That is almost always a primitive bypass. Stop and use the primitive.
3. **Is this shape new but recurring** (used or about to be used in ≥2 places)? Add a primitive, don't copy markup.
4. **Strings**: every user-visible string goes through `text("…")` (see [i18n](#i18n)) with an entry in **both** `en.ts` and `pl.ts`.

## Primitive catalog (what to use instead of hand-rolling)

| You are building… | Use | Do NOT hand-roll |
| --- | --- | --- |
| A titled section with optional count/meta + actions | `SectionHeader` | `<div className="*-toolbar/*-header"><h2/><p>count</p></div>` |
| A card in the feed/detail rail (needs `min-width:0` containment) | `DetailSection` | a `<section>` + manual `min-width:0` plumbing |
| A top-level screen panel with header | `Panel` + `PanelHeader` | a bespoke `*-panel` wrapper |
| A single-line text input | `TextField` | raw `<input>` (styled per-screen) |
| A select | `SelectField` | raw `<select>` |
| A multi-line input | `TextareaField` | raw `<textarea>` |
| A plain checkbox + label | `Checkbox` | `<label><input type="checkbox"/>text</label>` |
| A labelled field row | `FieldRow` | ad-hoc `<label>` grids |
| A search box | `SearchField` | input + clear button assembled by hand |
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
| Tabs / segmented views | `SegmentedControl` + `SegmentedControlOption` | bespoke tab bars |
| Sub-navigation | `Subnav` | bespoke nav rows |
| A filter bar | `FilterToolbar` | bespoke filter rows |
| A row of actions | `ActionRow` | a flex div of buttons |
| Sparkline / trend chart | `Sparkline` / `TrendChart` | a charting dependency or hand-drawn SVG |

## Decision rules where two primitives look similar

- **Chip vs pill (emphasis, not status-vs-tag):** the two badges differ by **visual weight**, and both carry a tone. `StatusChip` is the **quiet, low-emphasis** one (muted text, subtle surface; tones `neutral | accent | ok | warn | danger`) — use it for inline status/metadata that should not dominate the row: signal categories, "Saved", a fetch state. `StatusPill` is the **bold, high-emphasis** one (solid, primary-tinted, weight 700; tones `neutral | ok | warn | danger`) — use it for (a) a prominent process/job **state** that should stand out (transcript/analysis/KPI job status) and (b) solid keyword **tags**, normally inside `ChipList` (note tags, market codes, watchlist names). Pick by how much the badge should draw the eye; they are intentionally kept as two emphasis variants rather than merged.
- **Section containers:** `SectionHeader` for a titled section inside a screen/tab. `DetailSection` for cards in the fixed-width feed/detail rail (it bakes the containment contract so long content can't blow out the pane). `Panel`/`PanelHeader` for a top-level screen panel.
- **Rows:** `ListRow` for non-interactive media rows (a document, an attachment). `DenseRow` for selectable list rows. `ExpandableRow` when the row expands detail in place.

## Styling rules

- **No inline `style={{…}}`** in screens/components (lint-enforced). Exactly one case is tolerated — the dynamic `--sidebar-width` custom property in `AppShell` (a live px value a static stylesheet cannot express), carrying an inline disable with its reason; do not add more. Containment, truncation, and spacing belong in CSS or are baked into a primitive (e.g. `ListRow` truncates; `DetailSection` contains).
- New screen-specific selectors go under `src/styles/screens/`; cross-screen control/layout styling goes in the matching shared style module (`src/styles/ui.css`, `rows.css`, etc.). Use design tokens (`var(--border)`, `var(--muted)`, `var(--surface)`) — do not hardcode colors.
- Do not invent a new `*-panel` / `*-header` / `*-toolbar` class when a primitive renders that shape.

## i18n

Every user-visible string is `text("English text")` from `useLocale()`. The locale resources are typed: add the key to **both** `src/shared/locale/resources/en.ts` and `pl.ts`, or the build fails. See the en/pl maps and [ADR-tracked locale model]. Do not concatenate translated fragments where a single key reads better.

## When a native element is acceptable

Per [Modularization Design](modularization-design.md), a few natives may stay when a primitive would obscure semantics or accessibility: segmented controls, **toggle switches** (track + thumb, `role="switch"`) and **selectable list rows** that contain a checkbox (use the row primitive, not `Checkbox`), field-clear buttons, collapsible headers, suggestion rows, anchor links, and the inherently-native input types the lint rule exempts (`checkbox`/`radio`/`file`/`date`/`time`/…). For a **plain** checkbox-with-label, prefer `Checkbox`. "It was faster to write raw" is **not** an acceptable reason. When in doubt, use the primitive.

`TextField`, `SelectField`, `TextareaField`, and `Button` **forward refs**, so a control that needs imperative focus/blur (e.g. a registry-lookup form) is no longer a reason to drop to a raw element — pass the `ref` to the primitive.

**`Button` vs raw `<button>`:** use `Button` for a standard action button (it owns the `primary`/`secondary`/`minimal`/`icon`/`danger`/`action`/`ghost` variants). A raw `<button>` is the right choice — and intentionally **not** lint-banned — when the element is a *custom interactive widget* whose styling/semantics the variants don't model: a collapsible section header (`aria-expanded`), a selectable list row, a mode tab, a filter chip, or a picker cell. Reach for `SegmentedControl` for tab/segment groups and `ChipList` for tag groups before hand-rolling those.

## Enforcement

Primitive-first authoring is policy ([ADR 0037](adr/0037-ui-component-framework-and-authoring-contract.md)), not a style preference. Enforcement runs in the `check:frontend` gate (`typecheck → lint → stylelint → test → build`) and is deliberately **non-restrictive** — it catches regressions without blocking legitimate natives or legitimately-English strings:

- **Button variants must have CSS** — `src/styles/buttonVariantContracts.test.ts` fails if any `Button` variant maps to a class with no CSS (a CSS-less variant silently renders as a default grey button).
- **Polish translation ratchet** — `src/shared/locale/translationCompleteness.test.ts` fails when a **new** `text("…")` literal has no `plText` entry. The current backlog of intentionally/temporarily untranslated strings lives in `untranslated-baseline.json`; fix a new failure by translating it (preferred) or, for an intentionally-English string (acronym, brand), adding it to the baseline as a conscious choice. The baseline should shrink, never grow casually.
- **Pluralization** — counts render their noun through `pluralNoun(locale, n, forms)` (`src/shared/locale/plural.ts`); a `n === 1 ? a : b` ternary is wrong for Polish (3 forms).
- **Layout/containment** — `src/styles/layoutContracts.test.ts` and the browser-smoke viewport matrix in `playwright.config.ts`. Add a sample there when introducing a new layout shape.
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
