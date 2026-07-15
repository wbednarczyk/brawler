# UX Quality Loop v2 — implementation plan

Epic: `bd1a6af` · Q0 `0bb4680` · Q1 `b899ec1` · Q2 `a9992e2` ·
Q3 `ca99420` · Q4 `d4a68c2` · Q5 `81313f0` · Q6 `a26cc6e` ·
Q7 `3b8f1df` · Q8 `66621c9` · Q9 `31a0fd5`

Status: **ready to implement after the active v0.52 worktree is closed**. This is
a cross-cutting process/test-architecture epic with no `milestone:*` label and no
version bump of its own. The plan is non-normative: ADRs and canonical docs win on
conflict ([plans README](README.md)).

Design ancestry: [ADR 0045](../adr/0045-guardrail-harvest-loop.md),
[ADR 0048](../adr/0048-test-architecture-sample-data-broad-clickable-coverage-and-layered-parallelism.md),
[ADR 0066](../adr/0066-live-drive-remote-debugging.md),
[ADR 0074](../adr/0074-ux-journeys-and-anti-rot.md), and
[ADR 0076](../adr/0076-ui-design-system-and-density-contracts.md). Q0 creates the
focused durable decision, ADR 0081.

## Goal and success condition

Brawler already tests code behavior, contracts, journeys, accessibility, density,
overflow, and pixel stability extensively. The remaining escaped frontend/UX defects
cluster where those tests do not decide quality: incomplete UX specifications,
discoverability and hierarchy, transitions between async states, hostile real-content
shapes, integration seams, and owner validation that happens only near release.

This epic adds an **UX decision-validation loop**, not another generic test layer:

1. specify the human outcome before component work;
2. enumerate states and interaction frames before implementation;
3. write the first journey-level red test before the lower layers;
4. exercise hostile content and controlled async ordering on the one mock runtime;
5. make visual review cheap through a contact sheet;
6. inspect the real Windows app after the first vertical slice and mid-milestone;
7. classify escaped defects so planning failures improve planning rather than
   reflexively producing noisy tests;
8. prove the whole loop on J1 and J2 before making any new rule universal.

Success is not “more tests.” Success is a J1/J2 pilot in which defects are found at
the experience-contract, adversarial-scenario, contact-sheet, or early-live stage
instead of first appearing at the final gate or owner release walk. The existing
`make check` remains deterministic and in the seconds-to-low-minutes class.

## Start preconditions

Do not start Q0 implementation while the current v0.52 worktree is active. It already
modifies `Makefile`, `package.json`, `docs/testing.md`,
`docs/engineering-workflow.md`, `docs/ux-journeys.md`, the canonical scenario runtime,
the browser adapter, and J1/J2-adjacent behavior. Starting this epic on top would mix
two architecture changes and invalidate the planned red baselines.

Before moving Q0 to `state:in-progress`:

- close or checkpoint v0.52 and begin from a clean/reviewable worktree;
- run `rtk repoctx changed --since HEAD` and re-check every file named below;
- verify the v0.52 J2 expectation-vs-actual step has landed before editing J2;
- inspect `rad issue show 0db7a7a`; Q2 needs its mock failure-injection seam, not
  necessarily its independent real-DB evaluator;
- run and record warm baselines for targeted Vitest, J1/J2 Playwright, all visual
  projects, and the full gate. Timing is comparison evidence, never a hard wall-clock
  threshold.

## Locked boundaries

These choices are pre-decided for the implementation sessions. Q0 records them in ADR
0081; it does not reopen them unless repository reality contradicts the premise.

1. **One runtime.** `src/test/scenarios/runtime.ts` stays the only command router for
   Vitest and Playwright. No second chaos router, browser-only business handler, or
   alternate scenario factory.
2. **Base scenarios plus overlays.** `empty | minimal | rich` remain the base set.
   Hostile/dense/partial/stale/conflicting/mixed-locale data are composable overlays,
   not six new mega-scenarios.
3. **Existing failure ownership.** Epic `0db7a7a` owns generic command/job failure
   injection and poor-state/real-state foundations. Q2 consumes its settlement seam;
   it does not reproduce it.
4. **Three proof classes.** Deterministic behavior/layout/affordance contracts may
   hard-fail. Contact sheets and timing reports are mandatory review evidence but do
   not grade visual taste. Clarity/usefulness/trust remain explicit human verdicts.
5. **No wall-clock UX hard gate.** Feedback latency is recorded and controlled async
   proves an immediate pending state, but machine-load-sensitive milliseconds never
   block `make check`.
6. **No new Playwright project cross-product.** Reuse the current viewport/theme and
   visual projects. Adversarial variants are selected scenarios inside targeted specs,
   not multiplied across every project by default.
7. **No universal rule before the pilot.** Q1–Q8 build pilot infrastructure. Q9 ends
   with an adopt/revise/reject verdict per practice and owner sign-off; only adopted
   items then enter the universal DoD/gate.
8. **No silent product redesign.** The pilot may fix an in-scope defect needed to
   complete J1/J2. A new information architecture or functional redesign becomes a
   separately approved product card/mockup.
9. **Private evidence stays private.** Real-database screenshots/manifests live under
   gitignored `test-results/`; public docs and Radicle receive only non-sensitive
   metadata and verdicts.
10. **No release ritual.** Epic closure runs `make check-epic`, a retro, spec audit,
    and owner sign-off, but no version bump/release unless the pilot separately ships
    product behavior.

## Canonical ownership map

| Concern | Canonical owner after Q0 |
| --- | --- |
| durable rationale; hard/advisory/human boundary; pilot rollout | ADR 0081 |
| experience contract, storyboard/state matrix, discoverability authoring | `docs/ui-authoring.md` |
| mock scenarios, journey metrics, layer ownership, contact-sheet mechanics | `docs/testing.md` |
| first-slice, mid-milestone, and release exploratory checkpoints | `docs/dogfooding.md` |
| short universal handover checks after Q9 adoption | `docs/engineering-workflow.md` |
| escape taxonomy and per-retro record shape | tracked `docs/retros/TEMPLATE.md` |
| task-level execution detail | this plan |
| live status and blockers | Radicle/Radboard |

`docs/retros/` is currently ignored wholesale. Q0 changes `.gitignore` to:

```gitignore
docs/retros/*
!docs/retros/TEMPLATE.md
```

Only the canonical template becomes tracked; owner retros and real-use evidence remain
local unless the owner deliberately changes that policy later.

## Wave plan and ownership

Maximum three independent implementation slices at once. Do not overlap tasks that
edit the journey harness or J1/J2 specs. Run one heavy build/test command at a time.

| Wave | Slices | Gate |
| --- | --- | --- |
| 0 | Q0; in parallel, finish the minimum failure seam from `0db7a7a` | ADR 0081 owner sign-off |
| 1 | Q1 ∥ Q3 ∥ Q7 | targeted docs/unit/browser proof |
| 2 | Q2 ∥ Q6 ∥ Q8 | Q2 failure seam available; no file overlap assigned |
| 3 | Q4 ∥ Q5 | Q1+Q3 and Q1+Q2 respectively green |
| 4 | Q9, single owner | J1/J2 pilot + owner verdict |
| 5 | closure, orchestrator + owner | `make check-epic`, retro, sign-off |

Q3 owns `tests/browser/helpers/harness.ts` and budget migration. Q4 may add helpers
after Q3 but must not redesign the metrics API. Q9 is the only task allowed to combine
all new mechanisms in J1/J2. Q5, Q6, and Q7 all touch command surfaces/docs; assign one
integration owner for `Makefile`, `package.json`, and `docs/testing.md` merges.

## Shared task discipline

For every Q task:

- read its Radicle card and this section before editing;
- use `rtk` and `repoctx`; verify changed-boundary consumers before writing;
- update the named canonical docs in the same slice;
- write the smallest behavior test first and paste the failing command plus relevant
  failure excerpt into the task card before implementation;
- never “prove red” by breaking an existing gate or modifying a committed baseline;
- run targeted tests first, then the affected browser projects; run `make check` after
  the final change of the task;
- update this plan if an implementation-level name/path changes; stop for an ADR
  conflict or any tripwire;
- do not commit, push, solve a card, or close the epic without owner direction.

## Q0 — architecture and canonical process contract (`0bb4680`)

### Outcome

Create ADR 0081 and assign every rule one canonical home before tooling begins. ADR
0081 starts as `Status: Proposed`; owner review changes it to `Accepted`. Q1–Q9
enforcement does not begin while it remains Proposed.

### Files

- add `docs/adr/0081-ux-quality-loop-v2.md`;
- amend ADR 0074 and ADR 0076 with a short dated pointer to ADR 0081, without
  restating its decisions;
- add the new canonical sections/pointers to `docs/ui-authoring.md`,
  `docs/testing.md`, `docs/dogfooding.md`, and `docs/engineering-workflow.md`;
- make `docs/retros/TEMPLATE.md` tracked using the narrow `.gitignore` exception;
- regenerate `docs/adr/INDEX.md` through the generator only;
- keep this plan and the roadmap pointer in sync if the approved decision changes.

### ADR 0081 decisions

1. Define a non-mechanical UI change as a new panel/screen, functional redesign,
   changed cross-screen journey, or new primary user decision. Copy/token-only fixes,
   primitive-preserving mechanical migrations, and exact regression repairs are
   exempt unless they change a journey.
2. Require an approved experience contract for non-mechanical work: trigger, outcome,
   decision, evidence, hierarchy, primary action, entry/exit/recovery, done-well,
   storyboard, state matrix, journey mapping, and first red journey test.
3. Store the completed textual contract in the feature plan and the approved visual
   storyboard under `docs/mockups/`; templates are reusable sources, not approvals.
4. Separate deterministic automation, mandatory review artifacts, and human verdicts.
5. Keep contact sheets, live checkpoints, and trend reports outside `make check`.
6. Keep the current browser project matrix and gate budget; no global retries/timeouts.
7. Require first-vertical-slice, mid-milestone, and release checkpoints only for the
   pilot; promote selected checks after Q9 owner approval.
8. A P1 finding blocks expansion of the current slice. P2/P3 findings are fixed or
   tracked honestly; no silent deferral.
9. Record the exact boundary with solved epic `44beb6e` and open epic `0db7a7a`.

### Red and verification

This is a docs/decision slice. The meaningful stop gate is owner approval, not a
fabricated unit test. The new ADR intentionally makes the ADR index stale; verify that
`docs-drift` reports it, regenerate with:

```sh
rtk node scripts/check/docs-drift.mjs --write-adr-index
rtk node scripts/check/docs-drift.mjs
rtk make check
```

Acceptance: links resolve, the ADR ownership table has no duplicated canonical rule,
the ignored-template conflict is closed, full gate is green, and the owner explicitly
accepts ADR 0081 before the card advances.

### STOP-AND-ASK

- owner wants timing or screenshot aesthetics to hard-fail;
- a rule would duplicate mechanics across canonical docs;
- tracking the retro template would expose owner retros;
- implementation must begin before the v0.52 overlap is resolved.

## Q1 — experience-contract, storyboard, and state-matrix templates (`b899ec1`)

### Outcome

Make UX-first TDD copyable and concrete without inventing a broad script that guesses
whether a code change is “mechanical.”

### Files and artifacts

- add `docs/plans/EXPERIENCE-CONTRACT-TEMPLATE.md`;
- add `docs/mockups/STORYBOARD-TEMPLATE.html`;
- update `docs/plans/README.md`, `docs/mockups/README.md`, and
  `docs/ui-authoring.md` with the trigger, exemption, storage, and approval flow;
- add a worked current-state J1 example to this plan's pilot appendix; mark it
  explicitly **not redesign approval**.

### Exact template contract

The Markdown template contains these required sections:

1. card/plan section, owner, status, served journey, first red journey test;
2. user/context and trigger;
3. desired outcome and the decision being made;
4. evidence required before that decision;
5. information hierarchy: must-see, secondary, hidden-until-needed;
6. exactly one primary action, or an explicit reason why no single primary exists;
7. entry path, exit/next step, and recovery/undo path;
8. done-well criteria distinct from “record saved”;
9. assumptions and explicitly excluded redesign scope;
10. storyboard frame table;
11. state matrix.

Storyboard frames: entry, before action, loading/in-flight, success, error,
undo/recovery, and narrow pane. Each frame names the state, primary action, feedback,
and intended focus. The HTML template uses the existing mockup convention and no
runtime dependency.

State-matrix columns: `State | User sees | Primary action | Feedback/recovery |
Automated proof | Human review`. Required rows: empty, loading, partial, success,
error, stale, dense. Any `N/A` requires a written reason.

### Red and verification

No repo-wide linter tries to infer which features require a contract. Red evidence is
the worked J1 copy missing the required state rows before the template is applied; the
review catches it. Verify:

```sh
rtk node scripts/check/docs-drift.mjs
rtk make check
```

Acceptance: an implementer can copy the templates without re-deciding their shape;
the J1 example fills every field; the HTML opens in a browser; owner approval is
recorded before a copied storyboard becomes normative.

### STOP-AND-ASK

- adding a parser that decides subjective completeness;
- storing an approved artifact only in a session or `test-results/`;
- treating the worked J1 current state as permission to redesign Today.

## Q2 — hostile-content and controlled-async scenarios (`a9992e2`)

### Prerequisite boundary with `0db7a7a`

Q2 needs one small generic seam from the failure-path epic: the invocation settlement
layer must be able to reject a selected command/job with the normal mock command error.
It does **not** need to wait for that epic's independent real-Rust/real-DB evaluator.
If `0db7a7a` still has no child for this seam at kickoff, create/link one before coding
and replace Q2's broad blocker with that child.

### Files

- add `src/test/scenarios/overlays.ts` and `overlays.test.ts`;
- add `src/test/scenarios/controlledAsync.ts` and focused tests;
- extend `src/test/scenarios/scenarios.ts`, `runtime.ts`, and `runtime.test.ts`;
- extend the thin adapter in `src/test/browserSmokeRuntime.ts`;
- add `tests/browser/helpers/mockRuntime.ts` as the typed Playwright bridge;
- add `tests/browser/research-controlled-async.spec.ts`;
- update `docs/testing.md`.

Do not use the untracked v0.52 throwaway file `_mcp_shot.spec.ts` as a pattern or fold
it into this work.

### Data API

Keep base names unchanged and introduce serializable composition:

```ts
export type ScenarioOverlayName =
  | "hostile-content"
  | "dense-history"
  | "partial-data"
  | "stale-processing"
  | "conflicting-statuses"
  | "mixed-locale";

export type ScenarioSpec = {
  base: ScenarioName;
  overlays?: readonly ScenarioOverlayName[];
};

export function applyScenarioOverlays(
  data: ScenarioData,
  overlays: readonly ScenarioOverlayName[],
): ScenarioData;
```

Overlays are pure, fixed-ID/fixed-time, deterministic transformations. They reassign
collections rather than mutating entities in place. `buildScenario` accepts the old
`ScenarioName` for compatibility and the new `ScenarioSpec`; it returns a deep clone.
Overlay application is ordered and idempotent for a duplicate overlay name.

Required content:

- `hostile-content`: unbreakable URL/filename, long issuer/title/body, long metric
  label, Unicode/Polish diacritics;
- `dense-history`: hundreds of rows only when explicitly selected;
- `partial-data`: populated sibling domains with one relevant read missing;
- `stale-processing`: old visible result plus an in-flight job/status;
- `conflicting-statuses`: deliberately contradictory independent source/job states;
- `mixed-locale`: realistic Polish and English source strings, never untranslated UI
  literals planted as app copy.

### Controlled async API

Wire control once around `MockRuntime.invoke`; never inside the 150+ handlers. It
gates the real handler thunk:

```ts
type InvocationPhase = "before-handler" | "after-handler";

type InvocationMatch = {
  command: string;
  args?: Record<string, unknown>;
  phase?: InvocationPhase;
};

interface MockRuntimeControls {
  hold(match: InvocationMatch): string;
  pending(): readonly PendingInvocation[];
  release(id: string): void;
  reject(id: string, error: CommandError | Error): void;
  releaseAll(): void;
}
```

`before-handler` simulates delayed execution; `after-handler` captures the computed
response and simulates a stale network/IPC completion. Use `after-handler` only for
read handlers: a mutating handler has already changed the store by that phase and must
be held `before-handler` instead. Reset rejects and clears every held invocation so
promises never leak across tests. Generic rejection delegates to the `0db7a7a` failure
API.

Expose a typed test-only `window.__brawlerMock` bridge with `reset(spec)`,
`hold(match)`, `pending()`, `release(id)`, and `reject(id, error)`. The browser setup
order is **base → existing browser projection → overlays** on install and reset;
otherwise `seedBrowserStore()` silently overwrites hostile data.

### Tests that redden first

1. every overlay composes with empty/minimal/rich without throwing;
2. duplicate/order behavior and deterministic output;
3. two runtimes built from the same spec remain deeply isolated;
4. browser projection preserves the hostile invariant after reset;
5. two held invocations complete newest-before-oldest in a chosen order;
6. reset cleans pending work;
7. argument matching holds only the intended invocation;
8. failure uses the shared `0db7a7a` error path;
9. Playwright holds two `list_research_evidence` responses for different company
   intents, releases newest then oldest, and proves the older response cannot replace
   the latest state (`useResearchController` request-version seam).

### Verification

```sh
rtk npm test -- runtime
rtk npm test -- overlays
rtk npx playwright test tests/browser/research-controlled-async.spec.ts --project=chromium-compact
rtk make check
```

Acceptance includes no gate-wide dense scenario, no pending promise after a test, and
no second command/failure router.

### STOP-AND-ASK

- `0db7a7a` delivers an incompatible failure seam;
- an overlay requires application production code to understand test concepts;
- browser seed ordering cannot be unified without changing canonical scenario meaning;
- a mutating command is held after its handler and would change state before release;
- a test “fix” adds retries or sleep.

## Q3 — journey metrics beyond clicks (`ca99420`)

### Outcome and policy

Upgrade the current flat click counter to deterministic friction metrics. The canonical
happy journey owns budgets; adversarial probes report metrics but do not inflate the
happy-path floor. Wall-clock feedback stays advisory.

### Files

- add `src/test/journeyMetrics.ts` and `journeyMetrics.test.ts` for pure accounting;
- split browser orchestration into `tests/browser/helpers/journey.ts`, re-exported by
  `tests/browser/helpers/harness.ts`;
- migrate `tests/browser/journeys/budgets.json` to schema v2;
- add stable observation points in `src/app/AppShell.tsx` and
  `src/screens/Cockpit/CockpitScreen.tsx`;
- update all seven journey specs to `await j.assertBudget()`;
- establish initial J1/J2 expanded baselines; update `docs/testing.md` and
  `docs/ux-journeys.md` only where their canonical ceilings change.

### Budget shape

```json
{
  "schemaVersion": 2,
  "journeys": {
    "J1": {
      "interactions": 7,
      "screenTransitions": 3,
      "modalOpens": 0,
      "contextLosses": 0,
      "byProject": {}
    }
  }
}
```

`byProject` is present only where a real narrow-pane flow requires different user
actions; it must not hide a common regression. Add `data-app-section` to the main
workspace and `data-company-id` to the cockpit root. The active nav button gains
`aria-current="page"`; these are semantic observation points, not test-only business
state.

### Journey API

```ts
interface Journey {
  click(locator: Locator): Promise<void>;
  clickPrimary(surface: Locator, action: Locator): Promise<void>;
  fill(locator: Locator, value: string): Promise<void>;
  press(target: Locator | Page, key: string): Promise<void>;
  selectOption(locator: Locator, value: string): Promise<void>;
  markScreen(name: string): Promise<void>;
  markModal(name: string): Promise<void>;
  preserveContext(key: string | null): Promise<void>;
  expectFeedback(locator: Locator): Promise<void>;
  assertBudget(): Promise<void>;
}
```

The wrapper stores an event trace and samples state after the next explicit marker,
not immediately after a React click. Hard/ratcheted metrics after pilot approval:
interactions, distinct screen transitions, newly opened dialogs, and context loss when
the spec explicitly calls `preserveContext`. `clickPrimary` records whether the action
was outside its relevant initial scrollport before Playwright auto-scroll. Feedback
elapsed time and scroll observations are attached to the Playwright result for review;
they do not hard-fail on milliseconds.

Remove J2's forced 900×700 pane shortcuts from the journey. Density tests may force a
pane, but a user journey must take the real disclosure path at the current project
viewport. If counts genuinely differ, record a reviewed `byProject` budget.

### Tests that redden first

- missing schema/version/journey/metric budget;
- each hard metric exceeds its own limit with an error containing journey, metric,
  actual, limit, project, viewport, and trace;
- explicit context preservation catches a loss and ignores a deliberate reset;
- modal counting ignores a dialog already open at the prior marker;
- primary action records pre-scroll visibility;
- feedback before a Q2 controlled promise release is observed without a real-time
  threshold;
- J1/J2 establish baseline on every normal browser project.

### Verification

```sh
rtk npm test -- journeyMetrics
rtk npx playwright test tests/browser/journeys/j1-morning-review.spec.ts tests/browser/journeys/j2-company-published-a-report.spec.ts
rtk npx playwright test tests/browser/journeys
rtk make check
```

### STOP-AND-ASK

- a metric needs heuristics over arbitrary DOM instead of explicit markers;
- a wall-clock number is proposed as a hard gate;
- forced pane sizing is retained solely to keep one shared budget;
- budget is raised without an experience-contract reason.

## Q4 — discoverability and interaction-hierarchy contracts (`d4a68c2`)

### Outcome

Provide low-noise, explicitly scoped contracts for a decision surface. Do not globally
scan a multi-pane workspace or automate subjective visual taste.

### Files

- update `src/ui/Button.tsx`, `PrimitiveGallery.tsx`, `primitives.test.tsx`, and
  `primitives.a11y.test.tsx`;
- add `tests/browser/helpers/interactionContracts.ts` and a focused helper contract
  spec;
- adopt the helpers in J1/J2 pilot surfaces;
- document exemptions in `docs/ui-authoring.md` and the experience template.

### Primitive and helper contract

`Button` emits stable `data-ui-button-variant={variant}` metadata. For the explicit
experience primary action, callers add `data-ux-primary-action="true"`; this is not
inferred from a CSS class. Icon-only buttons retain the existing accessible-name
requirement; non-obvious icon actions also need a visible explanation or title per the
experience contract.

Helpers:

```ts
expectPrimaryActionCount(surface, { max: 1, reason?: string })
expectActionBeforeScroll(action, scrollOwner)
expectFocusOrder(page, locators)
expectNamedIconActions(surface)
expectNextStepVisible(locator)
```

Every helper receives an explicit surface/action locator. A multi-primary exemption
requires a non-empty reason and a matching experience-contract entry. Axe remains the
general accessible-name authority; these helpers cover the UX contract only.

### Tests that redden first

- primitive metadata disappears;
- a contracted surface renders two marked primary actions;
- primary action begins below the relevant scrollport;
- non-obvious icon action lacks its explanation/name;
- declared Tab sequence is reversed;
- success hides the contracted next step;
- real J1 and J2 decision surfaces fail before their primary metadata/contracts are
  added.

### Verification

```sh
rtk npm test -- primitives
rtk npx playwright test tests/browser/interaction-contracts.spec.ts --project=chromium-compact
rtk npx playwright test tests/browser/journeys/j1-morning-review.spec.ts tests/browser/journeys/j2-company-published-a-report.spec.ts
rtk make check
```

### STOP-AND-ASK

- proposed whole-page CTA count on a multi-pane workspace;
- selecting `.primary-button` CSS instead of semantic primitive metadata;
- a helper tries to decide which information “looks important”;
- exemption has no experience-contract rationale.

## Q5 — visual contact-sheet review (`81313f0`)

### Outcome

Generate one compact local HTML artifact from the existing visual scenarios. The
contact sheet makes human review cheap; committed Playwright baselines remain the
regression mechanism.

### Files

- add `tests/browser/visual/catalog.ts`;
- extend `tests/browser/visual/helpers.ts` and existing visual specs with stable
  screen/state metadata;
- add `scripts/ux/contact-sheet.mjs` and
  `scripts/ux/contact-sheet.test.mjs` using `node:test`;
- add `ux-contact-sheet` and guarded `visual-update` commands to `package.json` and
  `Makefile`;
- ignore `.artifacts/` and document the command in `docs/testing.md`.

### Mechanics

The catalog maps stable screen IDs to their owning visual spec and supported states.
Shared UI/style changes select all affected catalog entries; `--changed` maps a
read-only git diff through the catalog. An unknown file requires explicit `--screens`
instead of silently selecting nothing.

`shootPanel`/`shootScreen` gain `{ screen, state }`. With
`BRAWLER_CONTACT_SHEET_DIR` set, the helper captures the same settled locator as a
current PNG and writes unique per-worker JSON metadata **before** the existing
`toHaveScreenshot` assertion. The assertion still runs. If it fails, the orchestrator
builds the contact sheet from already-written evidence and then returns the original
non-zero Playwright exit. Metadata includes screen, state, S/M/L tier, theme, project,
and build stamp. Per-worker sidecars avoid `fullyParallel` write races.

`scripts/ux/contact-sheet.mjs`:

- accepts `--screens`, `--changed`, `--state`, and `--theme`;
- runs only `chromium-visual` and `chromium-visual-light` using existing specs;
- merges sidecars and emits a self-contained, base64-image `index.html` grid under
  `.artifacts/ux-contact-sheets/<build>/`;
- uses no Sharp/ImageMagick/native dependency;
- reports a missing expected cell as failure.

`visual-update` wraps Playwright `--update-snapshots` and refuses to run unless both
`SCREEN` and non-empty `REASON` are provided. It prints them into the run log; review
still requires the eventual change description to name the screens and reason.

### Tests that redden first

- duplicate/missing catalog screen IDs;
- visual case without catalog metadata;
- parallel sidecars merge without loss;
- HTML contains each selected screen/state/tier/theme cell and build stamp;
- `--changed` maps a representative screen and a shared style;
- baseline update without `SCREEN` or `REASON` fails;
- a real two-screen contact-sheet smoke produces an openable HTML file.

### Verification

```sh
rtk node --test scripts/ux/contact-sheet.test.mjs
rtk make ux-contact-sheet SCREENS="today,fundamentals"
rtk npx playwright test --project=chromium-visual --project=chromium-visual-light
rtk make check
```

Do not run `visual-update` during implementation unless a visual change is deliberate
and approved.

### STOP-AND-ASK

- adding a second screenshot framework or native image dependency;
- committing `.artifacts/` or owner-data images;
- masking deterministic content to make a diff pass;
- updating a baseline without a named screen and reason.

## Q6 — first-slice and mid-milestone live UX checkpoints (`a26cc6e`)

### Outcome

Keep release dogfooding and add two earlier, scoped exploratory checkpoints. Automation
collects mechanics/evidence; a human answers whether the journey is clear, useful, and
trustworthy.

### Files

- expand `docs/dogfooding.md` to three checkpoint levels;
- update the live-drive section of `docs/testing.md`;
- extend `Makefile` with a scoped live-spec argument/target;
- add `tests/live/ux-checkpoint.live.spec.ts` and, if useful,
  `tests/live/helpers/checkpointEvidence.ts`;
- keep output under `test-results/live/checkpoints/`.

### Command and privacy contract

Today `make live-drive` executes every historical live spec, which is too broad and may
mutate real state. Add:

```make
LIVE_SPEC ?=
```

and pass the optional path to the Playwright live invocation. Empty preserves current
full-suite behavior. Document:

```sh
rtk make live-cycle LIVE_SPEC=tests/live/ux-checkpoint.live.spec.ts
```

The generic checkpoint reads `BRAWLER_UX_JOURNEY`, `BRAWLER_UX_CARD`, and
`BRAWLER_UX_STAGE=vertical|mid|release`. It records date, app version/revision,
Windows-native confirmation, user agent/WebView2, viewport/DPR, locale/theme,
non-sensitive dataset label, screenshot directory, and mechanical observations. Never
record the database path or contents.

Three cadences:

- first vertical slice: 3–5 minutes, before expanding the slice;
- mid-milestone: about 10 minutes, after integration seams exist;
- release: the existing about-15-minute journey walk.

The human charter names one exploratory question, P1/P2/P3 findings, verdict
`proceed | revise | block`, and which judgments remained human. Non-sensitive verdict
metadata goes to the active Radicle card; screenshots stay local. P1 blocks expansion.

The generic J1 mechanical path may prove Today renders an attention stream or explicit
quiet state, no generic error state is visible, a visible Review action opens a
company-scoped cockpit, return works, and a screenshot is captured. It must never print
“UX good.” Feature-specific mutating steps belong to an explicit pilot spec and require
owner intent.

### Red and verification

- scoped `LIVE_SPEC` initially fails because Make runs the full suite;
- helper refuses missing card/journey/stage metadata;
- evidence paths are inside ignored `test-results/`;
- checkpoint spec distinguishes a quiet state from a blank/error state;
- full empty `LIVE_SPEC` behavior remains unchanged.

```sh
rtk make live-drive LIVE_SPEC=tests/live/ux-checkpoint.live.spec.ts
rtk make live-cycle LIVE_SPEC=tests/live/ux-checkpoint.live.spec.ts
rtk make check
```

The live commands require an intentionally running/rebuilt Windows app and are not part
of `make check`; record when they were not run in ordinary implementation sessions.

### STOP-AND-ASK

- checkpoint would write/delete owner data by default;
- screenshot/manifest could enter the public repo;
- automation is asked to judge clarity/usefulness;
- Q6 is proposed as universal DoD before Q9.

## Q7 — escaped-defect taxonomy and trend report (`3b8f1df`)

### Outcome

Classify where escaped frontend/UX defects originated and when they were detected.
Counts inform guardrail harvest; they are not a performance target.

### Files

- extend tracked `docs/retros/TEMPLATE.md`;
- add `scripts/ux/escaped-defects-report.mjs` plus Node built-in tests;
- add advisory `report:escaped-defects` to `package.json`/Make help;
- update `docs/testing.md` with interpretation rules;
- calibrate by reading the local v0.47 and v0.50 retros; do not expose private data.

### Taxonomy and record shape

Canonical origin slugs:

`spec-gap`, `ux-decision`, `missing-state`, `mock-realism`,
`integration-seam`, `responsive-layout`, `visual-hierarchy`, `async-race`,
`native-runtime`, `real-data-shape`, `test-flake`.

Detection stages:

`implementation`, `targeted-test`, `full-gate`, `vertical-slice`,
`mid-milestone`, `release-dogfood`, `post-release/user`, and
`unknown/historical-evidence-insufficient`.

Retro table:

```text
Ref | Origin class | Detected at | Earliest prevention point | Disposition | Status
```

Disposition is one of `automated-guardrail`, `human-checklist`,
`fixed-instance-only`, `tracked:<hex7>`, or `accepted-limitation`. Record one row per
actual defect; “missing test” is the earlier-prevention field, not a second defect.

The report parses only explicitly marked escaped-defect tables. Historical retros
without one remain valid. It validates known enums, required cells, and unique refs;
prints counts by origin/stage plus repeated classes (`count >= 2`); never fails because
counts increased. During Q7 it is advisory. Q9 decides whether malformed opted-in rows
join a deterministic docs check.

Calibration examples:

- v0.47 inner-scroll overflow → `responsive-layout`;
- v0.50 contradictory/lost design specs → `spec-gap`;
- v0.50 cross-wave panel misses → `integration-seam`;
- real owner `.xbri`/data findings → `real-data-shape` or `mock-realism` according to
  evidence;
- full-gate workflow timeout/race → `test-flake` or `async-race`, not both.

Do not invent a historical detection stage; use the explicit unknown value.

### Red and verification

- unknown class/stage/disposition fails validation;
- duplicate ref fails;
- old retro with no marked table is ignored;
- counts/repeated-class output matches a small sample;
- increasing a count remains exit 0;
- malformed opted-in row exits non-zero.

```sh
rtk node --test scripts/ux/escaped-defects-report.test.mjs
rtk npm run report:escaped-defects
rtk make check
```

### STOP-AND-ASK

- turning fewer findings into an incentive/target;
- adding telemetry or reading the live database;
- automatically opening a test card for every escape;
- editing ignored owner retros without explicit owner direction.

## Q8 — frontend test responsibility and integration seams (`66621c9`)

### Outcome

Move proof to the cheapest authoritative layer and reduce flaky full-app Vitest work
without deleting behavior coverage or lowering the coverage ratchet.

### Audit artifact

Add `docs/plans/ux-quality-loop-v2-test-inventory.md` with one row per audited behavior:

```text
Test/behavior | Current layer | Authoritative layer | Keep/move/split |
Replacement proof | Targeted runtime before/after
```

Classify component state, workflow, layout, journey, visual, and live-runtime proof.
Start from the approximately 20 `appWorkflowHarness` importers and the existing browser
families; do not rename or rewrite tests merely to fill the ledger.

### Initial candidates and retained layers

- `src/App.test.tsx`: browser owns broad navigation/shell walks; retain isolated
  shortcut/palette/state behavior.
- `src/app/GlobalSearch.test.tsx`: add authoritative
  `tests/browser/global-search.spec.ts` for cross-screen navigation; retain isolated
  grouping/highlight/input state.
- first cross-screen cases in `ResearchScreen.test.tsx`: move Research→Inbox seams to
  browser; keep timeline/forms/error/filter component behavior.
- `CockpitScreen.test.tsx`: browser owns command-palette/company/open-panel/Dockview
  integration; keep pure parsing and controlled internal state.
- keep `screens.a11y.test.tsx`: it is the unique fast structural axe layer.
- keep Settings/Notebooks/Today component state, forms, error, and controlled-async
  tests. Optimize their harness only with measured evidence; do not blindly migrate.

For each move, land the red browser replacement first, make it green, then remove only
the duplicated assertion. Playwright coverage does not count toward Vitest V8 line
coverage, so retain/extract equivalent component coverage before deletion. Never lower
`coverage-baseline.json`.

Update `docs/testing.md` with the responsibility table and update the Q1 planning
template so a multi-slice task names component proof, cross-screen browser proof,
consumer map, and full-wave verification.

### Red and verification

- replacement browser spec fails before the source test is changed;
- retained component test proves the local state/error branch;
- coverage floor holds;
- run the affected Vitest target repeatedly to compare flakes/runtime, but do not add
  retries/global timeout;
- record warm before/after targeted-loop median as advisory evidence.

```sh
rtk npm test -- GlobalSearch ResearchScreen CockpitScreen
rtk npx playwright test tests/browser/global-search.spec.ts tests/browser/cockpit-view-company.spec.ts tests/browser/journeys
rtk make coverage
rtk make check
```

### STOP-AND-ASK

- deleting behavior because a file is large;
- lowering the coverage baseline after moving a browser-owned assertion;
- retry/timeout increase described as stability work;
- broad harness rewrite without measured targeted-loop benefit.

## Q9 — J1/J2 pilot and adoption verdict (`31a0fd5`)

### Preconditions

All Q1–Q8 blockers are green, ADR 0081 is Accepted, the v0.52 J2 expectation step is
present, and the active worktree is reviewable. Q3/Q4 “foundation” assertions do not
count as the pilot; Q9 is the complete scenario matrix, human review, and rollout
verdict.

### Approved artifacts and files

- complete the J1/J2 experience contracts in this plan's appendix;
- add owner-approved `docs/mockups/j1-morning-review-storyboard.html` and
  `docs/mockups/j2-report-published-storyboard.html`;
- extend the existing J1/J2 journey specs in place;
- use Q2 overlays/controls, Q3 budgets, Q4 interaction helpers, Q5 contact sheets,
  Q6 checkpoint evidence, and Q7 classification;
- add `tests/live/ux-quality-pilot.live.spec.ts` for read-safe mechanical evidence;
- write the local epic retro `docs/retros/ux-quality-loop-v2.md`;
- amend ADR 0081 with dated pilot evidence and the final owner-approved adoption list.

### Pilot matrix

| Journey | Happy | Hostile | Partial/failure | Controlled async |
| --- | --- | --- | --- | --- |
| J1 morning review | canonical rich journey and expanded budgets | long/mixed-language titles, tickers, URLs; Review remains reachable | failed attention source is explicit; never false quiet | out-of-order attention/category reads cannot restore stale content |
| J2 report published | v0.52 expectation step + KPI/claim/note flow | long filenames, metric labels, citations, dense proposals | explicit extraction failure/partial result + retry; never success with zero effect | double action, modal close/reopen, and reversed completion cannot duplicate/overwrite newer state |

Happy paths alone assert ratcheted budgets. Adversarial probes assert honest states,
layout, recovery, and context but do not change the canonical interaction floor.

### Execution sequence

1. Complete/freeze both textual contracts and owner-approve both storyboards.
2. Add happy + hostile vertical slices; capture contact sheets.
3. Run first real-Windows checkpoint for J1 and J2 before expanding.
4. Add partial failure and controlled-async cases.
5. Capture expanded metrics/discoverability baselines and the complete contact sheet.
6. Owner records an explicit contact-sheet verdict.
7. Run mid-pilot Windows/real-DB checkpoints for both journeys.
8. Classify findings; fix only approved-scope defects and create separate cards for
   redesigns or unrelated bugs.
9. Run targeted matrix, full visual projects, `make check`, and `make check-epic`.
10. Retro decides `adopt | revise | reject` for every practice: experience contract,
    state matrix/storyboard, overlays, async controls, expanded budgets,
    discoverability contracts, contact sheets, early checkpoints, escape report, and
    test-layer audit.
11. Only adopted + owner-approved practices enter universal DoD or deterministic gate.

The live pilot defaults to read-safe navigation/evidence. Any J2 action that mutates
the owner database requires an explicit environment opt-in and an owner-selected
record; the charter records how it is restored or why it is intentionally durable.

### Verification

```sh
rtk npm test -- journeyMetrics runtime overlays primitives
rtk npx playwright test tests/browser/journeys/j1-morning-review.spec.ts tests/browser/journeys/j2-company-published-a-report.spec.ts
rtk npx playwright test --project=chromium-visual --project=chromium-visual-light
rtk make ux-contact-sheet SCREENS="today,inbox,fundamentals,claims,notebook-company"
rtk make live-cycle LIVE_SPEC=tests/live/ux-quality-pilot.live.spec.ts
rtk make check
rtk make check-epic
```

Report the exact Windows/real-DB steps that ran and any opt-in mutation. A green browser
pilot without owner clarity/usefulness verdict is incomplete.

### STOP-AND-ASK

- P1 finding appears;
- pilot needs a Today/J2 redesign outside approved contracts;
- visual baseline change has no explanation;
- a human judgment is being converted into a synthetic score;
- owner-data screenshot/output could be committed;
- universal enforcement is proposed before the per-practice verdict.

## Pilot appendix — current-state experience contracts

These are Q1 worked examples and Q9 starting points. They describe current intent, not
approval to redesign the screens. Q9 copies/completes them only after v0.52 lands.

### J1 — morning review

| Field | Current-state contract |
| --- | --- |
| Trigger | User opens Brawler at the start of a review session. |
| Outcome | User knows what changed and which 0–2 items deserve attention. |
| Decision | Open an item now, leave it for later, or conclude the state is quiet. |
| Required evidence | source/company/type/date, why the item is surfaced, one clear review action, explicit partial/error state |
| Must-see | prioritized stream, identity, timestamp, reason/action |
| Secondary | counters and supporting summary |
| Hidden until needed | run detail and deep company panels |
| Primary action | Review the selected attention item; quiet state has no artificial CTA. |
| Entry/exit | app start → Today; Review → company cockpit; return → preserved Today context |
| Recovery | failed category is explicit and retryable; it cannot masquerade as quiet. |
| Done well | high-signal state is handled in under ten minutes and no important item is silently absent. |
| First red test | `tests/browser/journeys/j1-morning-review.spec.ts` hostile/partial case |

State rows required at Q9: quiet/empty, progressive loading, partial category failure,
success stream, full error, stale processing, dense hostile stream.

#### J1 worked experience contract (Q1 example)

**NOT redesign approval — current-state description only.** This applies
[`EXPERIENCE-CONTRACT-TEMPLATE.md`](EXPERIENCE-CONTRACT-TEMPLATE.md)'s 11 sections to
the condensed table above, to prove an implementer can fill the template from a real
screen without re-deriving its shape. Q9 owns the final owner-approved version (a
real, owner-reviewed `docs/mockups/j1-morning-review-storyboard.html` and live pilot
evidence); until then, no field here authorizes a Today-screen redesign.

**1. Card, plan section, owner, status, served journey, first red journey test**

| Field | Value |
| --- | --- |
| Radicle card | `b899ec1` (Q1 template task); the full pilot is `31a0fd5` (Q9) |
| Plan section | `docs/plans/ux-quality-loop-v2.md` § Pilot appendix — current-state experience contracts |
| Owner | wbednarczyk |
| Status | worked example — current-state description, not owner-approved for redesign |
| Served journey | J1 — Morning review (`docs/ux-journeys.md`) |
| First red journey test | `tests/browser/journeys/j1-morning-review.spec.ts` hostile/partial case |

**2. User / context and trigger**

User opens Brawler at the start of a review session.

**3. Desired outcome and the decision being made**

Outcome: the user knows what changed and which 0–2 items deserve attention. Decision:
open an item now, leave it for later, or conclude the state is quiet.

**4. Evidence required before that decision**

source/company/type/date, why the item is surfaced, one clear review action, explicit
partial/error state.

**5. Information hierarchy**

| Tier | Content |
| --- | --- |
| Must-see | prioritized stream, identity, timestamp, reason/action |
| Secondary | counters and supporting summary |
| Hidden until needed | run detail and deep company panels |

**6. Primary action**

Review the selected attention item; a quiet state has no artificial CTA.

**7. Entry path, exit/next step, and recovery/undo path**

| Path | Description |
| --- | --- |
| Entry | app start → Today |
| Exit / next step | Review → company cockpit; return → preserved Today context |
| Recovery / undo | failed category is explicit and retryable; it cannot masquerade as quiet |

**8. Done-well criteria**

High-signal state is handled in under ten minutes and no important item is silently
absent — distinct from merely "an item was reviewed".

**9. Assumptions and explicitly excluded redesign scope**

| | |
| --- | --- |
| Assumptions | today's attention-stream data shape and navigation stay as-is; this describes the current screen, not a future one |
| Excluded scope | this contract does not authorize any Today-screen redesign; a visual change needs its own owner-approved contract |

**10. Storyboard frame table**

Illustrative only — no visual storyboard file exists yet for J1; Q9 adds an
owner-approved `docs/mockups/j1-morning-review-storyboard.html` and this row then
links to it.

| Frame | State named | Primary action | Feedback | Intended focus |
| --- | --- | --- | --- | --- |
| Entry | quiet or populated attention stream on app start | Review (if any item present) | stream renders prioritized items, or the quiet message | first attention item, or the quiet-state message |
| Before action | item selected in the stream, not yet opened | Review the selected item | selected row is highlighted | selected row |
| Loading / in-flight | attention categories still resolving | none (wait) | per-category progressive loading indicator | the still-loading category |
| Success | item reviewed, stream reflects it | next item, or return to Today | reviewed item is marked/removed from the pending stream | next unresolved item, or the quiet message if none remain |
| Error | one or more attention categories failed to load | Retry the failed category | explicit failed-category message, distinct from quiet | the failed-category message and its retry action |
| Undo / recovery | user retries a failed category | Retry | category re-attempts and either succeeds or fails explicitly again | the retried category |
| Narrow pane | same stream at ~960–1280px tall-narrow width | Review (if any item present) | stream stacks single-column, scrolls internally, no global horizontal scrollbar | first attention item |

Storyboard: `docs/mockups/j1-morning-review-storyboard.html` (Q9, owner-approved)

**11. State matrix**

| State | User sees | Primary action | Feedback/recovery | Automated proof | Human review |
| --- | --- | --- | --- | --- | --- |
| Empty | quiet-state message, no artificial CTA | none | N/A — nothing to recover from | `tests/browser/journeys/j1-morning-review.spec.ts` quiet case | Q9 contact sheet |
| Loading | per-category progressive loading indicator | none (wait) | indicator resolves per category as data arrives | `tests/browser/journeys/j1-morning-review.spec.ts` progressive-loading case | Q9 contact sheet |
| Partial | some categories loaded, one still pending or failed | Review available items; retry the pending/failed category | pending/failed category is explicit, never merged into "quiet" | Q2 `partial-data` overlay case | Q9 contact sheet |
| Success | prioritized attention stream with identity/timestamp/reason | Review the selected item | reviewed item is marked/removed | `tests/browser/journeys/j1-morning-review.spec.ts` happy case | Q9 contact sheet |
| Error | explicit failed-category message | Retry the failed category | retry re-attempts; failure cannot masquerade as quiet | `tests/browser/journeys/j1-morning-review.spec.ts` hostile/partial case | Q9 contact sheet |
| Stale | previously loaded stream with an in-flight refresh | Review existing items; wait for refresh | stale content is never silently replaced by an out-of-order response | Q2 controlled-async held/released `list_*` case | Q9 contact sheet |
| Dense | hundreds of attention items, long/mixed-language titles and URLs | Review the selected item | list scrolls internally, no overflow, no truncated-to-empty rows | Q2 `dense-history` + `hostile-content` overlay case | Q9 contact sheet |

### J2 — company published a report

| Field | Current-state contract |
| --- | --- |
| Trigger | A periodic report/autopilot item arrives. |
| Outcome | User reviews the extracted change, resolves due judgment, and leaves a durable trace. |
| Decision | confirm/reject facts, resolve expectation/claim, and record what changed. |
| Required evidence | report identity/provenance, extraction status, KPI values/validation, expectations, due claims, note/journal context |
| Must-see | current report, extraction/review state, one next action, explicit failures |
| Secondary | diff/provenance detail and prior-period context |
| Hidden until needed | advanced extraction diagnostics and unrelated cockpit panels |
| Primary action | changes with the stage: extract/retry, confirm review item, resolve claim, save judgment; never two peers in one decision surface |
| Entry/exit | Today/Inbox report → review surface → company cockpit → recorded trace |
| Recovery | failed/partial extraction shows cause and retry; closing/reopening cannot duplicate or apply stale completion. |
| Done well | facts are accepted/rejected, due items resolved, and the user's expectation/judgment is recorded with provenance. |
| First red test | `tests/browser/journeys/j2-company-published-a-report.spec.ts` partial/async case |

State rows required at Q9: no extractable attachment, in-flight, partial proposals,
success, provider/runtime error, stale prior run, dense hostile proposals.

## Global tripwires

Stop and ask if any task proposes:

- a second mock/runtime/router or failure system;
- weakening, skipping, retrying, or baselining away an existing gate;
- real elapsed time, screenshot similarity, or an automated “good UX” score as truth;
- a new global Playwright project/scenario cross-product;
- product behavior not present in an approved experience contract;
- owner-data screenshots, DB paths, or private content in the public repo/Radicle;
- a broad guard that flags legitimate multi-pane/dense UI;
- a coverage floor reduction after moving tests;
- universal DoD enforcement before Q9 verdict;
- work on overlapping v0.52 files before that worktree is closed.

## Epic closure

Closure is owner + strongest-model work:

1. Audit ADR 0081 decisions one by one and verify live-path/tooling invocations with
   `repoctx callers`/file evidence.
2. Verify every child card has red evidence, targeted proof, full-gate proof, and named
   canonical-doc updates.
3. Run `make check-epic`; record the gate's own exit code.
4. Run the J1/J2 real-Windows pilot and record what was and was not mutated.
5. Review contact sheets and the escape report; human-only judgments remain named.
6. Write the epic retro with the per-practice adopt/revise/reject table.
7. Apply only owner-approved universal DoD/gate promotions and update ADR 0081.
8. Do not version-bump or release solely for this process epic.
9. Mark cards and epic solved only after explicit owner sign-off.
