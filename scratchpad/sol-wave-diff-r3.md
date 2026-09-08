Current head reviewed: `6eb97a677dcc6092f2d0ed46e347acac4bb91583`; merge-base remains `7964f1d5`. `git diff --check` is clean.

## Re-verification

1. **NOT-FIXED — capacity measurement remains incomplete.**  
   The hidden all-period pass itself is implemented: `measuring` renders `total` periods and measures in a layout effect ([useVisiblePeriods.ts:62](/home/wojtas/projects/brawler/src/screens/Companies/useVisiblePeriods.ts:62), [useVisiblePeriods.ts:86](/home/wojtas/projects/brawler/src/screens/Companies/useVisiblePeriods.ts:86), [useVisiblePeriods.ts:105](/home/wojtas/projects/brawler/src/screens/Companies/useVisiblePeriods.ts:105)). The oldest-widest fixture and independent facts-matrix oracle are real ([overlays.ts:220](/home/wojtas/projects/brawler/src/test/scenarios/overlays.ts:220), [fundamentals-periods.spec.ts:44](/home/wojtas/projects/brawler/tests/browser/fundamentals-periods.spec.ts:44)).  
   However, the cache is not invalidated by all width-affecting data. See blocker 1 below.

2. **FIXED for the reported S-tier clipping, but a cross-tier defect remains.**  
   S narrows the KPI column and hides both delta columns ([companies.css:1867](/home/wojtas/projects/brawler/src/styles/screens/companies.css:1867)); the browser guard asserts `clip`, no overflow, hidden deltas, and visible values ([density-companies.spec.ts:175](/home/wojtas/projects/brawler/tests/browser/density-companies.spec.ts:175)). See blocker 2 for stale measurement after S↔M resizing.

3. **FIXED — `cqh` fallback.**  
   Expanded tables first receive `max-height: 60vh`; `min(60vh, 80cqh)` is confined to `@supports` ([companies.css:463](/home/wojtas/projects/brawler/src/styles/screens/companies.css:463)). This is safe for older WebView2 builds.

4. **FIXED for the named documentation/CSS corrections.**  
   IA now describes no periods list, the newest-header origin chip, and header warning chip ([ui-information-architecture.md:222](/home/wojtas/projects/brawler/docs/ui-information-architecture.md:222)); the wiki correctly identifies the figure as the control and ticket as its static label ([company-view.md:89](/home/wojtas/projects/brawler/wiki/company-view.md:89)); the ticket has no pointer cursor ([spolka.css:242](/home/wojtas/projects/brawler/src/styles/screens/spolka.css:242)). `.periods-list` remains legitimately used by `CustomKpiManager`; dead `.fundamentals-autopilot*` production CSS is gone. The merged engineering-workflow resolution retains both sides’ live rules. Residual comment archaeology is finding 3.

5. **FIXED — Trend and live selectors.**  
   Trend is explicitly tested hidden at S and visible at M ([density-companies.spec.ts:248](/home/wojtas/projects/brawler/tests/browser/density-companies.spec.ts:248)); Fundamentals remains in the full S/M/L visual catalog ([catalog.core.mjs:20](/home/wojtas/projects/brawler/tests/browser/visual/catalog.core.mjs:20)); the live test selects the newest header through `th[data-period-cell]`, independent of hidden/expander columns ([wave-2026-09.live.spec.ts:74](/home/wojtas/projects/brawler/tests/live/wave-2026-09.live.spec.ts:74)).

6. **FIXED — closed combobox Home/Enter/End.**  
   Production returns without consuming these keys when closed ([useComboboxListbox.ts:90](/home/wojtas/projects/brawler/src/ui/useComboboxListbox.ts:90)); the unit test now sends Home and confirms `aria-expanded=false` ([useComboboxListbox.test.tsx:42](/home/wojtas/projects/brawler/src/ui/useComboboxListbox.test.tsx:42)). The layered coverage is sufficient: the App test proves Shift+J focuses the real picker ([App.test.tsx:629](/home/wojtas/projects/brawler/src/App.test.tsx:629)), while `ComboboxField` opens only on click, not focus ([ComboboxField.tsx:91](/home/wojtas/projects/brawler/src/ui/ComboboxField.tsx:91)). An additional Shift+J/list-closed assertion would be useful but is not required.

## New findings

1. **BLOCKER — facts capacity is cached before provenance arrives and survives value changes.**  
   The facts `measureKey` contains only locale, period IDs, and visible definition IDs ([FundamentalsPanel.tsx:342](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPanel.tsx:342)). Yet measurement includes formatted values, annotation/quality markers, and the origin chip. Provenance arrives asynchronously after the initial measuring pass ([FundamentalsPanel.tsx:196](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPanel.tsx:196)), then changes the newest header chip ([FundamentalsPanel.tsx:402](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPanel.tsx:402), [FundamentalsFactsMatrix.tsx:86](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsFactsMatrix.tsx:86)) without changing the key. Editing an existing fact also preserves its ID while changing its formatted width, then refreshes the same row set ([useFundamentalsController.ts:107](/home/wojtas/projects/brawler/src/app/useFundamentalsController.ts:107), [useFundamentalsController.ts:156](/home/wojtas/projects/brawler/src/app/useFundamentalsController.ts:156)). The cached capacity can therefore become too large, and collapsed `overflow-x: clip` silently hides the excess.

   **Concrete fix:** invalidate on the actual presentation signature: formatted cell values, marker presence, relevant definition formatting fields, and origin label/mixed state. Do the equivalent for Pozycje comparison values/flags—not merely metric keys. Add threshold tests where provenance resolves after mount and an existing fact is edited from a short to a wide value.

2. **BLOCKER — Pozycje reuses an S-only width cache after its delta columns reappear at M.**  
   Its measurement explicitly skips `display:none` cells ([FundamentalsPeriodsSection.tsx:120](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPeriodsSection.tsx:120)), while S hides QoQ/YoY ([companies.css:1869](/home/wojtas/projects/brawler/src/styles/screens/companies.css:1869)). The cached width is refreshed only during a `measureKey` pass; ResizeObserver merely recomputes capacity from that cache ([useVisiblePeriods.ts:64](/home/wojtas/projects/brawler/src/screens/Companies/useVisiblePeriods.ts:64), [useVisiblePeriods.ts:93](/home/wojtas/projects/brawler/src/screens/Companies/useVisiblePeriods.ts:93)). The key contains no responsive column-set state ([FundamentalsPeriodsSection.tsx:111](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPeriodsSection.tsx:111)). Opening at S therefore caches only `value`; resizing collapsed to M makes QoQ/YoY visible while capacity still divides by the value-only width, allowing too many groups before the clipped edge. M→S is conversely over-conservative.

   The existing density test reaches S after opening and never resizes the collapsed table back to M ([density-companies.spec.ts:163](/home/wojtas/projects/brawler/tests/browser/density-companies.spec.ts:163)); the independent capacity oracle covers only the facts matrix.

   **Concrete fix:** cache separate maxima for value/QoQ/YoY and sum the currently displayed set during every recompute, or detect the active column mask in ResizeObserver and trigger a new all-period measuring pass when it changes. Add exact-capacity and no-overflow tests for collapsed S→M→L and the reverse.

3. **LOW — live comments still describe retired behavior.**  
   Production shortcut comments still call the company picker a `<select>` ([shortcuts.ts:83](/home/wojtas/projects/brawler/src/app/shortcuts.ts:83), [useSpolkaScreenWiring.tsx:33](/home/wojtas/projects/brawler/src/app/useSpolkaScreenWiring.tsx:33)). The quarterly density test is titled and introduced as testing an internal scrolling table even though it now asserts a clipped, non-scrolling collapsed wrapper ([density-companies.spec.ts:155](/home/wojtas/projects/brawler/tests/browser/density-companies.spec.ts:155)). The Fundamentals comment also says fixed width includes the expander although the implementation reserves it separately ([FundamentalsPanel.tsx:336](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPanel.tsx:336)).

   **Concrete fix:** rewrite these as current combobox, collapsed-clip/expanded-scroll, and separately measured fixed/expander constraints.

The remaining measuring attacks are acceptable: the layout effect settles the hidden pass before paint; measurer callback identity cannot create an effect loop; `total === 0` produces zero visible periods and a fresh mount remeasures; Chromium behavior is pinned by computed `overflowX="clip"` / `overflowY="visible"` browser assertions. The 120px one-row result is the explicit approved floor, not uncontrolled row inflation; a one-row browser case would be prudent but is not a ship requirement. The minimum-one clamp still cannot mathematically guarantee full natural content at arbitrarily small widths, but the supported 380px fixtures pass—add a long Polish origin/status-chip S fixture if that guarantee is intended to include untruncated chip text.

PR status at cutoff: exact-head browser shards, visual baselines, frontend checks, Windows build and boot smoke are green; Rust coverage is still pending. The PR body still records the third post-round-2 live drive as pending.

**Verdict: DO-NOT-SHIP.** The hidden measuring pass fixes slice dependence, but the two cache-invalidation defects can still produce incorrect capacity and silently clipped newest-period content.

Codex session ID: 01a07c1c-5e62-7f52-b585-4265e8fb0227
Resume in Codex: codex resume 01a07c1c-5e62-7f52-b585-4265e8fb0227
