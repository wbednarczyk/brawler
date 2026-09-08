Verdict: **DO-NOT-SHIP** until two contract blockers are resolved: period visibility is still not guaranteed, and the shared Escape model conflicts with the palette’s existing one-Escape modal behavior. The rewrite fixes 8 of the 12 R1 findings; 4 remain partial.

## R1 disposition

1. **FIXED — sticky bleed.** The plan now requires reproduction before CSS changes, covers both matrices/themes/tiers, and combines hit-testing, computed backgrounds, and compositor screenshots ([plan:15](</home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:15>)).

2. **PARTIAL — Escape ownership.** Native-only selector cleanup is correct, but “open list → close” conflicts with the palette’s existing one-Escape modal close, and `preventDefault()` alone does not stop the document-level modal listener ([plan:13](</home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:13>), [Modal.tsx:82](/home/wojtas/projects/brawler/src/ui/Modal.tsx:82)).

3. **FIXED — primitive extraction seam.** The controller/field now has two real consumers, with Global Search explicitly recorded as a follow-up ([plan:13](</home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:13>)).

4. **PARTIAL — visible-period contract.** Resize observation, corner placement, host-specific limits, keyboard coverage, and no mount autoscroll are specified, but hard-coded M/L=8 and Pozycje=8 do not ensure the newest column is actually in the viewport ([plan:16](</home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:16>)).

5. **FIXED — stored report summaries.** Suppression is exact-literal-only and the plan explicitly protects meaningful report and draft summaries with non-vacuous cases ([plan:19](</home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:19>)).

6. **FIXED — scrollbar gutter scope.** The change is now Activity-only, removes the nested scroll owner first, and has a forced-overflow geometry guard ([plan:20](</home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:20>)).

7. **PARTIAL — persistent deep-link selection.** Persistence, retargeting, heading focus, and `aria-current` are specified, but Claims can render the same claim twice and the plan does not define which copy owns the singular current state ([plan:21](</home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:21>)).

8. **FIXED — thread/ticket a11y.** The plan now has exactly one control, one explicit accessible name, a static visual twin, and the required ADR 0104 wording ([plan:11](</home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:11>)).

9. **FIXED — centered picker proof.** The plan adds a numeric M/L centering invariant plus S overflow coverage and an ADR 0107 amendment ([plan:12](</home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:12>)).

10. **FIXED — ADR 0056 scope.** The amendment now distinguishes the retired Fundamentals editor from IR URL and sector override, including the one-company settings path and dependent docs/tests ([plan:14](</home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:14>)).

11. **FIXED — presentation mapping.** A host-neutral exhaustive mapping covers all presentation kinds, including `redFlag`, across detail and both row hosts in EN/PL ([plan:18](</home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:18>)).

12. **PARTIAL — slicing/baselines.** Shared tails have a clear owner and an explicit baseline list, but the #6 isolated-commit gate is not reflected in S1’s mixed slice, and the palette migration has no visual baseline ([plan:42-48](</home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:42>)).

## R2 findings

1. **BLOCKER — `ResizeObserver` does not make eight columns fit.**

The plan simultaneously fixes M/L at eight, says eight is only an upper bound, and promises no horizontal scrolling to see the newest period ([plan:16](</home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:16>), [plan:30-37](</home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:30>)). Those are incompatible unless measurements prove the fit. The table uses nowrap cells and remains an intrinsic-width scroller ([companies.css:436-464](/home/wojtas/projects/brawler/src/styles/screens/companies.css:436)).

Pozycje is worse: one period expands into value plus QoQ/YoY columns ([FundamentalsPeriodsSection.tsx:203](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPeriodsSection.tsx:203)). It already selects the newest eight ([FundamentalsPeriodsSection.tsx:105](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPeriodsSection.tsx:105)), yet the newest is still at the far-right end of the scroller. Keeping eight and forbidding mount autoscroll preserves that defect.

Concrete fix:

- Derive capacity continuously from the observed inline width, with host-specific per-period widths; treat 8 as `max`, not the M/L result.
- Give Pozycje its own smaller measured capacity or explicitly authorize an initial right-edge position for that host.
- Make the browser contract geometric: before interaction, the newest header’s right edge must be within the scroller’s client box at S/M/L.
- Test width transitions across tier boundaries, zero-width/remounted panes, observer cleanup, and that resize does not yank an expanded user who has scrolled into history.

2. **BLOCKER — the rewritten Escape contract would either require two Escapes or close the palette while clearing.**

The palette currently keeps its listbox open for the life of the modal and one Escape closes the palette and restores focus ([CommandPalette.tsx:62](/home/wojtas/projects/brawler/src/shared/components/CommandPalette.tsx:62), [commandPalette.test.tsx:226](/home/wojtas/projects/brawler/src/app/commandPalette.test.tsx:226)). Under the plan’s generic rule, the first Escape closes only the list.

There is also propagation leakage: current `SearchField` consumes a clear with both `preventDefault` and `stopPropagation` ([SearchField.tsx:45](/home/wojtas/projects/brawler/src/ui/SearchField.tsx:45)). `Modal` closes on any document Escape without checking `defaultPrevented` ([Modal.tsx:82](/home/wojtas/projects/brawler/src/ui/Modal.tsx:82)). A controller that merely prevents default can clear the query and close the modal together.

Concrete fix:

- Give the controller a host Escape policy/callback.
- Picker: open → close popup; closed/non-empty → clear; closed/empty → bubble to tool frame.
- Palette: Escape always closes the modal in one press and restores the invoker; it never enters a closed-popup-but-open-modal state.
- Make `Modal` ignore `event.defaultPrevented`, and specify whether consuming composites also stop propagation.
- Add a `Modal.test` proving consumed descendant Escape stays open and unconsumed Escape closes.
- Removing `[role=combobox]` and `[role=listbox]` from `NATIVE_PICKER_SELECTOR` is otherwise correct; the tool frame already honors `defaultPrevented` ([toolRegistry.tsx:96](/home/wojtas/projects/brawler/src/screens/Spolka/toolRegistry.tsx:96)).

3. **Claims needs a canonical current row, not merely a current claim ID.**

The same claim appears in the main list and review queue, and both currently receive the highlight class ([CompanyClaimsPanel.tsx:285](/home/wojtas/projects/brawler/src/shared/components/CompanyClaimsPanel.tsx:285), [CompanyClaimsPanel.tsx:329](/home/wojtas/projects/brawler/src/shared/components/CompanyClaimsPanel.tsx:329)). Browser journeys intentionally tolerate both visual twins ([j2-company-published-a-report.spec.ts:227](/home/wojtas/projects/brawler/tests/browser/journeys/j2-company-published-a-report.spec.ts:227)).

Putting `aria-current="true"` on both would undermine the promised singular target. Define the main-list row as the canonical deep-link target: it receives `aria-current`, scroll destination, and persistent selection; a queue twin may receive a separate visual-match attribute but not current state.

No current Claims test relies on the four-second clear. The unit test only checks the immediate highlight and scroll ([CompanyClaimsPanel.test.tsx:224](/home/wojtas/projects/brawler/src/shared/components/CompanyClaimsPanel.test.tsx:224)); the timer exists only in production code ([CompanyClaimsPanel.tsx:121](/home/wojtas/projects/brawler/src/shared/components/CompanyClaimsPanel.tsx:121)). Therefore persistence will not redden existing tests. The new `>4 s`, retarget, and reopen-without-target tests are essential, as is removing stale “flash” terminology.

4. **The corner-cell control is viable, but its naming contract is incomplete.**

A button inside the existing `<th>` avoids an extra column and remains keyboard reachable. The risk is that its descendant text becomes part of the column header’s accessible name. The existing headers are simply “KPI” and “Position” ([FundamentalsPanel.tsx:581](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPanel.tsx:581), [FundamentalsPeriodsSection.tsx:203](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPeriodsSection.tsx:203)).

Concrete fix:

- Name the header explicitly through `aria-labelledby` pointing only to “KPI”/“Position”.
- Give the button an action name such as `Pokaż 4 wcześniejsze okresy`, not merely `+4 wcześniej`.
- Expanded state must say `Ukryj wcześniejsze okresy`/`Pokaż mniej`, not `+0 wcześniej`.
- Specify `ActionButton kind="control"`; `kind` belongs to `ActionButton`, not `Button` ([ActionButton.tsx:14](/home/wojtas/projects/brawler/src/ui/ActionButton.tsx:14)).
- Test exact column-header name, exact button name, Enter/Space, focus-visible, and expanded/collapsed labels in both locales.

5. **Palette migration is a reasonable blast radius only if the hook is headless and its DOM contract stays stable.**

The palette is the right second consumer: it already contains the keyboard/listbox logic the hook is intended to centralize ([CommandPalette.tsx:41](/home/wojtas/projects/brawler/src/shared/components/CommandPalette.tsx:41)). Migrating it avoids a gallery-only primitive and exercises the abstraction against two different hosts. Keep command execution, modal lifecycle, focus fallback, empty state, and palette CSS outside the primitive.

Expected reddening:

- `commandPalette.test.tsx:149-238` will catch lost ARIA wiring, unstable option IDs, active-descendant navigation, Enter/click execution, empty results, and one-Escape focus restoration.
- `src/App.test.tsx:537-564` will catch global shortcuts swallowed by the new editable picker.
- `src/App.test.tsx:617-677` will catch pointer-focus and Shift+J/K focus-intent regressions.
- `src/App.test.tsx:680-758` will catch lost combobox/option roles, filtering, ArrowDown/Enter execution, modal close, and F3c heading fallback.
- `paletteCopy.test.ts:22-95` will not redden for this migration: it exercises command producers and labels directly, never `CommandPalette`. It should not be cited as migration coverage.

Add active-index reset/clamping after filtering and dynamic command removal, pointer-hover selection, stable IDs across filtering, and explicit palette-vs-picker Escape policy tests.

6. **The slicing is workable, but the review and baseline gates need two corrections.**

One PR remains reviewable because the owner explicitly chose it, provided #6 is a genuine stop/go commit. The current S1 assignment mixes #4–#7 while the header promises #6 as an isolated commit ([plan:3](</home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:3>), [plan:45](</home/wojtas/.claude/plans/sparkling-sniffing-pizza.md:45>)). Require S1 to produce separate commits and run the #6 storyboard, journey, and visual gate before later Fundamentals changes are stacked.

The manifest covers most changed screens, but there is no Command Palette screen in the visual catalog ([catalog.core.mjs:18](/home/wojtas/projects/brawler/tests/browser/visual/catalog.core.mjs:18)). Existing visual tests use the palette only as navigation and close it before screenshots ([visual-companies.spec.ts:15](/home/wojtas/projects/brawler/tests/browser/visual/visual-companies.spec.ts:15)). Add an open-palette M dark/light baseline or an equivalent targeted screenshot. Also replace “default baselines unchanged” with “no default scroll-position mutation”: #4, #6, and #7 necessarily change Fundamentals pixels.

After those two blockers and four plan clarifications are folded, this can move to **SHIP-WITH-FIXES** without splitting the owner’s one-PR wave.

Codex session ID: 01a06d22-21f7-7790-9c2f-fe4545e96a7a
Resume in Codex: codex resume 01a06d22-21f7-7790-9c2f-fe4545e96a7a
