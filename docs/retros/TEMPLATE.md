# Retrospective — <milestone / version + short title>

Status: **<shipped | shipped, scope-narrowed | descoped | …>.** One or two sentences:
what was built, the key decision, and where the evidence lives (ADR link). This retro
is the process record — *for the maintainer to review and decide what needs further
action*. Mark every gap **closed** or **still-open** honestly, never as a victory lap.

## What happened

What was planned vs. what actually shipped, in a short paragraph or two: the concrete
surfaces (commands, jobs, schema/migrations, UI panels, tests, docs) and any real-data
validation that gated the approach. State the pre-handover gate result.

## App domain

**Worth keeping**
- <patterns/decisions that paid off — what to repeat next time>

**Scope finding (closed)** *(if any)*
- <a scope narrowing/change made with evidence; recorded in which ADR, not silently>

**Still-open (app)**
- <honest limitations, deferred behavior, tracked bugs (Radicle hex7) — not folded into "done">

## Development-loop domain

**Went well / worth keeping**
- <process that worked: doc-first, guardrail-harvest-during, real-data-first, …>

**What went wrong — the most important part**
- <the honest failures: guessing instead of measuring, missing test, gate mis-read.
  Mark each **closed** (instance + class) or **still-open**>

**Guardrails harvested (ADR 0045)**
1. <where> — <the durable rule/gate landed this milestone (canonical doc, not memory)>

**What to stop doing**
- <habits to drop>

**What to improve / start**
- <concrete next-time changes; feed still-open items into the guardrail-harvest loop>

## Escaped defects (ADR 0081 Q7)

One row per actual defect that escaped an earlier, cheaper catch point — not per fix,
not per missing test ("missing test" is the *earliest prevention point* cell, not a
second row). Mark the table with the HTML comments below so
`rtk npm run report:escaped-defects` (`scripts/ux/escaped-defects-report.mjs`) can
parse it; omit the markers (and this section) on a retro predating the taxonomy —
historical retros with no marked table stay valid and are silently skipped, never
flagged. The report is advisory: it prints counts and repeated classes, never fails
because counts increased, and is never a target to minimize.

Canonical origin-class slugs: `spec-gap`, `ux-decision`, `missing-state`,
`mock-realism`, `integration-seam`, `responsive-layout`, `visual-hierarchy`,
`async-race`, `native-runtime`, `real-data-shape`, `test-flake`.

Detection stages: `implementation`, `targeted-test`, `full-gate`, `vertical-slice`,
`mid-milestone`, `release-dogfood`, `post-release/user`, or (only when the historical
evidence genuinely does not say) `unknown/historical-evidence-insufficient`.

Disposition — one of: `automated-guardrail` (a precise gate now catches the class),
`human-checklist` (documented rule, not automatable), `fixed-instance-only` (the one
occurrence was fixed, the class is not yet worth a guardrail), `tracked:<hex7>` (open
Radicle issue), or `accepted-limitation` (deliberately not fixed). **Not every escape
needs a new automated test** — `human-checklist` and `fixed-instance-only` are valid,
honest dispositions.

Status is `open` or `closed` (whether the disposition's follow-up action landed).

<!-- escaped-defects:start -->
| Ref | Origin class | Detected at | Earliest prevention point | Disposition | Status |
| --- | --- | --- | --- | --- | --- |
<!-- escaped-defects:end -->

## UX (ADR 0074)

Which journeys got shorter/longer this milestone, measured. `Measured now` / `Prior`
are the `tests/browser/journeys/budgets.json` floors this milestone vs. last; `Δ` is
shorter / longer / unchanged (a shortened journey means the floor was re-ratcheted
down). Only list journeys this milestone touched.

| Journey | Budget (ceiling) | Measured now | Prior | Δ |
| --- | --- | --- | --- | --- |
| J<n> — <name> | ≤<ceiling> | <floor> | <prior floor> | shorter / longer / — |

**Still-open UX items** *(feed the guardrail-harvest loop)*
- <friction found but not fixed; journeys that grew and why; missing journey coverage>

## Net

One honest paragraph: the strongest pattern this milestone, the weakest moment, and
what is genuinely **still-open** and tracked (with hex7s) rather than closed.
