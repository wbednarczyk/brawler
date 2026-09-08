Verdict: **DO-NOT-SHIP** the plan in its current form.

Several root causes are correct, but #5 is unverified, #9’s proposed regression test is already green, and #3/#6 contain behavioral contradictions. No files were edited.

## Root-cause audit

| # | Verdict | Tree evidence |
|---|---|---|
| 1/1b | Verified | The dotted thread is a non-interactive `<td>` class at [CoreKpiTable.tsx:98](/home/wojtas/projects/brawler/src/screens/Spolka/CoreKpiTable.tsx:98); the separate ticket remains a button at [CoreKpiTable.tsx:122](/home/wojtas/projects/brawler/src/screens/Spolka/CoreKpiTable.tsx:122). |
| 2 | Verified | `PanelHeader` is flex/`space-between` at [ui.css:18](/home/wojtas/projects/brawler/src/styles/ui.css:18), and the Spółka header passes the picker through `actions` at [SpolkaScreen.tsx:181](/home/wojtas/projects/brawler/src/screens/Spolka/SpolkaScreen.tsx:181). No Spółka-scoped header layout exists. |
| 3 | Partly verified | The picker is `SelectField` and its focus ref is `HTMLSelectElement` at [SpolkaScreen.tsx:102](/home/wojtas/projects/brawler/src/screens/Spolka/SpolkaScreen.tsx:102) and [SpolkaScreen.tsx:193](/home/wojtas/projects/brawler/src/screens/Spolka/SpolkaScreen.tsx:193). The palette owns the only complete APG combobox implementation. The proposed Escape behavior is not compatible with the current frame guard. |
| 4 | Verified | Both sections exist at [FundamentalsPanel.tsx:943](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPanel.tsx:943) and [FundamentalsPanel.tsx:962](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPanel.tsx:962); bulk/single-selection autopilot remains reachable in `CompanySettingsManager`. |
| 5 | **Not verified** | The stated CSS facts are real, but they do not establish the cause. Body row headers already have opaque `var(--surface)` and `z-index:1` at [companies.css:489](/home/wojtas/projects/brawler/src/styles/screens/companies.css:489). Missing `z-index` on the sticky top header does not explain values painting over body row labels. |
| 6 | Verified as implementation inventory | Facts are sorted oldest→newest and returned uncapped at [factMatrix.ts:114](/home/wojtas/projects/brawler/src/screens/Companies/factMatrix.ts:114); the matrix renders all periods twice at [FundamentalsPanel.tsx:585](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPanel.tsx:585). Pozycje × okresy caps at eight at [FundamentalsPeriodsSection.tsx:25](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPeriodsSection.tsx:25). The proposed responsive behavior is not yet specified well enough. |
| 7 | Verified | Origin and completeness derive from `latestMatrixPeriod` at [FundamentalsPanel.tsx:395](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPanel.tsx:395) and render in the footer at [FundamentalsPanel.tsx:678](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPanel.tsx:678). |
| 8 | Verified | Both rows render raw `item.type`: [CompanyFeedSection.tsx:178](/home/wojtas/projects/brawler/src/screens/Companies/CompanyFeedSection.tsx:178), [InboxScreen.tsx:296](/home/wojtas/projects/brawler/src/screens/Inbox/InboxScreen.tsx:296). Detail-only mapping exists at [FeedDetailContent.tsx:28](/home/wojtas/projects/brawler/src/shared/components/feedDetail/FeedDetailContent.tsx:28). |
| 9 | Partly verified | `feedItemSummary` suppresses only `filing`, but the named overlay at [overlays.ts:453](/home/wojtas/projects/brawler/src/test/scenarios/overlays.ts:453) is already a filing and is already suppressed. Real attachment-bearing reports can carry the literal, as shown by [report_documents.rs:212](/home/wojtas/projects/brawler/src-tauri/src/storage/tests/report_documents.rs:212). |
| 10 | Verified only for Activity | `.activity-panel` is the inner `overflow:auto` owner at [activity.css:10](/home/wojtas/projects/brawler/src/styles/activity.css:10), inside the already padded/scrollable modal body at [ui.css:1187](/home/wojtas/projects/brawler/src/styles/ui.css:1187). The other five scrollers have not been shown to share the defect. |
| 11 | Verified | The component scrolls, sets a transient attribute, and clears it after four seconds at [CompanyReportDocumentsPanel.tsx:355](/home/wojtas/projects/brawler/src/shared/components/CompanyReportDocumentsPanel.tsx:355); no CSS selector styles that attribute. |
| 12 | Verified | Every Documents target gets `Open document` through one target-based branch at [ActivityPanel.tsx:54](/home/wojtas/projects/brawler/src/shared/components/activity/ActivityPanel.tsx:54). |

## Blocking findings

1. **#5’s proposed stacking fix can make the header overlap worse.**

   Setting every sticky header to `z-index:3` while the corner is currently `z-index:2` means ordinary horizontally scrolling period headers can paint above the sticky corner. The causal claim also confuses vertical header overlap with the reported horizontal body-cell bleed.

   Concrete fix: first reproduce with 12 periods, long KPI labels, selected/hovered cells, horizontal and vertical scroll, S/M/L, dark/light, and the Windows WebView2 runtime. Probe both the corner and a body row header using `elementFromPoint(...).closest("th")`, opaque computed backgrounds, and a tight screenshot because hit-testing alone cannot detect a compositor paint defect. If vertical stacking is implicated, use body sticky `1`, header `2`, corner `3`; do not assign `3` to every header.

2. **#3’s Escape contract is impossible as written.**

   `[role=combobox]` is unconditionally excluded by [toolRegistry.tsx:30](/home/wojtas/projects/brawler/src/screens/Spolka/toolRegistry.tsx:30) and [toolRegistry.tsx:100](/home/wojtas/projects/brawler/src/screens/Spolka/toolRegistry.tsx:100). Therefore Escape from a closed combobox never reaches the tool frame, regardless of `preventDefault`. `SearchField` also consumes Escape whenever its query is non-empty at [SearchField.tsx:45](/home/wojtas/projects/brawler/src/ui/SearchField.tsx:45).

   Concrete fix: reserve `NATIVE_PICKER_SELECTOR` for genuinely native popup controls. Composite widgets must call `preventDefault()` only while consuming Escape themselves. Test four integrated states: popup open, closed/empty, closed/non-empty, and dirty tool.

   The plan names the wrong select-pinned test. `App.test.tsx` mostly queries the accessible combobox role; the actual `selectOptions` dependency is [SpolkaToolHost.test.tsx:365](/home/wojtas/projects/brawler/src/screens/Spolka/SpolkaToolHost.test.tsx:365) and [SpolkaToolHost.test.tsx:400](/home/wojtas/projects/brawler/src/screens/Spolka/SpolkaToolHost.test.tsx:400).

3. **The proposed one-use `ComboboxField` violates the repository’s primitive rule.**

   `ui-authoring.md` requires a new primitive to have at least two real uses. The plan creates it for Spółka while explicitly deferring palette migration.

   Concrete fix: introduce a generic combobox/listbox primitive or headless controller and use it in both the company picker and `CommandPalette` now; keep the modal/command execution domain-specific. Also explicitly dispose of `GlobalSearch`, which already renders a `listbox` with button options but lacks combobox ownership at [GlobalSearch.tsx:130](/home/wojtas/projects/brawler/src/app/GlobalSearch.tsx:130) and [GlobalSearch.tsx:160](/home/wojtas/projects/brawler/src/app/GlobalSearch.tsx:160). Migrate it or obtain owner approval for a tracked follow-up.

4. **#6 is not implementation-ready and contradicts the recorded owner decision.**

   The ledger says autoscroll inside the expanded state; the plan adds autoscroll on initial mount. That changes every default Fundamentals visual baseline and can make a matrix appear mysteriously pre-scrolled.

   Additional defects:

   - “Measured once” is wrong for a resizable pane. Use `ResizeObserver` on the actual pane/scroller and react to tier changes.
   - Adding an independent “first header cell” shifts table-column association unless every body row gains a matching column. Put the control inside the existing sticky corner cell or immediately above the scroller.
   - The expander needs an explicit Tab/Enter/Space/focus-visible test.
   - Facts use one column per period; Pozycje × okresy uses two or three. They should share expansion/order logic, not necessarily one numeric limit. Preserve `MAX_PERIODS=8` as an upper bound until browser measurements establish per-host S/M/L limits.
   - Coverage is period-per-row, not newest-period-at-the-right; Report Diff is a list. Record both as audited exemptions rather than silently omitting them.

   This needs the actual ADR 0081 mini-contract and storyboard before approval, not merely a promise to create them in S0.

5. **#9 would destroy meaningful report summaries and the proposed test is vacuous.**

   The current test deliberately proves a `report` retains a meaningful summary at [useNotebookController.test.ts:35](/home/wojtas/projects/brawler/src/app/useNotebookController.test.ts:35). Notebook draft construction depends on it at [useNotebookController.test.ts:51](/home/wojtas/projects/brawler/src/app/useNotebookController.test.ts:51) and [useNotebookController.test.ts:64](/home/wojtas/projects/brawler/src/app/useNotebookController.test.ts:64). Today’s `FilingRow` and storage-backed global-search snippets do not call this helper; the notebook draft is the actual affected consumer.

   Concrete fix: suppress the exact dead literal, independently of attachment/presentation kind, rather than blanking all reports. Separate display-summary behavior from draft fallback if necessary. Tests must cover:

   - attachment-bearing report + `Komunikat ESPI/EBI` → hidden;
   - report + meaningful summary → preserved;
   - note draft with empty body + meaningful summary → summary retained;
   - note draft with dead literal → deliberate title/empty fallback.

6. **#10 overgeneralizes one reproduced defect into six layout changes.**

   `.feed-detail-body` is a prose scroller, not a scroller hosting row actions. The other four proposed additions likewise lack reproduction evidence. `scrollbar-gutter:stable` reserves space whenever classic scrollbars could appear, shifting layouts even with no overflow; overlay scrollbars may reserve nothing, so the padding still does the real work.

   Concrete fix: first try removing Activity’s nested scroll owner and let the padded `.ui-modal-body` own scrolling. Otherwise scope `padding-inline-end`/gutter to `.activity-panel` only. Add other selectors only after individual reproduction.

   The guard belongs in Playwright. Stylelint cannot infer that a CSS scroller hosts a particular DOM descendant. The helper must compare the action’s right edge with `scroller.clientWidth`/content edge—not merely the outer bounding box used by [interactionContracts.ts:53](/home/wojtas/projects/brawler/tests/browser/helpers/interactionContracts.ts:53)—and run with forced vertical overflow. An exact Activity CSS-source contract may supplement it, but cannot replace it.

7. **#11 has an unresolved focus and consistency contract.**

   The ledger requests row focus; the plan silently keeps heading focus. The latter better matches ADR 0107 and avoids introducing a focusable `<li>`, but a persistent `data-*` paint alone is invisible to screen readers. Claims still clears the same navigation highlight after four seconds at [CompanyClaimsPanel.tsx:137](/home/wojtas/projects/brawler/src/shared/components/CompanyClaimsPanel.tsx:137).

   Concrete fix: define one deep-link-target contract. Recommended: keep heading focus, scroll the target, persist its visible selection while the tool remains open, expose `aria-current` or an equivalent selected-state announcement, and apply the same rule to Claims. If Claims intentionally remains transient, that inconsistency needs owner approval and documented rationale—not an unapproved deferred card.

   Tests should advance beyond four seconds, retarget to another document, unmount/close, assert no duplicate focus stop, and verify both accessible state and expected border/background.

8. **#1b creates two controls for one source.**

   The footer ticket is already a button. Adding a button inside the `<td>` produces two consecutive controls opening the same document, likely with the same generic accessible name.

   Concrete fix: expose one semantic action. The simplest valid structure is a real button inside the threaded cell and a non-interactive footer ticket; give the button a specific name containing metric, period, and source. If the ticket must also be clickable, redesign the markup so both visual hit areas belong to one control before implementation.

   ADR 0104 decision 7 should say: “The thread and ticket form one provenance action with one keyboard focus target and one accessible name; hover/focus highlights the pair; activation navigates.” Test exactly one source-navigation control.

## Additional required fixes

9. **#2 lacks a test that can redden on the reported defect.**

   Component tests cannot prove visual centering. Add browser geometry assertions: at L/M the picker’s center is within tolerance of the panel center, independent of identity width; at S it stacks without overlap or overflow. Amend ADR 0107, as the ledger itself requires.

10. **#4’s ADR amendment and documentation list are too broad/incomplete.**

   Do not retract all of ADR 0056’s in-context-control language. Amend it narrowly: Fundamentals’ autopilot editor retires; Companies → Manage settings becomes the only autopilot editor, including the one-company case; IR URL and sector override remain in Basic Info.

   Also update:

   - [wiki/company-settings.md](/home/wojtas/projects/brawler/wiki/company-settings.md:1)
   - [wiki/README.md:46](/home/wojtas/projects/brawler/wiki/README.md:46)
   - [fundamentals-ai.spec.ts:30](/home/wojtas/projects/brawler/tests/browser/fundamentals-ai.spec.ts:30)
   - the existing fold tests at [FundamentalsPanel.test.tsx:143](/home/wojtas/projects/brawler/src/screens/Companies/FundamentalsPanel.test.tsx:143) and [density-companies.spec.ts:69](/home/wojtas/projects/brawler/tests/browser/density-companies.spec.ts:69).

11. **#8’s helper must cover the entire closed union.**

   `kindChip` currently returns `null` for `redFlag`. A “no raw `item.type`” assertion could pass by rendering no kind at all.

   Concrete fix: move an exhaustive `presentationKind → localized label + tone` mapping into a host-neutral feed-presentation module, including `redFlag`, and use it in detail plus both row hosts. Assert the expected visible chip in EN and PL for all four kinds.

12. **The slicing and visual budget are not credible.**

   Current direct conflicts include:

   - S1/S3: `companies.css`;
   - S2/S3: `spolka.css`;
   - S2/S3: `App.test.tsx`;
   - all slices: locale resources;
   - owner merge: docs and visual catalog updates.

   The baseline list also omits at least `company-feed` and `spolka-tool-claims`; those are directly changed by #8 and the Spółka header. Broad gutter changes to `.spolka-tool-body`, Today, Research, feed detail, and Fundamentals would potentially churn nearly every corresponding tool/screen baseline.

   Concrete fix: assign shared locale/CSS/test tails to one integration slice and require each worker to touch unique files. Include a baseline manifest naming every expected PNG before updates. Prefer putting #6 in its own PR because it is a responsive pattern change with a separate experience contract; if the one-wave/one-PR owner decision stands, make #6 an isolated commit and explicit review gate inside the PR.

One repository-state discrepancy: the SHA and branch match, but `git status --short --branch` reports `?? scratchpad/`, so the worktree is not literally clean as stated.

Codex session ID: 01a06d22-21f7-7790-9c2f-fe4545e96a7a
Resume in Codex: codex resume 01a06d22-21f7-7790-9c2f-fe4545e96a7a
