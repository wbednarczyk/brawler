Reviewed `d29b338e...ba1f6e2070e66dbef03adf9c4cc6a61f3795a567` in full. No files were changed.

No blocker-class defect found, but the following should be corrected before release:

1. **High — ADR 0107’s normative decisions still describe architecture that no longer exists.**  
   [ADR 0107:29](/home/wojtas/projects/brawler/docs/adr/0107-company-view-paradigm.md:29) declares a `company | namedView` route union, then its closure note admits `namedView` never existed after ADR 0108. [Decision 5](/home/wojtas/projects/brawler/docs/adr/0107-company-view-paradigm.md:40) likewise says named views, legacy dashboard rows, and dockview remain reachable, although ADR 0108 removed them. This fails the epic’s “decision with a live path” audit and leaves contradictory present-tense guidance.  
   **Fix:** rewrite Decision 2 to the actual `Section + companyId + Tool` composition and closed `Tool` union. Replace Decision 5 with the current retired state and an ADR 0108 pointer; do not leave the superseded behavior as normative text followed by a rebuttal.

2. **High — “Create backup” is not the section’s primary action in either state.**  
   [BackupsSettings.tsx:62](/home/wojtas/projects/brawler/src/screens/Settings/BackupsSettings.tsx:62) promises exactly one primary action, but the shared button at line 65 has neither `variant="primary"` nor `data-ux-primary-action="true"`. The inventory tests explicitly expect zero primaries in both the loaded and empty states at [SettingsScreen.contract.test.tsx:252](/home/wojtas/projects/brawler/src/screens/Settings/SettingsScreen.contract.test.tsx:252) and [line 279](/home/wojtas/projects/brawler/src/screens/Settings/SettingsScreen.contract.test.tsx:279). That contradicts the owner-card plan and single-primary requirement.  
   **Fix:** add the primary variant and marker to the shared `createButton`; change both inventories to expect exactly one primary and pin the variant/marker in the component test.

3. **Medium — GlobalSearch can be logically open while no popup exists, breaking Escape semantics and ARIA.**  
   The panel requires a nonblank query at [GlobalSearch.tsx:151](/home/wojtas/projects/brawler/src/app/GlobalSearch.tsx:151), but every focus calls `controller.open()` at [line 172](/home/wojtas/projects/brawler/src/app/GlobalSearch.tsx:172), while `aria-expanded` mirrors controller state at [line 167](/home/wojtas/projects/brawler/src/app/GlobalSearch.tsx:167). Focusing an empty search—or refocusing after Clear—therefore reports an expanded combobox with no listbox. The first Escape is consumed by `close-list` at [line 45](/home/wojtas/projects/brawler/src/app/GlobalSearch.tsx:45), although no visible list is open.  
   **Fix:** open on focus only when the trimmed query is nonempty, and derive exposed expanded/controls state from the visible panel. Add an empty Ctrl+F/clear regression asserting one Escape bubbles immediately.

4. **Medium — Async options are not atomically associated with the current query.**  
   Changing the query at [GlobalSearch.tsx:160](/home/wojtas/projects/brawler/src/app/GlobalSearch.tsx:160) renders before the effect marks the request searching at [line 102](/home/wojtas/projects/brawler/src/app/GlobalSearch.tsx:102). During that render, controller options at [line 92](/home/wojtas/projects/brawler/src/app/GlobalSearch.tsx:92) can still contain the preceding query’s result set. The existing test flushes effects and does not observe this transition.  
   **Fix:** key stored results by their source query and expose them only when it equals the current normalized query, or synchronously invalidate results in `onChange`. Cover it with deferred responses and an immediate Enter assertion.

5. **Medium — Backups renders a false empty state while status is unknown or failed.**  
   `status` starts as `null` at [BackupsSettings.tsx:11](/home/wojtas/projects/brawler/src/screens/Settings/BackupsSettings.tsx:11), which is treated as zero backups at [line 61](/home/wojtas/projects/brawler/src/screens/Settings/BackupsSettings.tsx:61). Consequently the three-beat “no backups yet” invitation renders before the initial request resolves and remains after an initial failure, misrepresenting unknown state as empty.  
   **Fix:** model `loading | loaded | error`; render the invitation only for a successfully loaded empty list. Preserve last-known-good data when refresh fails, and add pending/rejection tests.

6. **Medium — The #476 tests reuse a Locator, not the same DOM element.**  
   Both [spolka-documents.spec.ts:64](/home/wojtas/projects/brawler/tests/browser/spolka-documents.spec.ts:64) → [line 77](/home/wojtas/projects/brawler/tests/browser/spolka-documents.spec.ts:77) and [activity.spec.ts:77](/home/wojtas/projects/brawler/tests/browser/activity.spec.ts:77) → [line 104](/home/wojtas/projects/brawler/tests/browser/activity.spec.ts:104) re-evaluate the locator after navigation. A remounted replacement row would pass the claimed “same-element” proof. The current implementation probably preserves the node through batched same-company navigation, but the contract is not pinned.  
   **Fix:** retain the original `ElementHandle`, assert the post-navigation locator resolves to that exact node and that it remains connected, then obtain post-paint from that handle.

7. **Medium — The #454 regression gate does not enforce command families, and its Enter test never presses Enter.**  
   The palette gate accepts any translated command beginning with a generic verb, so a regression such as `Otwórz fundamenty` would pass. The disambiguation test named as an Enter test clicks the result at [paletteCopy.test.tsx:163](/home/wojtas/projects/brawler/src/app/paletteCopy.test.tsx:163).  
   **Fix:** validate prefixes by action-key family in both locales (`tool.open.*` → `Open tool:`/`Otwórz narzędzie:`, etc.). Assert the sole result is active through `aria-activedescendant`, then execute it with `{Enter}`.

8. **Medium — Canonical and user-facing documentation still exposes retired labels and routes.**  
   Notable drift includes:

   - Backups still located in Diagnostics: [docs/testing.md:1090](/home/wojtas/projects/brawler/docs/testing.md:1090), [docs/data-model.md:1793](/home/wojtas/projects/brawler/docs/data-model.md:1793).
   - Activity still described as a destination action: [ui-information-architecture.md:46](/home/wojtas/projects/brawler/docs/ui-information-architecture.md:46).
   - Old flat workshop wording remains at [ui-information-architecture.md:16](/home/wojtas/projects/brawler/docs/ui-information-architecture.md:16), [ux-journeys.md:38](/home/wojtas/projects/brawler/docs/ux-journeys.md:38), and several wiki pages.
   - [wiki/autopilot.md:14](/home/wojtas/projects/brawler/wiki/autopilot.md:14) now says `Open tool: Fundamentals`, but Autopilot editing has moved to Companies → Manage settings; this is a dead route, not merely stale wording.

   **Fix:** perform a docs-wide split between noun-only workshop destinations and explicit Ctrl+K command-family labels; update Backups to Settings → Data storage, Activity to one `open` action, and remove the obsolete Fundamentals Autopilot path.

The j1 same-row rebuttal is valid: the sole seeded claim is auto-highlighted, so an unmarked pre-state for that row is unreachable. I also found no regression in SearchField clear behavior/data hooks, the Feed→Kanał fan-out, restore confirmation semantics, timestamp formatting, retired-key coverage, or ActivityIndicator DTO/tone behavior.

**Verdict: SHIP-WITH-FIXES.**

Codex session ID: 01a08014-867f-79d1-8e5a-5fc0d163a550
Resume in Codex: codex resume 01a08014-867f-79d1-8e5a-5fc0d163a550
