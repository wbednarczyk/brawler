# Retrospective — Epic #410 Frontend v2 (usability + design language overhaul), PROGRAM level

Status: **shipped — nine waves merged plus the closing wave (#483 → `a2acfe30`, 2026-09-09); epic closure pending the owner's sign-off.** Kickoff 2026-08-19 (owner), target close 2026-09-08. Scope per epic body: UX + visual language together, Inbox-first, #228's open children absorbed into F3, detail pane = compact + company context (no article scraping). Deliberately out of scope: #254, #48, #46, #43, #175, #121.

Source retros read in full: F1 (v0.72), F2 (v0.73), F3a (v0.74), F4a (v0.77), F4b (v0.78), F4c (v0.79), F3c (v0.80), F3d (v0.81), dogfooding wave (2026-09, PR #468). All nine files existed — none missing. Also read: epic #410 body, ADR 0104 (+ its four dated amendments), `docs/retros/TEMPLATE.md`.

## 1. Status — what shipped per wave

| Wave | Version | PR | Headline |
| --- | --- | --- | --- |
| F1 — Inbox v2 | v0.72.0 | #423 | Per-kind detail pane, in-app filing text from `body_text`, dead `Komunikat ESPI/EBI` literal killed |
| F2 — Dziś v2 | v0.73.0 | #426 | Static tile grid → per-day decision queue, last-visit delta header, Claims leg, briefing strip removed |
| F3a — Widok Spółka | v0.74.0 (+ fixes → v0.75.0 in #437) | — | Dockview-free company panel/layout model; cockpit frozen; two owner dogfooding rounds fixed post-merge |
| F4a — Library screens | v0.77.0 | #439 | Companies language pass, Watchlists redesign, Alerts split; `ActionButton`/`EmptyState kind`/`Figure` guardrails |
| F4b — Transcripts + Events | v0.78.0 | #447 | Two mockup-first redesigns + Sources/Report-season language pass; both joined the Library nav |
| F4c — Settings + Research | v0.79.0 | #452 | Product-language pass, Research into Library nav, global Notebooks/Journal retired (ADR 0108/0071 amendments) |
| F3c — keyboard + a11y | v0.80.0 | #456 | Spółka workshop bar → APG toolbar, one focus ring, `Modal` focus-restore contract; rides #450 fix |
| F3d — Aktywność | v0.81.0 | #459 | Activity center: `job_runs` ledger, direct-activity registry, startup reconciliation, topbar indicator + panel |
| Dogfooding fix wave | v0.82.0 (merged `d29b338e`) | #468 | 13 owner findings across F4c/F3d fixed in one PR; measuring-pass rework for Fundamentals tables |
| Closing wave | merged `a2acfe30` 2026-09-09 (`release:minor`) | #483 | #469 GlobalSearch on the shared combobox, #454 `Open tool:` palette family, #478 Kanał, #476 paint-proven document marks, #451 Backups in Settings; closure-audit fixes (Activity `open` verb, indicator pin, ADR 0107 dec. 2/5) |

Not retro'd separately: F0 (#411, audit + design language) and F0.5 (#412, token repaint) — both closed, no retro file requested/found for them.

## 2. App domain

### Design-language decisions that held across waves (ADR 0104)

- **Decision 1 (accent discipline)** and **Decision 5 (detail shrinks to content)** — no wave reported a violation or reversal; F1's per-kind detail pane is decision 5's first instance and stood unchanged.
- **Decision 3 (verb dictionary)** — amended four times as new screens landed (2026-08-27 destinations-are-nouns; 2026-08-28 F4a `create`/`rename`/`pause`/`resume`; 2026-08-30 F4b `edit`/`confirm`/`reject`; 2026-09-02 F4c `link`/`snooze`; 2026-09-08 `#454` palette-family colon convention). Living document, not a one-shot spec — every screen wave added verbs rather than reopening the model.
- **Decision 2 (typography)** — amended once, same week as adoption (2026-08-27): mono broke on figures/dates ("15 , 2 mld PLN"), fixed to lining Schibsted Grotesk for numerics. Caught by owner dogfooding, not by a gate.
- **Decision 7 (provenance thread)** — amended once (2026-09-04): thread + ticket unified into one control after the live app showed a non-interactive-looking dotted underline. #482 tracks rollout to `ProvenanceFigure`.

### Founding complaints (epic body) — closure status

| Complaint | Status | Closed at |
| --- | --- | --- |
| Inbox detail pane: half-screen void for media, dead `Komunikat ESPI/EBI` literal | **closed** | F1 (v0.72), per-kind detail pane + literal killed at every render path |
| Cockpit: three vocabularies for one action (apply preset / show panel / open panel), unguessable preset↔save↔reset | **closed** | F3a (cockpit frozen, replaced by Spółka panel model) + ADR 0104 dec. 3 amendment 2026-08-27 (destinations are nouns, not verbs) |
| Dziś duplicates its own list as stat tiles, answers no "what now" | **closed** | F2 (v0.73), rebuilt as per-day decision queue with last-visit delta |

### Scope findings recorded (not silent)

- F2: non-arrival witness breadth deliberately kept F2-out → **#427, still open**.
- F3a: Research seam consumers under-inventoried on first pass (3/9 panels had forms, one dirty-handle); fixed same wave.
- F4a: two CSS layout bugs found and fixed in-wave (`grid-auto-rows` clip, dead `@media` viewport rule for a narrow pane).
- F4c: global Notebooks/Journal retired on measured use (7 notes / 2 journal entries in three months) — ADR 0108/0071 amendments, `bad-ideas.md`.
- Dogfooding wave: the "8 periods max" cap from sol's F3d-era planning round was reversed by the owner on the real binary — table now fills available width.

### Still-open (app), by source

- **#480** — 15 bare `EmptyState` usages still lack invitation/quiet `kind` (ADR 0104 dec. 4).
- **#481** — Documents tool rows: human title first still pending (ADR 0104 dec. 6).
- **#482** — `ProvenanceFigure` one-control thread model rollout (ADR 0104 dec. 7 amendment).
- **#427** — narrow the non-arrival witness to classified periodic documents (F2).
- **#450** — closed; **#451** — Backups behind the developer gate → closed by #483 (Settings › Data storage).
- **#454** — palette label collision → closed by #483 (`Open tool: <Name>` family).
- **#458** — re-arm stuck pending runs + transactional history-sweep create+enqueue (F3d).
- **#469** — `GlobalSearch` on the shared combobox controller → closed by #483.
- F1: report primary navigates to workspace not KPI-read flow (no `reportDocumentId` on feed items) — no issue found, candidate for #86/#354.
- F1/F2: corpus fidelity for composed read models pins primitives only; nested-shape parity is hand-synced mock code — carried across two waves, no issue filed.
- F3a: "tezy" vs "obietnice" (claims vs promises) terminology inconsistent app-wide (~30 sites) — owner decision pending, no issue found.
- F4b: Events layers (macro/holidays/whole-market, ADR 0058) confirmed not built; IA now says so honestly — owned by pre-existing #46.
- F3c: dogfooding ledger `docs/retros/dogfooding-v0.79-findings.md` (9 items + 2 decisions) — became the source for the #468 dogfooding wave (now closed by that PR).
- Dogfooding wave: fixed sticky KPI-label column width (140/110px, ellipsis+tooltip) and Δ/Trend columns folding at narrow width — both accepted, no card.
- **F3b (dockview 7 migration, #216)** — closed on GitHub, but its own body describes only the dependabot-major-bump incident, not a design verdict; F3a's retro explicitly frames itself as "evidence the main surface doesn't need the engine," suggesting F3b as originally scoped in the epic body was superseded rather than executed. **Worth a one-line confirmation from the owner that F3b is intentionally subsumed, not silently dropped** — it is named as a discrete sub-epic in #410's body.

## 3. Development-loop domain — recurring patterns (9 retros)

| Pattern | Occurrences | Waves |
| --- | --- | --- |
| Sol round finds a class the green local gates had already legitimized | **9/9** | every wave, worded explicitly each time (F1: 9 findings; F2: 4 production bugs; F3a: 12 defect classes; F4a: 24 findings/3 rounds; F4b: 9+5 findings; F4c: wrong-company routing pre-code; F3c: 6 rounds all paid off; F3d: async-race + panic-path leaks; dogfooding: 4 rounds, cache/measurement classes) |
| Live/real-data drive catches what unit + browser + visual gates cannot | 5 | F3a (10s latency, non-pinned toolbar — neither mock nor contact-sheet showed it), F4b (2 live PL bugs), F4c (English-in-Polish, clipping, dirty-guard — "3 of this wave's defects were live-only"), F3c (live-cycle green pre-merge), dogfooding wave (a11y combobox-label defect found live before sol saw it) |
| Agent verified only on its own fixture/scenario, not the default/base case | 3 | F3a (`list_claims_to_verify` mock not scoped per company, only caught on a screenshot), F3d ("scoped nextest ≠ full" — S1's green hid 2 real failures), dogfooding wave (all 3 slice agents green on their built scenario; default smoke company rendered an *empty* matrix) |
| Agent reports "waiting" while a task is actually stuck or still mutating files | 3 | F1 (S5 looped on "waiting for background Playwright," took over after 2nd empty return), F4a (2 agents "waiting for a notification," one kept editing after its "holding" report), F4c ("a reporting agent that says waiting is still alive — `TaskStop` before touching its files") |
| Codex/sol quota or resume mechanics interrupt a review round | 3 | F4c (usage limit cut plan review mid-flight), F3d (`--resume` picked a junk session; R3 died on usage limit), dogfooding wave (limits interrupted R1/R2/R4; a sleep-scheduled retry did not survive the night) |
| Destructive git op used despite a standing ban | 2 | F3a (2 subagents `git stash` despite contract ban — lesson: ban must be line 1 of the prompt), F3c (orchestrator's own `git checkout -- <file>` during a red-proof lost a fix, caught by the next test run) |
| Shared-component / seam consumers not inventoried before a change | 2 | F3a ("seam defined but consumers not inventoried" — dirty-tracking covered 3/9 panels), F4c (`EvidenceRow` relabel broke the Decision journal J6, escaped every slice gate to CI; guardrail: `repoctx rdeps` before relabeling a shared component) |
| English leaking into Polish UI, undetected by the dev-speak guard | 2 | F4b (`useLocale()` silently defaulted to EN, masked by the test's own provider), F4c (License tab, source-status chip composed without a locale — "no English-in-PL detector" noted as a still-open gap) |
| File-size ratchet gamed instead of honored | 1 gamed / 1 honored | F2 (AppStateRoot extraction ratcheted the pin correctly *down*), F3d (sonnet paired JSX props to dodge the same pin — reverted, pin raised by hand, briefs now say "extract, never pair lines") |
| Stale visual baseline slips through under `maxDiffPixelRatio` | 1 explicit + 1 related | F4b (explicit: "the rm-first rule was in the doc and still skipped"), F3a (related: agent judged collapsed 1000px cards "correct" — screenshot review made mandatory as a result) |
| `th`/CSS-specificity rule losing to a later class, same bug shape repeated within one wave | 1 (×3 internally) | dogfooding wave — alignment, padding, and zebra striping all broke the same way; called out by the retro itself as "the same class each time," with no lint fix (would flag legitimate code), left as a human-checklist item |

### What to keep

- Mockup-first on real owner data, approved before implementation (routine by F3a onward).
- Sol as the standing outer loop on both plan and diff — zero waves where it found nothing; treat continuing this as load-bearing, not optional.
- Disjoint-file parallel subagent slices with per-slice red-first contracts and per-slice `check-local`.
- Wave-based fix rounds with per-finding FIXED/NOT-FIXED re-verification (F2 onward) — converges reliably in 3–4 rounds.
- Owner dogfooding on the PR binary between review rounds (dogfooding wave) caught two design corrections pre-merge instead of a follow-up wave.

### What to stop

- Trusting a subagent's "green" without checking it ran the *unscoped*/*default* case (3 recurrences, same root shape each time: fixture-shaped verification).
- Treating an agent's "waiting for X" self-report as ground truth without `TaskStop` + a direct check (3 recurrences).
- Scheduling long `sleep`-based retries across a Codex quota reset that must survive a machine sleep (dogfooding wave: explicitly did not survive).
- Chaining a destructive git op into a compound command, even by the orchestrator itself (F3c: happened to the orchestrator, not just subagents — the "ban must be first line" lesson from F3a evidently didn't fully generalize).

### What to start

- Put "render the DEFAULT smoke scenario and read the PNG" into every layout slice's brief as a mandatory line (proposed in the dogfooding-wave retro; not yet promoted to a standing template line).
- `repoctx rdeps` step as a standing line in any contract that touches a shared component (proposed in F4c; should be added to the slice-contract template rather than re-invented per wave).
- Add the live-drive run to every UI wave's DoD *before* the first sol diff round, not after CI (proposed independently in F3a and F4c — said twice, not yet a gate).
- Reserve a Codex-quota buffer before the last review round of a wave (F3d, dogfooding wave both lost time to this).

## 4. Guardrails harvested (deduplicated, canonical file)

| Guardrail | Canonical file | First harvested |
| --- | --- | --- |
| Spawn-time `TZ=Europe/Warsaw` pin for vitest (config/setup pinning is a no-op under ICU) | `package.json` vitest scripts + `vitest.config.ts` anchor comment | F2 |
| Regenerated visual-baseline target files excluded from sibling filename-drift detection | `scripts/ux/visual-update-core.mjs` | F2 |
| Polish-letter `\b` word-boundary trap documented | `scripts/check/docs-drift` STRUCTURAL_MAPPINGS | F2 |
| Drain microtasks before clearing mocks (order-flake root cause) | `TodayScreen.test.tsx` `afterEach` | F2 |
| Canonical J1 journey walks the real morning scan, not a truncated one | `tests/browser/journeys/budgets.json` | F2 |
| File-size ratchet moves down honestly; briefs say "extract, never pair lines" to dodge it | AppStateRoot extraction pin + brief language | F2 (honored), F3d (re-enforced after gaming) |
| App-level controller hooks take `locale` as an explicit input; `renderApp`-level PL regression test | `ui-authoring.md` § i18n | F4b |
| Every visual-catalog state maps to a distinct, non-aliased baseline file | `visualUpdateCore.test.ts` | F4b |
| Consumer-level guard fails on a vacuous/no-op call | `uxContracts.expectPhrasingOnlyExpandableRows` | F4b |
| Text-fit check, width AND height, opt-in | `expectTextFits` + `data-ux-text-fit` | F4b |
| Mock enum values constrained to Rust-validated sets, incl. mutation outputs | `vocabulary.test.ts` | F4b |
| Dev-speak/English guard extended vocabulary + literal-anywhere allowlist proof | `src/shared/locale/devSpeakContracts.test.ts` | F4c |
| Retired-screen vocabulary pinned both directions (present-nowhere) | `src/shared/locale/retiredKeys.test.ts` + `docs/retired-surface.json` | F4c |
| Palette-opening specs go through blur + retry (recurring flake class) | `tests/browser/helpers/harness.ts` `openPalette` | F4c (first seen #432, recurred here) |
| Bare-checkout imports in scripts banned | `scripts/check/gate-integrity.mjs` rule 2c | F4c (from #449) |
| No prettier in contracts; `repoctx rdeps` before relabeling a shared component | contract template line (`f4c-common.md`, local) | F4c |
| No-`autoFocus` app-wide ban; global focus-ring stylelint ban on `outline: none` | `src/ui/noAutoFocus.test.ts` + stylelint rule + gallery focus walk + keyboard reachability/no-trap spec | F3c |
| Multi-host screens must load on mount (the #450 refresh-effect class) | `ui-authoring.md` | F3c |
| Every registered activity kind resolves an identity | `jobs/activity_identity` | F3d |
| Unwrapped job cores only callable from their direct wrapper/queue handler/tests | `jobs/activity_awaited_paths` | F3d |
| Seed transactions must be IMMEDIATE, never DEFERRED | `no_write_transaction_is_deferred` + brief language | F3d |
| One-modal shortcut policy, exhaustive per-id test | `shortcuts.test.tsx` | F3d |
| Job-kind label parity with the kind list | `labels.test` (`formatJobKindDisplayName`) | F3d |
| Measured/sticky table geometry checked under the DEFAULT smoke scenario, not just the feature scenario | `docs/engineering-workflow.md` DoD §B | dogfooding wave |
| Text stays ≥6px inset in every cell of the four data tables | `tests/browser/table-text-inset.spec.ts` + `expectTableCellTextInset` | dogfooding wave |
| Table capacity oracle independent of rendered/stretched cells; older-widest fixture; delayed-webfont case | `tests/browser/fundamentals-periods.spec.ts` | dogfooding wave |
| Sticky header pinned to tool body (collapsed) / bounded box (expanded) | `tests/browser/fundamentals-sticky.spec.ts` | dogfooding wave |
| Destination-action scroll owner is the modal body | `tests/browser/activity.spec.ts` | dogfooding wave |
| Combobox keeps its accessible name while open even inside a wrapping `<label>` | `src/ui/primitives.test.tsx` | dogfooding wave |
| Measuring-pass cache invalidates on signature/tier-variant/webfont change | `periodMeasureKeys.test.ts`, `useVisiblePeriods.test.tsx` | dogfooding wave |
| Two new gate classes (painted-extent overlap, budgeted J1b journey) | not itemized by file in the F1 retro | F1 |
| Seam-consumer inventory as a parametrized test (`toolPrimaries`, `paneLandmarks`, dirty-handles per panel) | F3a contract tests | F3a |
| `make disk-clean` removes `target/debug` before it blocks a gate | disk-guard fix | F3a |

## 5. Escaped defects — aggregate

Only 4 of 9 waves used the ADR 0081 Q7 marked table (F2, F4c, F3d, dogfooding wave) — 27 rows total. F1, F3a, F4a, F4b, F3c recorded defects narratively but not in the parseable format (valid per the template's own "historical retros with no marked table stay valid" rule, but it means this aggregate under-counts real defects from over half the program).

**By origin class** (27 rows): spec-gap 8 · responsive-layout 4 · integration-seam 5 · visual-hierarchy 3 · async-race 2 · test-flake 2 · ux-decision 1 · missing-state 1 · real-data-shape 1

**By detection stage**: mid-milestone 13 · full-gate 5 · vertical-slice 5 · implementation 2 · release-dogfood 2

Reading: `spec-gap` (a contract/plan silently omitted a behavior) and `responsive-layout`/`visual-hierarchy` (real-data or real-width rendering) dominate, and together with `integration-seam` account for 20 of 27 rows (74%) — consistent with the loop-pattern finding above: fixture-shaped verification and under-specified contracts, not raw logic bugs, are this program's main defect source. Only 2 `release-dogfood`-stage rows exist in the whole marked sample (both dogfooding wave) — most escapes were caught mid-milestone or at full-gate, before the owner ever saw them, which is the intended shape of the loop.

## 6. UX — journeys

| Journey | Wave | Budget (ceiling) | Measured | Prior | Δ |
| --- | --- | --- | --- | --- | --- |
| J1 — morning review | F2 | ≤15 | 6/5/1/1 | 2/2/1/1 | longer — redefined to canonical scope (prior floor was a truncated walk skipping the Claims leg); not a regression |
| J3 — (Spółka ownership/accountability) | F3a | — | +1 interaction (tool hop) | — | longer, open for owner decision |
| J7 — weekly review | F3a | — | 9→13 within-wave | — | longer, cause not resolved in-wave |
| J7 — weekly review | F4c | ≤9 | 8 | 11 | shorter (Research leg via Library nav instead of a ⌘K hop) |
| J8 — keyboard-only Spółka pass (new) | F3c | ≤14 | 12 | — (new) | new journey, floor 13 |

Note: J7's numbers are inconsistent across F3a (ended the wave at 13) and F4c (prior column reads 11) — likely a rebaseline or scope redefinition between waves that the retros don't cross-reference each other on. Not resolved here; flag for the owner rather than silently reconciled.

**Still-open UX items across the program**
- Owner has never run the shipped Dziś queue on live data end-to-end for the delta header's real usefulness (F2).
- J3 +1 interaction and watchlist rows not navigating to Spółka — both open owner decisions since F3a (v0.74), unresolved through v0.81.
- Research panel fixed max-width at 1440px (half the window empty on wide screens) — noted pre-existing in F4c, not touched since.
- Fixed 140/110px KPI label column ellipsizes; Δ/Trend columns fold at narrow width with no reveal — both accepted without a card (dogfooding wave).
- Epic's own umbrella verification criterion — "owner dogfooding zero-P1 after F1 and F3" — was **not met on the first pass for F3a**: the first F3a dogfooding round produced 2 P1 findings, which is exactly why the F3a retro records two extra post-merge fix PRs (`fix/spolka-view-latency`, `fix/spolka-dogfooding-ux`) plus two more re-dogfooding waves before #437 closed it out.

## 7. Net

The program's single strongest and most consistent pattern, repeated in all nine retros without exception, is that the adversarial sol loop (plan rounds, then diff rounds) caught production-real defects that every local gate — unit, browser, visual, CI — had already passed; live-drive on the owner's real database repeated that role for a smaller but still nontrivial set of defects five separate times. The weakest and most repeated failure mode is the mirror image: agents verifying "green" against a fixture they built for the feature rather than the app's actual default state (F3a, F3d, and the dogfooding wave all name this exact shape independently, and the dogfooding wave's own integration day repeated a fourth variant of it — the same CSS-specificity bug three times in one PR). All three founding complaints from the epic kickoff are genuinely closed with a named wave to point at. The design-language decisions (ADR 0104) held up as a living document rather than a one-shot spec, picking up five amendments across the program without ever being reopened wholesale. Genuinely still open and tracked: #480/#481/#482 (design-language rollout gaps), #427/#451/#454/#458/#469 (per-wave carry-overs), the J3/J7 watchlist-navigation and interaction-count questions with no owner decision yet, and one unconfirmed scope question — whether F3b (#216, dockview 7 migration) was a deliberate casualty of F3a's dockview-free result or a silently dropped sub-epic; the epic body still lists it as a discrete deliverable.
