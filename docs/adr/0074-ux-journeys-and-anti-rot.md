# ADR 0074: UX Journeys as Spec — Journey Tests, Step Budgets, and Anti-Rot Guardrails

Status: Accepted

The app has grown to 14 sections whose *micro*-consistency is machine-enforced (primitive lint, ADR 0037) while *macro*-level flows — what a user actually comes to do — have no spec, no test, and no gate. Screens can each be fine while the task that spans them quietly gets longer. This is a process ADR in the ADR 0038/0045 family: it makes user journeys a canonical spec with enforcement.

## Context

- 2026-07-03 audit: E2E coverage is per-screen and concentrated on older surfaces; flagship v0.44–0.49 features (autopilot, quality frameworks, report season, claims) have no clickable regression net at all.
- The CLAUDE.md rule "a capability is not done until a user can reach it" has no structural answer to *where* the user reaches it from.

## Decision

1. **`docs/ux-journeys.md` is the canonical journey spec** (jobs-to-be-done level, above `ui-flows.md`'s per-feature layer). Each journey: trigger → steps → screens → interaction budget → done-well criteria. Initial catalog: morning review; company published a report; new-company onboarding; report-season preparation; claim verification; buy/pass decision; weekly review.
2. **Journey E2E tests** in `tests/browser/journeys/`, one spec per journey, exercising the cross-screen path (not per-screen features). The v0.44–0.49 E2E backfill is delivered in this form — the audit gap and the journey net are the same work.
3. **Interaction step budgets as assertions**: each journey spec asserts its interaction count against the documented budget. Budgets are calibrated by first measurement, then ratcheted (coverage-ratchet precedent) — a UX regression reddens the gate like a byte-budget regression.
4. **Definition of Done line** (engineering-workflow.md §I): every user-facing capability names the journey it serves (or is explicitly declared a utility outside journeys). Closes the reachability rule structurally.
5. **Milestone retro gains a UX section**: which journeys got shorter/longer this milestone, measured, with still-open items feeding the guardrail harvest.

## Consequences

- UX quality gets the same treatment as code quality: spec + reddening test + ratchet, instead of taste and memory.
- New-feature planning starts from a journey (which one does this serve?) rather than a screen.
- `ui-flows.md` remains the per-feature behavior spec; journeys reference it, no duplication.
- Budgets are honest numbers, not aspirations: first measurement sets them; improvements tighten them deliberately.

## Related (2026-07-12)

[ADR 0081](0081-ux-quality-loop-v2.md) builds a UX decision-validation loop on top of this journey substrate (experience contracts, adversarial scenarios, richer journey metrics, contact-sheet review, early live checkpoints). It reuses these journeys and step budgets; it does not change this ADR's decisions.
