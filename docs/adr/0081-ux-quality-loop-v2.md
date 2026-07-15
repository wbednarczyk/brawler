# ADR 0081: UX Quality Loop v2 — Experience Contracts, Adversarial Validation, Continuous Dogfooding

Status: Accepted (owner sign-off 2026-07-12)

Brawler already tests code behavior, contracts, cross-screen journeys (ADR 0074),
accessibility, density and overflow (ADR 0076), and pixel stability extensively. The
UX defects that still escape cluster where **none of those tests decides quality**:
incomplete UX specifications, discoverability and hierarchy, transitions between async
states, hostile real-content shapes, integration seams, and owner validation that only
happens near release. This is a process ADR in the ADR 0038/0045/0074 family: it adds a
**UX decision-validation loop**, not another generic test layer. Enforcement is
pilot-gated — it does not become universal until the J1/J2 pilot returns a per-practice
verdict (Decision 7, [Rollout](#rollout-pilot-gated)).

Task-level execution detail lives in
[docs/plans/ux-quality-loop-v2.md](../plans/ux-quality-loop-v2.md) (non-normative; this
ADR and canonical docs win on conflict).

## Context

- The escaped-defect classes are not caught by "more of the same" testing: a
  happy-path-only mock hides dishonest states; per-screen assertions miss discoverability
  and hierarchy; deterministic snapshots do not grade clarity; real content (long titles,
  Polish diacritics, dense history) never enters the fixtures; async ordering is untested;
  owner review happens only at the release walk.
- Existing gates must stay deterministic and in the seconds-to-low-minutes class. The fix
  cannot add wall-clock UX thresholds or a second screenshot framework.
- The solved epic `44beb6e` (ADR 0074) established the journey substrate — journey specs,
  step budgets, DoD §I reachability. This epic builds the decision-validation loop **on
  top of** that substrate; it does not redo it.
- The open epic `0db7a7a` owns generic command/job failure injection and poor-state /
  real-DB foundations. This epic **consumes** its settlement seam; it does not reproduce
  failure infrastructure.

## Decision

### Canonical ownership

Every rule introduced by this epic has exactly one canonical home. Update it there; do
not duplicate mechanics across docs.

| Concern | Canonical owner |
| --- | --- |
| durable rationale; hard / advisory / human boundary; pilot rollout | **this ADR (0081)** |
| experience contract, storyboard / state matrix, discoverability authoring | [docs/ui-authoring.md](../ui-authoring.md) |
| mock scenarios, journey metrics, layer ownership, contact-sheet mechanics | [docs/testing.md](../testing.md) |
| first-slice, mid-milestone, and release exploratory checkpoints | [docs/dogfooding.md](../dogfooding.md) |
| short universal handover checks (after Q9 adoption only) | [docs/engineering-workflow.md](../engineering-workflow.md) |
| escape taxonomy and per-retro record shape | tracked [docs/retros/TEMPLATE.md](../retros/TEMPLATE.md) |
| task-level execution detail | [docs/plans/ux-quality-loop-v2.md](../plans/ux-quality-loop-v2.md) |
| live status and blockers | Radicle / Radboard |

### Rules

1. **Non-mechanical UI change** is a new panel/screen, a functional redesign, a changed
   cross-screen journey, or a new primary user decision. Copy/token-only fixes,
   primitive-preserving mechanical migrations, and exact regression repairs are **exempt**
   unless they change a journey.
2. Non-mechanical work requires an **approved experience contract**: trigger, outcome,
   decision, evidence, information hierarchy, single primary action, entry/exit/recovery,
   done-well criteria, storyboard, state matrix, journey mapping, and a first red journey
   test — authored *before* component work.
3. The completed **textual contract lives in the feature plan**; the approved **visual
   storyboard lives under `docs/mockups/`**. Templates are reusable sources, never
   approvals.
4. **Three proof classes are separate.** Deterministic behavior / layout / affordance
   contracts may hard-fail. Contact sheets and timing reports are **mandatory review
   evidence but do not grade visual taste**. Clarity / usefulness / trust remain
   **explicit human verdicts** — never a synthetic score.
5. Contact sheets, live checkpoints, and trend reports stay **outside `make check`**.
6. Keep the current browser project matrix and gate budget: **no global retries or
   timeouts**, no new Playwright project cross-product.
7. First-vertical-slice, mid-milestone, and release checkpoints are **pilot-only**;
   selected checks become universal only after Q9 owner approval.
8. A **P1 finding blocks expansion** of the current slice. P2/P3 findings are fixed or
   tracked honestly (`tracked:<hex7>`) — no silent deferral.
9. **Epic boundaries** (see [Context](#context)): the substrate from solved `44beb6e` is
   reused, not rebuilt; the failure-injection seam from open `0db7a7a` is consumed, not
   reproduced.

### Locked boundaries (constraints on implementation)

These are pre-decided; implementation sessions do not reopen them unless repository
reality contradicts the premise (then: stop and raise a doc/ADR change).

- **One runtime.** `src/test/scenarios/runtime.ts` stays the only command router for
  Vitest and Playwright — no second router, chaos or otherwise.
- **Base scenarios plus overlays.** `empty | minimal | rich` stay the base set;
  hostile/dense/partial/stale/conflicting/mixed-locale data are composable overlays.
- **No wall-clock UX hard gate.** Feedback latency is recorded and controlled-async proves
  an immediate pending state, but machine-sensitive milliseconds never block `make check`.
- **No universal rule before the pilot** (Decision 7).
- **No silent product redesign.** The pilot may fix an in-scope defect needed to complete
  J1/J2; a new information architecture or functional redesign is a separately approved
  product card/mockup.
- **Private evidence stays private.** Real-database screenshots/manifests live under
  gitignored `test-results/`; public docs and Radicle receive only non-sensitive metadata
  and verdicts.

## Rollout (pilot-gated)

Q1–Q8 build the pilot infrastructure (templates, overlays + async controls, journey
metrics, discoverability contracts, contact sheet, early checkpoints, escape report,
test-layer audit). **Q9 proves the whole loop on journeys J1 (morning review) and J2
(company published a report)** across happy, hostile, partial/failure, and
controlled-async cases, then returns an explicit **`adopt | revise | reject` verdict per
practice** with owner sign-off. Only adopted-and-approved practices then enter the
universal Definition of Done or a deterministic gate. Until this ADR is `Accepted`, no
Q1–Q9 enforcement begins.

### Pilot outcome and adoption (Q9, owner sign-off 2026-07-13)

The Q9 pilot ran the full loop on J1 (morning review) and J2 (company published a
report) across happy, hostile, partial/failure, and controlled-async cases.

**Evidence.** The deterministic J1/J2 scenario matrix is green across the viewport
matrix (`tests/browser/journeys/j1-morning-review.spec.ts`, `j2-company-published-a-report.spec.ts`); the read-safe live pilot ran on the real Windows app (`tests/live/ux-quality-pilot.live.spec.ts`) — J1 verified end to end (28 real attention rows, Review→cockpit→return), J2 surfaced one discoverability finding for owner classification. The pilot caught **three real escaped defects** the old process would have missed: a hostile-URL modal overflow and a Today false-quiet-on-error (both fixed + regression-guarded in approved scope), and a shared-primitive light-theme contrast miss (tracked, Radicle `9416da8`). Full retro + escaped-defect table: `docs/retros/ux-quality-loop-v2.md` (local).

**Verdict: `adopt` — all ten practices** (owner sign-off 2026-07-13): experience
contract, state matrix + storyboard (storyboards remain owner-approval-gated),
adversarial overlays, controlled-async controls, expanded journey budgets,
discoverability contracts, visual contact sheets (opt-in review, not a gate), early
live checkpoints, escaped-defect report (advisory), and the test-layer responsibility
audit. No practice was rejected or revised; none is converted into a machine
taste-threshold (the locked boundaries hold). Enforcement lives in each practice's
canonical home (the ownership map above) and the existing deterministic gate — the
journey/a11y/overlay specs, the DoD's `ui-authoring.md`/`dogfooding.md` links, and the
opt-in tools — **without** a new always-loaded Definition-of-Done line, so the
mandatory-read context stays within its ADR 0063 budget.

## Consequences

- UX decisions get specified and validated before code, the way code behavior already is —
  the escape classes above get a home where they can be caught early (contract,
  adversarial scenario, contact sheet, or first-slice live check) instead of at the final
  gate or release walk.
- The gate stays deterministic and fast: taste and human judgment are never converted into
  a machine threshold or committed as a hard check.
- New enforcement is earned, not assumed: a practice becomes universal only after the pilot
  shows it pays off. Rejected practices leave no residue in the gate.
- `docs/retros/TEMPLATE.md` becomes tracked (narrow `.gitignore` exception) so the escape
  taxonomy (Q7) has a public, canonical shape while individual retros stay local.
