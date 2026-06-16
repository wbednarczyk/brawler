# ADR 0037: UI Component Framework and Authoring Contract

Status: Accepted

## Context

Brawler has a substantial in-house UI primitive library under `src/ui` (~24 components: `SectionHeader`, `DetailSection`, `Panel`, `TextField`, `SelectField`, `StatusChip`, `StatusPill`, `DenseRow`, `EmptyState`, `InfoGrid`, `Modal`, `SegmentedControl`, charts, …) plus domain-level shared components under `src/shared/components`. The framework is good; **adoption is partial**, and that is what makes views look incoherent.

Measured at the time of this ADR:

- `SectionHeader` adopted in only 4 places, while ~20 distinct ad-hoc `*-header`/`*-toolbar`/`section-heading` classes hand-roll the same titled-section shape.
- `TextField`/`SelectField` adopted in only 6 files, while 10+ screens use raw `<select>` and several use many raw `<input>`; no `TextareaField` exists, so every note/claim body is a raw `<textarea>`.
- `error-text` hand-rolled in ~15 files; no `ErrorText` primitive.
- Media-row shapes (icon + truncating title + trailing meta/badge) reinvented per screen.

The **root cause is a guidance/process gap, not a missing framework**: the component guidance was not discoverable where agents look, not explicit enough, and not enforced. Agents (human and AI) sat down to build UI and did not reach for the primitives — the v0.41.0 report-document and backfill panels are recent examples that bypassed `DetailSection`/`SectionHeader`. Fixing the views without fixing the guidance guarantees regression on the next feature.

## Decision

1. **Primitive-first authoring is policy.** All Brawler UI composes from `src/ui` primitives (or `src/shared/components` for domain-level reuse). Hand-rolling a control, section, badge, row, layout, or modal that a primitive already provides is a defect, not a style choice. `src/ui/index.ts` is the source of truth for what exists.

2. **Close the guidance gap on three layers, all required:**
   - **Discoverable** — [AGENTS.md](../../AGENTS.md) Required Reading and Working Rules now point to the authoring guide and state the primitive-first rule, so it is in context every session.
   - **Explicit** — [docs/ui-authoring.md](../ui-authoring.md) is the canonical authoring guide: the primitive catalog, a "do not hand-roll X → use Y" table, the decision rules for look-alike primitives, the styling/i18n rules, and a **pre-write self-check** agents run before writing JSX.
   - **Enforced** — the `check:frontend` gate (`typecheck → lint → test → build`), designed as non-restrictive guards that catch regressions without blocking legitimate code. Guard tests: `buttonVariantContracts.test.ts` (every Button variant has CSS), `translationCompleteness.test.ts` (no new untranslated `text()` literal, against a shrinking `untranslated-baseline.json`), plus `layoutContracts.test.ts` and the browser-smoke viewport matrix. **ESLint** (`eslint.config.js`, flat config) bans raw `<input>`/`<select>`/`<textarea>` and inline `style={{` outside `src/ui/` via `no-restricted-syntax` (error). After the forms retrofit, the only remaining raw controls are genuine natives, so the rule is escapable rather than restrictive: inherently-native input types (`checkbox`/`radio`/`file`/`date`/`time`/…) are exempt by the rule itself, and the handful of bespoke natives (ref-bound or keyboard-driven widgets, composite pickers, the dynamic `--sidebar-width` style) each carry an inline `// eslint-disable-next-line no-restricted-syntax -- <reason>`. The same config runs `@typescript-eslint` recommended + `react-hooks` as warnings; their backlog was driven to zero and the script runs with `--max-warnings 0`, so the whole lint (ban + standard rules) is a hard gate with no silent backlog. Intentional cases (lifecycle effects that must not re-run, a ref read in cleanup) carry a reviewed `// eslint-disable-next-line <rule> -- <reason>`. We chose ESLint over an equivalent zero-dependency Vitest guard once the conservative-dependency posture was relaxed for tooling that adds real value (editor integration, autofix, standard rule coverage the repo previously lacked).

3. **Look-alike primitives have documented roles, not arbitrary choice.** `StatusChip` and `StatusPill` are kept as two **emphasis variants** rather than merged: `StatusChip` is the quiet, low-emphasis tone-carrying badge (muted surface; tones `neutral | accent | ok | warn | danger`) for inline status/metadata; `StatusPill` is the bold, high-emphasis solid badge (weight 700; tones `neutral | ok | warn | danger`) for a prominent process/job **state** or a solid keyword **tag** (normally in `ChipList`). The badge audit (task `612740d`) confirmed current call sites already follow this emphasis split, so no forced merge — only the doc framing was corrected from the earlier "status-vs-tag" wording to "emphasis". `SectionHeader` titles a section inside a screen (now with an `h2`/`h3`/`h4` `level` prop so nested sections keep a correct document outline); `DetailSection` is the fixed-width rail card (bakes the `min-width:0` containment contract); `Panel` is a top-level screen panel. `ListRow` is a non-interactive media row; `DenseRow` is selectable; `ExpandableRow` expands in place. These are not interchangeable.

4. **Missing primitives are added, not inlined.** When a genuinely recurring shape (≥2 real uses) has no fitting primitive, add one to `src/ui`/`src/shared`, export it, style it in the right CSS module, document it in the authoring guide, and retrofit the motivating call sites. This ADR's epic adds `ListRow`, `ErrorText`, `Hint`, `TextareaField`, and `Checkbox`, and makes `TextField`/`SelectField`/`TextareaField`/`Button` **forward refs** so a control needing imperative focus is no longer a reason to use a raw element (it removed the CompaniesScreen lookup-form natives). A toggle *switch* and a selectable *row* remain bespoke — they are distinct shapes, not plain checkboxes.

5. **No inline styles.** Containment, truncation, and spacing live in CSS or are baked into a primitive. Exactly one tolerated inline-style case exists in the codebase; no new ones.

## Consequences

- New views are coherent by construction because there is one obvious, documented way to build each shape, surfaced where agents already read.
- A deliberate retrofit pass (this ADR's epic) migrates the ad-hoc header/form/badge/row sites; per [modularization-design.md](../modularization-design.md), mass conversion happens in that pass, not opportunistically during unrelated feature work.
- Enforcement makes the policy mechanical: a bypass fails a check rather than relying on memory or review diligence.
- Intentional natives remain allowed where a primitive would obscure semantics/accessibility (segmented controls, row selectors, field-clear buttons, collapsible headers, suggestion rows, anchor links), as already noted in modularization-design.md. "Faster to write raw" is not a justification.
- Related: this complements the modularization design (where code lives) by governing how UI is composed; the UX/IA docs (ui-flows, ui-information-architecture) continue to own what screens do, not how components are authored.
