Reviewed current head `dd96206d73813788cf6144a43735910f02df6eea`; merge base remains `7964f1d5`. `git diff --check` is clean. PR #468 is mergeable/CLEAN, CI is 21/21 green, and live drive 4 passed 1/1 on the exact head.

## R3 findings

1. **FIXED — facts capacity invalidation now covers the rendered data signature.**

   [periodMeasureKeys.ts](/home/wojtas/projects/brawler/src/screens/Companies/periodMeasureKeys.ts:9) includes locale, period identity/label inputs, latest-period/origin state, definition formatting fields, every rendered numeric/as-reported value, currency, quality marker and annotation presence. [FundamentalsPanel.tsx](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPanel.tsx:341) derives the latest period and provenance state before calling the hook, then supplies the complete key at line 369.

   The former late-provenance and surviving-value-edit paths therefore trigger a new hidden measuring pass. [periodMeasureKeys.test.ts](/home/wojtas/projects/brawler/src/screens/Companies/periodMeasureKeys.test.ts:21) pins value, annotation, origin, mixed-origin and locale changes.

   `localizedKpiLabel` does not need to enter the period-width key: it renders in the fixed 140 px sticky KPI column at [FundamentalsFactsMatrix.tsx](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsFactsMatrix.tsx:111), whose width is fixed in [companies.css](/home/wojtas/projects/brawler/src/styles/screens/companies.css:549). Facts header width inputs—year, period type and origin label—are covered.

2. **FIXED — Pozycje remeasures after S/M/L presentation changes.**

   [useVisiblePeriods.ts](/home/wojtas/projects/brawler/src/screens/Companies/useVisiblePeriods.ts:33) accepts `measureVariant`; every recomputation compares the current variant with the measured one at lines 69–77. A mismatch invalidates `measuredKey` and starts a fresh all-period pass.

   [FundamentalsPeriodsSection.tsx](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPeriodsSection.tsx:112) defines the variant from the actually displayed `value,qoq,yoy` subset and skips `display:none` columns when summing per-column maxima at lines 130–141.

   The two-direction S→M→L/L→M→S oracle at [fundamentals-periods.spec.ts](/home/wojtas/projects/brawler/tests/browser/fundamentals-periods.spec.ts:281) independently expands, measures displayed column groups, applies the capacity formula, collapses, and checks both count and overflow.

3. **FIXED — the specifically reported R3 comments and missing delta regression case are addressed.**

   Both picker comments in [shortcuts.ts](/home/wojtas/projects/brawler/src/app/shortcuts.ts:83) now identify the combobox input; [useSpolkaScreenWiring.tsx](/home/wojtas/projects/brawler/src/app/useSpolkaScreenWiring.tsx:33) says “control”; the density test describes clipped-collapsed versus scrolling-expanded behavior.

   [periodMeasureKeys.test.ts](/home/wojtas/projects/brawler/src/screens/Companies/periodMeasureKeys.test.ts:42) now changes `deltaYoY` and proves the key changes.

## Attack results

- **(a) Mostly pass.** All data-driven rendered width inputs are represented. The remaining non-data invalidation gap is finding 4 below.
- **(b) Pass.** `measuring` requires `total > 0` at [useVisiblePeriods.ts](/home/wojtas/projects/brawler/src/screens/Companies/useVisiblePeriods.ts:67). The variant is captured before the immediate recomputation, so an initial empty variant cannot create an infinite pass.
- **(c) Pass for resize ordering.** The measuring work runs in the layout effect at lines 97–103; the observer is installed later in the passive effect at lines 105–113. An observer callback during an invalidated render can at worst repeat `setMeasuredKey(null)` before the layout pass settles it.
- **(d) Pass.** The host publishes source-data total at [FundamentalsPeriodsSection.tsx](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPeriodsSection.tsx:256). The oracle reads that total but reimplements natural measurement and the capacity formula at [fundamentals-periods.spec.ts](/home/wojtas/projects/brawler/tests/browser/fundamentals-periods.spec.ts:294); it does not read `visibleCount` or `data-visible-periods`.
- **(e) Pass.** The merged #474/#475 acceptance-corpus, protected-journey and one-home-per-rule changes remain intact. No substantive contradiction with those master changes was introduced.

## New findings

4. **MEDIUM — late webfont swaps can leave the natural-width cache stale.**

   The bundled UI fonts deliberately use `font-display: swap` at [fonts.css](/home/wojtas/projects/brawler/src/styles/fonts.css:11), so a measuring pass can observe fallback-font metrics and later render Schibsted Grotesk metrics. The hook only invalidates for `measureKey`, presentation variant, or a resize of the wrapper at [useVisiblePeriods.ts](/home/wojtas/projects/brawler/src/screens/Companies/useVisiblePeriods.ts:95); it does not observe `document.fonts`/`FontFaceSet`. A font swap can change table intrinsic widths without changing the wrapper’s `clientWidth`, so its `ResizeObserver` need not fire. Because collapsed wrappers use `overflow-x: clip` at [companies.css](/home/wojtas/projects/brawler/src/styles/screens/companies.css:450), an overestimated capacity can silently clip content.

   **Concrete fix:** invalidate the cached measurement on `document.fonts.loadingdone`—with cleanup and no-op fallback where `document.fonts` is unavailable—and remeasure once after an in-flight `document.fonts.ready`. Add a browser test that delays the relevant WOFF2 response, lets the fallback measurement occur, releases the font, and asserts the final collapsed count/overflow.

5. **LOW — several comments and one canonical density sentence still describe the retired always-scrolling/bar behavior.**

   Examples:

   - [App.test.tsx](/home/wojtas/projects/brawler/src/App.test.tsx:540) still calls the picker a `<select>`.
   - [FundamentalsFactsMatrix.tsx](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsFactsMatrix.tsx:57) says the matrix scrolls inside the wrapper without distinguishing collapsed/expanded states.
   - [FundamentalsPeriodsSection.tsx](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPeriodsSection.tsx:247) makes the same always-scrolling claim.
   - [factMatrix.ts](/home/wojtas/projects/brawler/src/screens/Companies/factMatrix.ts:11) still names the retired “completeness bar”.
   - [ui-authoring.md](/home/wojtas/projects/brawler/docs/ui-authoring.md:230) says the S-tier facts matrix scrolls, while the shipped collapsed state clips a measured fitting slice and only the expanded state scrolls.

   **Concrete fix:** rewrite these as current constraints: combobox input; collapsed newest-fitting slice with no scrollbar; expanded bounded scroller; unnamed-row warning chip in the section header.

## Verdict

**SHIP-WITH-FIXES**

The two R3 release blockers are fixed, and exact-head automated/live evidence is green. Address the font-load invalidation before merge because it is the remaining path to stale capacity and silent clipping; clean the stale comments in the same follow-up.

Codex session ID: 01a07c1c-5e62-7f52-b585-4265e8fb0227
Resume in Codex: codex resume 01a07c1c-5e62-7f52-b585-4265e8fb0227
