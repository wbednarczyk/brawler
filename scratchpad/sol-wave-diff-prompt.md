READ-ONLY, do not edit files. Adversarial diff review, round 1.

Repo: /home/wojtas/projects/brawler, branch `fix/dogfooding-v0.79-v0.81` (PR #468 → master; base `7964f1d5`, head = `git rev-parse HEAD`). Review `git diff 7964f1d5...HEAD` in full. Use `rtk` prefixes and `repoctx` for structure; never run git checkout/restore/stash/commit.

Context: the dogfooding fix wave 2026-09 — 13 owner findings on v0.79.1 (Spółka core KPI / Fundamentals) and v0.81.0 (Aktywność). The approved plan (rows = contract) is `/home/wojtas/.claude/plans/sparkling-sniffing-pizza.md`; the ledger `docs/retros/dogfooding-v0.79-findings.md`; PR body draft `docs/plans/wave-2026-09-contracts/pr-wave-body.md`. Docs amended on the branch: ADR 0056/0104/0107, IA, ui-flows, ui-authoring, wiki. Verify, don't trust the summary.

Claim inventory (verify each against the tree):
1. #3 `src/ui/useComboboxListbox.ts` + `ComboboxField.tsx` (headless APG combobox, host Escape policy close-list/clear/bubble/close-host, consuming = preventDefault+stopPropagation) drive BOTH the Spółka company picker (`SpolkaScreen.tsx`) and `CommandPalette.tsx`; `Modal.tsx` skips close on `defaultPrevented`; `toolRegistry.tsx` `NATIVE_PICKER_SELECTOR` keeps only native popups. The field opens on click, not focus (Shift+J/K focus intent must not pop the list; activedescendant folds into the accessible name). `onEscapeBubble` prop because the picker is not a DOM descendant of the tool frame.
2. #1b `CoreKpiTable.tsx`: the threaded newest cell holds the ONLY button (`Open source: {metric} · {period} · {source}`), the footer ticket is a static span; `data-thread-hot` lights the pair.
3. #2 `spolka.css` header grid identity · picker · actions, single column under the S container tier.
4. #6 `useVisiblePeriods.ts` (capacity = floor((clientWidth − stickyWidth)/measured period width), clamp [1,8], ResizeObserver, expanded ignores resize) used by the facts matrix (`FundamentalsFactsMatrix.tsx`, extracted from `FundamentalsPanel.tsx`) and Pozycje × okresy (`FundamentalsPeriodsSection.tsx`); full-height sticky expander column (rowSpan td, absolute-positioned button, 120 px cell floor); absorber columns (Trend `width:100%` + `min-width:108px`, a filler cell in Pozycje) so period columns keep natural widths; `FACTS_TREND_COLUMN_WIDTH` joins `stickyWidth`.
5. #4/#7 Reporting periods list + Autopilot fold removed (`CompanyAutopilotField.tsx` deleted), retired keys pinned (`retiredKeys.test.ts`), origin chip inside the newest period `<th>`.
6. #5 `companies.css`: period-table scrollers bounded (`max-height: 60vh; overflow-y: auto`), sticky header for Pozycje, z-index ladder 1/2/3, right edge on the sticky column; guard `tests/browser/fundamentals-sticky.spec.ts`.
7. #13 one `.ui-zebra :where(tbody tr:nth-child(even)) > *` rule in `utilities.css` (`color-mix(--text 6%, --surface)`), applied to facts matrix, Pozycje, Coverage, core KPI table.
8. #8 `feedPresentation.ts` exhaustive kind chip (incl. `redFlag`) used by `FeedDetailContent`, `CompanyFeedSection`, `InboxScreen`.
9. #9 `useNotebookController.feedItemSummary` suppresses the exact literal `Komunikat ESPI/EBI` for any kind.
10. #10 `.activity-panel` no longer a nested scroller; `.ui-modal.activity-modal` carries the height cap; `expectActionInsideScroller` helper + `activity.spec.ts`.
11. #11 `CompanyReportDocumentsPanel.tsx` / `CompanyClaimsPanel.tsx`: persistent `aria-current` + selection driven by the prop (no 4 s timer); Claims main-list row canonical, queue twin `data-claim-match`; CSS `.doc-row[data-document-highlighted="true"]`.
12. #12 `Open in documents` / `Otwórz w dokumentach`; dead keys `Open document`, `Open source document`, t-key `Reporting periods` retired + pinned.

ATTACK list (what I doubt most):
- A. Escape contract completeness: picker closed+non-empty → clear; closed+empty → bubble → `closeTool("overview")` — is a dirty tool still guarded on that path? Does the palette's one-Escape restore the invoker in every host? Does `Modal`'s `defaultPrevented` skip break any other Modal consumer's Escape?
- B. `ComboboxField` blur → `controller.reset()` — pointer selection vs blur ordering (`onMouseDown preventDefault` on the listbox) on WebView2; touch; the displayed value vs query while focused; `aria-activedescendant` when the list is closed.
- C. Capacity measurement: is the measured header width ever the stretched one (single visible column) after my absorber change — at S with the sticky 140+44+108 subtracted, can capacity mis-count by one and push the newest header outside the client box? Expanded state + resize; remount with zero width; `total` changes.
- D. rowSpan expander: `td` height 120 px floor with 1 body row; `aria-hidden` empty `<th>` in the header with a `<td>` in the body — table semantics for screen readers (column count mismatch, `scope`), Tab order, focus-visible.
- E. #5 nested 60vh scroller inside the panel scroller: scroll trapping, keyboard scrolling, the geometric contract (`fundamentals-periods.spec`) at S (380 px) — does the bounded height interact with the pane height tiers (`data-short-expanded`)?
- F. Zebra: `.ui-zebra` in `utilities.css` at (0,1,0) — any hover/selected/focus rule on those tables with LOWER specificity now loses (check `.facts-matrix-cell:hover`, `.coverage-row:hover`, `.spolka-kpi-thread`); light theme contrast of muted text on the tinted row.
- G. Retired-key pins: are `Open document` / `Open source document` truly dead everywhere incl. `wiki/`, live specs, `App.test`? Any `text("Reporting periods")` left? `translationCompleteness` parity for the new keys in `plText` vs `en.ts`/`pl.ts`.
- H. Deep-link contract: Claims `aria-current` exactly one; Documents `scrollIntoView` loop when `groups`/`expandedFolds` change; `highlightDocumentRef` retarget; close clears (prop lifecycle at the tool host).
- I. Anti-archaeology and doc drift: comments state live constraints only; ADR/IA/wiki sentences match the shipped behavior (e.g. wiki/company-view.md period tables paragraph vs the 60vh scroller); `docs/ui-authoring.md` density row.
- J. Tests that assert defects or are vacuous (e.g. zebra parity tests in jsdom, `fundamentals-sticky.spec` probes, the `expectActionInsideScroller` self-test), and gaps: no test for the palette after migration on every consumer path (`App.test.tsx` cases 537-758)?

Evidence you may check: `make check-local` CHECK_LOCAL_EXIT=0 on `569769f4`-equivalent code; Playwright named specs 5 projects 225/230 → the 5 reds were the new zebra guard catching a regression fixed in `95b8c10b`, re-run 40/40; `make check-visual` 54/54; CI 21/21 on `18b6e73f`; live drive on the owner's Windows build: see the PR #468 comments (may be pending).

Deliver: numbered findings, blockers first, each with file:line evidence and the concrete fix; then a verdict `SHIP / SHIP-WITH-FIXES / DO-NOT-SHIP`. Be adversarial; do not pad with praise.
