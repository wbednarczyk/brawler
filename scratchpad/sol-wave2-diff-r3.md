Re-verified head `78d2844ddb0c0999b54c022ce89f027906aeed8a`. No files changed; only the existing untracked `scratchpad/` remains.

| R2 item | Status | Evidence |
|---|---|---|
| Activity live-spec blocker | **FIXED** | The live spec now requires exactly one `open` action per row at [f3d-activity.live.spec.ts:74](/home/wojtas/projects/brawler/tests/live/f3d-activity.live.spec.ts:74), matching `ActivityPanel`’s `verb="open"` contract. |
| Finding 5: false Backup facts | **FIXED** | `InfoGrid` is rendered only when `status` exists at [BackupsSettings.tsx:86](/home/wojtas/projects/brawler/src/screens/Settings/BackupsSettings.tsx:86). Loading and initial-failure coverage pins the absence of facts/invitation at [BackupsSettings.test.tsx:187](/home/wojtas/projects/brawler/src/screens/Settings/BackupsSettings.test.tsx:187) and [line 197](/home/wojtas/projects/brawler/src/screens/Settings/BackupsSettings.test.tsx:197). Last-known-good refresh behavior remains intact. |
| New finding 2: whitespace-only search | **FIXED** | Trimmed-empty input now calls `controller.reset(value)` instead of opening at [GlobalSearch.tsx:155](/home/wojtas/projects/brawler/src/app/GlobalSearch.tsx:155). The test types spaces and verifies closed ARIA/listbox state plus an unconsumed Escape at [GlobalSearch.test.tsx:448](/home/wojtas/projects/brawler/src/app/GlobalSearch.test.tsx:448). |
| Finding 8: docs/comments sweep | **NOT-FIXED** | The substantive wiki and route-copy corrections landed, but residual stale references remain. |

Remaining Finding 8 cleanup:

1. [docs/ui-information-architecture.md:16](/home/wojtas/projects/brawler/docs/ui-information-architecture.md:16) and [docs/ui-authoring.md:210](/home/wojtas/projects/brawler/docs/ui-authoring.md:210) still point to the nonexistent `paletteCopy.test.ts`; the file is `.tsx`.

2. [density-notebook-claims.spec.ts:22](/home/wojtas/projects/brawler/tests/browser/density-notebook-claims.spec.ts:22) and [visual-notebook-claims.spec.ts:13](/home/wojtas/projects/brawler/tests/browser/visual/visual-notebook-claims.spec.ts:13) still claim the WorkshopBar label is `Open <tool>`, contradicting the noun-only destination contract.

3. [journeys/budgets.json:49](/home/wojtas/projects/brawler/tests/browser/journeys/budgets.json:49) still records the J6 route as `Spółka → Otwórz dziennik decyzji` instead of the noun-labeled workshop path.

Concrete fix: update the two test-file references to `.tsx`, describe WorkshopBar labels as nouns, and rewrite the J6 budget comment to `Spółka → Dziennik decyzji`.

No new runtime or blocker findings.

**Verdict: SHIP-WITH-FIXES.** The remaining work is a small but required canonical-doc/test-comment cleanup.

Codex session ID: 01a08014-867f-79d1-8e5a-5fc0d163a550
Resume in Codex: codex resume 01a08014-867f-79d1-8e5a-5fc0d163a550
