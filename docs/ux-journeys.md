# UX Journeys

Canonical catalog of user journeys — the cross-screen tasks the app exists for — per [ADR 0074](adr/0074-ux-journeys-and-anti-rot.md). This is the macro-level UX spec: [ui-flows.md](ui-flows.md) stays the per-feature behavior layer; journeys sit above it and reference it. Enforcement: one Playwright spec per journey with an interaction step budget ([testing.md](testing.md#user-journey-e2e-and-step-budgets-adr-0074)); every user-facing capability names the journey it serves ([Definition of Done §I](engineering-workflow.md#definition-of-done-the-handover-gate)).

Format per journey: **Trigger** (why the user shows up) → **Steps** → **Screens** → **Budget** (max interactions: clicks + key presses, asserted in E2E) → **Done well** (what must be true at the end). Budgets are calibrated by first measurement, then ratcheted; a step referencing a future milestone is tagged with its version and joins the spec (and the budget) when it ships.

Enforcement lives in `tests/browser/journeys/` (one `@journey` spec per journey) with the ratchet floors in `tests/browser/journeys/budgets.json`; the budget numbers below are the normative **ceilings** those floors must never exceed ([testing.md](testing.md#user-journey-e2e-and-step-budgets-adr-0074)).

## J1 — Morning review

- **Trigger:** opening the app at the start of the day.
- **Steps:** land on Today/Pulse → (v0.54: read the morning briefing) → triage new attention items → open the 0–2 that matter → back to Today.
- **Screens:** Today, Inbox detail, (Company workspace).
- **Budget:** ≤15 interactions at 10 new items.
- **Done well:** no unhandled high-signal item; under 10 minutes; the user knows *what changed and whether anything needs action*.
- **Redesign note (v0.50 phase 2):** the Today screen is being redesigned **to this journey** (task U-Rb, [plans/v0.50-ux-overhaul.md](plans/v0.50-ux-overhaul.md)) — J1's budget is the acceptance bar for that mockup.

## J2 — A company published a report

- **Trigger:** autopilot/alert notification of a new periodic report.
- **Steps:** open the run card → review extracted KPIs (drift/diff views) → (v0.51: expectation-vs-actual review) → resolve claims-to-verify → capture a note / update the assessment.
- **Screens:** Today, Company workspace (Fundamentals / Report diff / Claims), Notebook.
- **Budget:** ≤25 interactions.
- **Done well:** facts confirmed or rejected, due claims resolved, a trace of the user's judgment exists in the notebook/journal.

## J3 — Onboarding a new company

- **Trigger:** deciding to track a company.
- **Steps:** Companies → registry lookup/autofill → add to watchlist → history backfill kicks off → (v0.53+: sector, ratios; v0.56: ownership; v0.57: health scores arrive automatically) → first note: "why I'm watching this".
- **Screens:** Companies, Watchlists, Company workspace.
- **Budget:** ≤12 interactions to reach "company fueled".
- **Done well:** after one session the company has feed, reports, fundamentals, and a recorded reason for being tracked.

## J4 — Report-season preparation

- **Trigger:** upcoming report dates across the watchlist.
- **Steps:** Report-season cockpit → per-company pre-report card (open questions, unresolved claims, last KPIs, evidence) → (v0.51: write expectations) → mark prepared.
- **Screens:** Report season, Company workspace.
- **Budget:** ≤10 interactions per company.
- **Done well:** every near-report company has a reviewed card and (from v0.51) recorded expectations before results land.

## J5 — Claim verification

- **Trigger:** the "claims to verify" queue resurfaces a due claim.
- **Steps:** open queue → claim beside its evidence (matching confirmed fact for quantitative claims) → set verdict → optional note.
- **Screens:** Claims (company workspace / review queue).
- **Budget:** ≤6 interactions per claim.
- **Done well:** verdict recorded against evidence; nothing left silently overdue.

## J6 — Buy / pass decision (full from v0.51, enriched through v0.64)

- **Trigger:** research maturity or a price condition (v0.54 alert: price enters my range).
- **Steps:** Company workspace synthesis (fundamentals, quality score, red flags, valuation range, thesis when available) → record the decision in the journal with rationale and evidence links → (v0.64: link to thesis, plan the review).
- **Screens:** Company workspace, decision journal.
- **Budget:** ≤15 interactions for the recording flow (the thinking is not budgeted).
- **Done well:** the decision has a date, a rationale, provenance — and will come back for outcome review (NS2 calibration).

## J7 — Weekly review

- **Trigger:** weekend / recurring ritual.
- **Steps:** week calendar (what's coming) → watchlist overview (v0.63: heatmap + leaderboard) → research gaps (v0.63 detector) → plan the week.
- **Screens:** Events/Calendar, (v0.63: watchlist command center).
- **Budget:** ≤20 interactions.
- **Done well:** next week's dates are known; the backlog of research debts is explicit, not vague guilt.

## Journey-independent utilities

Settings, Diagnostics, Sources administration, import/export, and global search serve all journeys; capabilities there are declared `utility` in the DoD check rather than forced into a journey.
