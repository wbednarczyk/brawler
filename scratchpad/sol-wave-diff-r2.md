Reviewed current head `c6131b2685f3f7e604c3add13fd2673dc5e012a9` against `7964f1d5`. No files were changed.

## Round-1 finding status

| # | Status | Evidence |
|---|---|---|
| 1 | **FIXED** | Closed `Enter`/`Home`/`End` return before consumption or selection in [useComboboxListbox.ts:90](/home/wojtas/projects/brawler/src/ui/useComboboxListbox.ts:90); `ComboboxField` blurs only after consumed Enter in [ComboboxField.tsx:84](/home/wojtas/projects/brawler/src/ui/ComboboxField.tsx:84). The unit test at [useComboboxListbox.test.tsx:42](/home/wojtas/projects/brawler/src/ui/useComboboxListbox.test.tsx:42) is sufficient together with the Shift+J integration at [App.test.tsx:629](/home/wojtas/projects/brawler/src/App.test.tsx:629), because focus itself does not open the list. Minor test weakness: its title names Home, but it only dispatches Enter and End. Add an explicit Home event and `aria-expanded=false` assertion. |
| 2 | **NOT-FIXED — blocker** | Natural ink measurement was introduced, but it only measures the currently rendered slice. It is neither complete nor guaranteed to converge. See Finding 1 below. |
| 3 | **FIXED** | A target present in `rows` but absent from `filteredRows` resets both filters at [CompanyReportDocumentsPanel.tsx:331](/home/wojtas/projects/brawler/src/shared/components/CompanyReportDocumentsPanel.tsx:331); retarget coverage is at [CompanyReportDocumentsPanel.test.tsx:719](/home/wojtas/projects/brawler/src/shared/components/CompanyReportDocumentsPanel.test.tsx:719). The subsequent effects expand and then scroll without a state loop. |
| 4 | **FIXED** | Both tables now have real `scope="col"` expander headers: [FundamentalsFactsMatrix.tsx:73](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsFactsMatrix.tsx:73) and [FundamentalsPeriodsSection.tsx:250](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPeriodsSection.tsx:250). `rowSpan` alignment is now semantically valid. A one-row Chromium case would usefully pin the intentional 120px floor, but is not required to establish the semantic fix. |
| 5 | **FIXED, with compatibility caveat** | Hosted Fundamentals no longer scrolls independently at [companies.css:326](/home/wojtas/projects/brawler/src/styles/screens/companies.css:326); collapsed wrappers are clip/visible and expanded wrappers become bounded scroll boxes at [companies.css:443](/home/wojtas/projects/brawler/src/styles/screens/companies.css:443). The browser guard checks computed `clip`/`visible`, sticky pinning, and expanded bounding at [fundamentals-sticky.spec.ts:76](/home/wojtas/projects/brawler/tests/browser/fundamentals-sticky.spec.ts:76). See Finding 3 for the `cqh` fallback. |
| 6 | **FIXED** | `.fundamentals-periods-table .fundamentals-periods-focus` has specificity 0-2-0 versus zebra’s 0-1-0 at [companies.css:893](/home/wojtas/projects/brawler/src/styles/screens/companies.css:893) and [utilities.css:32](/home/wojtas/projects/brawler/src/styles/utilities.css:32). |
| 7 | **FIXED** | `redFlag` gets the shared chip before the generic body at [FeedDetailContent.tsx:37](/home/wojtas/projects/brawler/src/shared/components/feedDetail/FeedDetailContent.tsx:37); the visible label is asserted at [FeedDetailContent.test.tsx:90](/home/wojtas/projects/brawler/src/shared/components/feedDetail/FeedDetailContent.test.tsx:90). |
| 8 | **FIXED** | The production browser test forces modal overflow, checks every destination against `.ui-modal-body`, finds its nearest scrolling ancestor, and pins `.activity-panel` to `overflow-y: visible` at [activity.spec.ts:88](/home/wojtas/projects/brawler/tests/browser/activity.spec.ts:88). A synthetic inserted-scroller self-test is unnecessary; the production assertion directly catches the regression. |
| 9 | **FIXED** | The KPI unit now asserts the table’s `ui-zebra` class at [CoreKpiTable.test.tsx:139](/home/wojtas/projects/brawler/src/screens/Spolka/CoreKpiTable.test.tsx:139). Pixel behavior remains covered by the Spółka visual baseline. |
| 10 | **FIXED as scoped** | The two named comments were corrected at [toolRegistry.tsx:159](/home/wojtas/projects/brawler/src/screens/Spolka/toolRegistry.tsx:159) and [FundamentalsPanel.tsx:510](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPanel.tsx:510). `ł/Ł` folding and its test are at [SpolkaScreen.tsx:67](/home/wojtas/projects/brawler/src/screens/Spolka/SpolkaScreen.tsx:67) and [SpolkaScreen.test.tsx:876](/home/wojtas/projects/brawler/src/screens/Spolka/SpolkaScreen.test.tsx:876). Broader documentation drift remains; see Finding 4. |

## New / remaining findings

1. **[BLOCKER] Capacity is derived from the rendered slice, not from all candidate periods, and the two consumers fail in different ways.**

   `useVisiblePeriods` starts at one column and invokes the DOM measurer at [useVisiblePeriods.ts:44](/home/wojtas/projects/brawler/src/screens/Companies/useVisiblePeriods.ts:44) and [useVisiblePeriods.ts:52](/home/wojtas/projects/brawler/src/screens/Companies/useVisiblePeriods.ts:52). The facts matrix queries only currently mounted `[data-period-cell]` elements at [FundamentalsPanel.tsx:349](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPanel.tsx:349); Pozycje does the same at [FundamentalsPeriodsSection.tsx:118](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPeriodsSection.tsx:118).

   Consequences:

   - Pozycje passes a numeric `stickyWidth`, so its observer effect does not rerun merely because capacity rendered more columns. An older, wider value or warning chip can therefore overflow after the first newest-only measurement.
   - Facts passes a new inline `stickyWidth` function every render, while that function is a `recompute` dependency at [useVisiblePeriods.ts:64](/home/wojtas/projects/brawler/src/screens/Companies/useVisiblePeriods.ts:64). A wider cell that appears only at capacity N can shrink capacity; removing that cell can grow it again, producing an N↔M effect loop.
   - `naturalCellWidth` adds padding but not borders at [useVisiblePeriods.ts:98](/home/wojtas/projects/brawler/src/screens/Companies/useVisiblePeriods.ts:98), while every Pozycje subcolumn has a border at [companies.css:751](/home/wojtas/projects/brawler/src/styles/screens/companies.css:751).
   - Both consumers always reserve the 44px expander before knowing whether every period fits. Around the boundary, a period can be hidden solely to make room for an expander that would not be needed if all periods were shown.

   The fixture masks the first defect because values monotonically increase toward the newest period at [overlays.ts:276](/home/wojtas/projects/brawler/src/test/scenarios/overlays.ts:276). The browser test then reads capacity back from the implementation at [fundamentals-periods.spec.ts:44](/home/wojtas/projects/brawler/tests/browser/fundamentals-periods.spec.ts:44), so it does not independently prove the count.

   **Concrete fix:** measure all period cells independently of the visible slice—e.g. an offscreen measurement row/table or data-derived measurement model—then calculate from a stable global maximum. Stabilize both measurer callbacks in refs, include borders, and first test whether all periods fit without the expander before reserving its width. Add an older-widest fixture, an expander-boundary fixture, and independent expected counts for both tables.

2. **[BLOCKER] Collapsed quarterly Pozycje is knowingly clipped at S, with no way to reach the hidden content.**

   Production uses `overflow-x: clip` at [companies.css:732](/home/wojtas/projects/brawler/src/styles/screens/companies.css:732). Yet the S-tier quarterly test explicitly requires `scrollWidth > clientWidth` at [density-companies.spec.ts:155](/home/wojtas/projects/brawler/tests/browser/density-companies.spec.ts:155) and [density-companies.spec.ts:180](/home/wojtas/projects/brawler/tests/browser/density-companies.spec.ts:180). With `clip`, that overflow is not user-scrollable. Therefore the reported all-green run proves clipped, unreachable value/QoQ/YoY content instead of the new “fills but never overflows” contract.

   This happens because capacity clamps to at least one group, but one quarterly group plus the fixed 140px KPI and 44px expander columns can itself exceed a 380px pane after section padding.

   **Concrete fix:** define an S compact representation that guarantees one period group fits—reduce/fold the fixed columns or stack value/QoQ/YoY inside one period column. Then reverse this test to require `scrollWidth <= clientWidth` while collapsed and add the same geometric contract used for the facts matrix.

3. **[MEDIUM] `60vh` is not actually a fallback when `cqh` is unsupported, and round‑2 WebView2 evidence is still absent.**

   The single declaration `max-height: min(60vh, 80cqh)` at [companies.css:461](/home/wojtas/projects/brawler/src/styles/screens/companies.css:461) is wholly invalid if `cqh` is unsupported; `60vh` does not survive as a fallback. The pinned Chromium path supports it—the sticky spec requires a non-`none` computed cap at [fundamentals-sticky.spec.ts:110](/home/wojtas/projects/brawler/tests/browser/fundamentals-sticky.spec.ts:110)—but the current PR body still marks the third, post-round‑2 WebView2 drive pending at [pr-wave-body.md:28](/home/wojtas/projects/brawler/docs/plans/wave-2026-09-contracts/pr-wave-body.md:28).

   **Concrete fix:** declare `max-height: 60vh` first, followed by `max-height: min(60vh, 80cqh)`, and make the live test assert the expanded wrapper’s computed cap and vertical scroll ownership.

4. **[MEDIUM] Normative docs and live comments still describe removed or contradictory behavior.**

   - IA still says there is a reporting-period list, newest-first matrix, and source/completeness bar at [ui-information-architecture.md:222](/home/wojtas/projects/brawler/docs/ui-information-architecture.md:222).
   - Wiki says the static footer ticket and figure are “one control” and both click/Enter-open the filing at [company-view.md:90](/home/wojtas/projects/brawler/wiki/company-view.md:90). The implementation has only the figure button; the ticket is a span without activation at [CoreKpiTable.tsx:156](/home/wojtas/projects/brawler/src/screens/Spolka/CoreKpiTable.tsx:156), although CSS misleadingly gives it a pointer cursor at [spolka.css:251](/home/wojtas/projects/brawler/src/styles/screens/spolka.css:251).
   - The sticky spec’s header comment still says both wrappers are always bounded 2-axis scrollers at [fundamentals-sticky.spec.ts:4](/home/wojtas/projects/brawler/tests/browser/fundamentals-sticky.spec.ts:4).
   - Removed Reporting-period and Autopilot UI leaves dead CSS at [companies.css:416](/home/wojtas/projects/brawler/src/styles/screens/companies.css:416) and [companies.css:1762](/home/wojtas/projects/brawler/src/styles/screens/companies.css:1762).
   - The PR draft claims Pozycje retains a trailing filler cell at [pr-wave-body.md:34](/home/wojtas/projects/brawler/docs/plans/wave-2026-09-contracts/pr-wave-body.md:34), but round 2 removed it.

   **Concrete fix:** rewrite the IA/wiki/spec/PR text to match the current contract, remove dead styles, and change the static ticket cursor to non-interactive.

5. **[LOW] New tier/live guards remain structurally fragile.**

   The visual catalog does shoot Fundamentals at S/M/L at [catalog.core.mjs:18](/home/wojtas/projects/brawler/tests/browser/visual/catalog.core.mjs:18), but the density test never directly asserts Trend hidden at S and visible at M. The live header probe uses `.at(-2)` and tries to exclude `[data-expander]` at [wave-2026-09.live.spec.ts:74](/home/wojtas/projects/brawler/tests/live/wave-2026-09.live.spec.ts:74), although the expander has a class rather than that attribute. It works today only because the hidden Trend `<th>` remains last in the DOM.

   **Concrete fix:** assert Trend visibility explicitly across S/M and select period headers with the same class exclusions used by `fundamentals-periods.spec.ts`.

## Attack-surface conclusions

- **A/B:** Dirty-tool Escape still passes through guarded `closeTool`; palette close restores its invoker; no other Modal consumer was found relying on a prevented Escape to close. Mouse selection ordering is protected. Touch remains unverified on the post-round‑2 WebView build.
- **C:** Not safe; rendered-slice measurement and callback identity create undermeasurement/oscillation paths.
- **D:** Table semantics are fixed; one-row coverage is advisable, not the blocking issue.
- **E:** Collapsed nesting is fixed; expanded nesting is intentional and bounded, subject to the `cqh` fallback.
- **F:** Zebra/focus specificity is fixed.
- **G/H:** Retired keys, translations, persistent deep links, retargeting, and close-clearing are intact.
- **I/J:** Documentation drift and capacity-test blind spots remain. The Activity production guard is not vacuous; the capacity and S-quarterly guards are.

PR state also blocks shipment: GitHub reported head `c6131b26`, `mergeStateStatus: DIRTY` and an empty check rollup. The PR body reports local `check-local`, all five Playwright projects, and visual checks green, but no current-head GitHub checks or post-round‑2 live drive substantiate them. Resolve the master conflict and rerun the gates on the resulting head.

## Verdict

**DO-NOT-SHIP**. The collapsed-capacity contract can both oscillate and clip unreachable quarterly content, and PR #468 is currently not mergeable.

Codex session ID: 01a07c1c-5e62-7f52-b585-4265e8fb0227
Resume in Codex: codex resume 01a07c1c-5e62-7f52-b585-4265e8fb0227
