# Frontend test-responsibility inventory (Q8, ADR 0081)

Audit ledger for [Q8 `66621c9`](ux-quality-loop-v2.md#q8--frontend-test-responsibility-and-integration-seams): classify each full-app Vitest test by the **cheapest authoritative layer** and record keep/move/split decisions. Scope: the 19 `appWorkflowHarness` importers + the browser families. Companion: [testing.md § Frontend test responsibilities](../testing.md#frontend-test-responsibilities).

**Guardrails honored.** No behavior coverage was deleted; `coverage-baseline.json` is unchanged (this pass only *added* tests and removed one always-skipped browser spec that never counted). Per the Q8 STOP-AND-ASK, a cross-screen case is **moved only** when a cheaper layer is genuinely authoritative — not speculatively, and not without measured flake. Where the browser is not yet cheaper (or lacks the seed/back end), the row says so honestly rather than migrating for the ledger's sake.

Categories: `component-state` (controlled state/forms/error/async within one screen), `workflow` (multi-step, one screen), `cross-screen` (acts in A, asserts B appears), `layout`, `a11y`, `pure-logic` (no render), `seam` (ordering/stale-response).

## Ledger

| Test file / behavior | Current layer | Authoritative layer | Keep / move / split | Replacement proof | Targeted runtime before/after |
| --- | --- | --- | --- | --- | --- |
| `useResearchController` request-version seam (#9) | was skipped Playwright | **component (renderHook)** | **moved** | `src/app/useResearchController.controlledAsync.test.ts` (green); skipped `tests/browser/research-controlled-async.spec.ts` removed | n/a (browser case never ran) |
| `ResearchScreen.test.tsx` — evidence → Inbox / Dashboard (4 cross-screen) | full-app Vitest | browser for the journey; **component for the routing branch** | **split** (done in epic c793ca1) | browser: `cockpit-view-company.spec.ts` (evidence preset follows company) + `journeys/`; component branches retained in-file (Inbox scope, Dashboard evidence preset) | not measured (no flake) |
| `ResearchScreen.test.tsx` — question/brief/reminder/timeline/forms/error | full-app Vitest | **component** | **keep** | in-file | — |
| `GlobalSearch.test.tsx` — 3 cross-screen nav | full-app Vitest | browser *would* be authoritative, **but blocked**: the browser mock runtime has **no search back end** (`search_*` unhandled) and the Vitest cases rely on mock-injected `searchResponse`, which the browser layer cannot replicate today | **keep** (documented blocker) | in-file grouping/highlight/input-state is genuine component proof; cross-screen nav re-uses `navigateToSearchResult`, itself browser-covered by `journeys/` | — · unblocks with a runtime `search` handler (NS1) |
| `App.test.tsx` — shell nav walks (~9) | full-app Vitest | **browser** (`smoke-walk.spec.ts` already walks every primary screen) | **keep** (browser already owns the broad walk; retain isolated shortcut/palette/state per plan) | `tests/browser/smoke-walk.spec.ts`; keep in-file shortcut/palette/count state | — |
| `App.uiGuardrails.test.tsx` — dev-speak wording per section | full-app Vitest | **component** (mounts each section directly; asserts wording, not nav) | **keep** | in-file | — |
| `screens.a11y.test.tsx` — axe per screen | full-app Vitest | **component** (unique fast structural axe) | **keep** | in-file (real-browser contrast is a deferred `axe-playwright` follow-up, not this pass) | — |
| `CockpitScreen.test.tsx` — palette/company/open-panel/Dockview (21) | full-app Vitest | **component** (host screens are *panels*, not navigation) + browser for real Dockview render | **keep** (+ browser `cockpit-view-company.spec.ts` owns real panel-body render) | in-file + browser | — |
| `CockpitScreen.test.tsx` — 2 descriptor round-trips | Vitest | **pure-logic** | **keep** | in-file | — |
| `EventsScreen.test.tsx` — 2 `resolvePreference` | Vitest | **pure-logic** | **keep** | in-file | — |
| `InboxScreen.test.tsx` — 5 cross-screen (→Cockpit/Notebooks/Companies/Sources) | full-app Vitest | browser for the journey; component for the empty-state routing | **keep** (primary flows browser-covered by `journeys/`; retain component routing + the 22 component-state cases) | `journeys/` + in-file | not measured (no flake) |
| `CompaniesScreen.test.tsx` — 4 cross-screen (→Watchlists/Cockpit) | full-app Vitest | browser for the journey | **keep** (retain form/lookup component-state; journey covered by `journeys/` + `shell.spec.ts`) | `journeys/` + in-file | not measured |
| `TodayScreen.test.tsx` — 2 cross-screen (Show-all/Review → Inbox) | full-app Vitest | browser + live checkpoint | **keep** (retain stream/merge/counts/undo component-state; Review→cockpit is a Q6 live-checkpoint mechanical step) | `ux-checkpoint.live.spec.ts` + in-file | not measured |
| `SourcesScreen.test.tsx` — 1 cross-screen (→Diagnostics) | full-app Vitest | browser | **keep** (retain refresh/poll/registry component-state) | in-file | not measured |
| `Notebooks / Settings / McpSettings / License / CompanySettingsManager / Diagnostics / Transcripts / Watchlists` | full-app Vitest | **component** | **keep** (controlled state/forms/error; correctly placed — the plan says optimize only with measured evidence) | in-file | — |

## Findings

1. **The clean authoritative-layer moves were the two with no cheaper equivalent surface**: the request-version seam (a controller concern → renderHook) and the research evidence→screen routing (a journey → browser, done during the Dashboard redesign). Both are now proven where they can actually be observed.
2. **GlobalSearch cannot move yet** — the browser mock runtime has no `search` back end, and the Vitest suite's value *is* its mock-injected responses (grouping, `<mark>` highlight, input state). Migrating would mean building a runtime search handler first; tracked as an NS1-adjacent follow-up, not silently skipped.
3. **Most "cross-screen" Vitest cases are already browser-backed** by `tests/browser/journeys/` (the served-journey specs) and `smoke-walk.spec.ts`. They are retained in Vitest for fast component/routing coverage; per the Q8 STOP-AND-ASK they are **not** churned into browser duplicates absent measured flake — no flake was observed in this pass.
4. **No coverage was removed and the ratchet held**: this pass added `useResearchController.controlledAsync.test.ts`, `uxCheckpointEvidence.test.ts`, and browser assertions, and removed only an always-skipped browser spec.
5. **The Q2 browser controlled-async bridge (`tests/browser/helpers/mockRuntime.ts`) is live again.** Moving the #9 seam to the component layer briefly orphaned it (it was parked in `knip.json` `ignore` for the owner's decision). The **Q9 J1/J2 pilot now consumes it** — the controlled-async cases (J1 stale-read, J1 failed-category, J2 extraction-failure, J2 double-action) import `holdInvocation`/`releaseInvocation`/`rejectInvocation` — so the parking was removed and the file is a normal, imported helper again.

## Multi-slice contract update

The [experience-contract template](EXPERIENCE-CONTRACT-TEMPLATE.md) § 12 now requires a multi-slice task to name its component proof, cross-screen browser proof, integration-seam proof, consumer map, and full-wave verification before the final gate — so the layer split is decided in planning, not discovered at the gate.
