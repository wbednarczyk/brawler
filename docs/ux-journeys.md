# UX Journeys

Canonical catalog of user journeys — the cross-screen tasks the app exists for — per [ADR 0074](adr/0074-ux-journeys-and-anti-rot.md). This is the macro-level UX spec: [ui-flows.md](ui-flows.md) stays the per-feature behavior layer; journeys sit above it and reference it. Enforcement: one Playwright spec per journey with an interaction step budget ([testing.md](testing.md#user-journey-e2e-and-step-budgets-adr-0074)); every user-facing capability names the journey it serves ([Definition of Done §I](engineering-workflow.md#definition-of-done-the-handover-gate)).

Format per journey: **Trigger** (why the user shows up) → **Steps** → **Screens** → **Budget** (max interactions: clicks + key presses, asserted in E2E) → **Done well** (what must be true at the end). Budgets are calibrated by first measurement, then ratcheted; a step referencing a future milestone is tagged with its version and joins the spec (and the budget) when it ships.

Enforcement lives in `tests/browser/journeys/` (one `@journey` spec per journey) with the ratchet floors in `tests/browser/journeys/budgets.json`; the budget numbers below are the normative **ceilings** those floors must never exceed ([testing.md](testing.md#user-journey-e2e-and-step-budgets-adr-0074)).

## J1 — Morning review

- **Trigger:** opening the app at the start of the day.
- **Steps (F2 Dziś v2, #422 / ADR 0068 amendment 2026-08-20):** land on Today → read the **delta header** ("what arrived since your last visit" — the journey's entry point, a passive scan) → scan the per-day decision queue (today's calendar + unreviewed days; media pre-clustered per company) → open the 0–2 that matter via actions that name their destination and land ON the item (`Otwórz komunikat` → that filing selected in Inbox; `Otwórz tezę` → that claim highlighted in Claims) → mark the day reviewed → done.
- **Screens:** Today, Inbox detail, (Company workspace).
- **Budget:** ≤15 interactions at 10 new items.
- **Done well:** no unhandled high-signal item; under 10 minutes; the user knows *what changed and whether anything needs action*.
- **Redesign note (v0.50 phase 2):** the Today screen is being redesigned **to this journey** (task U-Rb, [plans/v0.50-ux-overhaul.md](plans/v0.50-ux-overhaul.md)) — J1's budget is the acceptance bar for that mockup.
- **J1b — the Inbox leg (F1, #413):** the morning review's Inbox pass (open Inbox → open a filing's per-kind detail → mark read → back to Today) is budgeted as its **own journey `J1b`** (`budgets.json`; ceiling ≤8 interactions) because J1's floors were already saturated by the Today↔workspace loop — extending J1 would have loosened an existing gate instead of adding one. Covered by `tests/browser/journeys/j1-morning-review.spec.ts` ("J1b — inbox filing review").
- **v0.55 ([ADR 0069](adr/0069-source-reliability-and-disclosure-signals.md)):** the triage stream gains two signal categories — `auditor_opinion` (auditor red flags, danger badge) and `short_position_change` (KNF register moves) — both alert-rule-capable, plus the system `source_reconciliation` attention event ("the primary source missed an official report", Today stream + briefing line). Journey shape and budget unchanged: they arrive as ordinary attention/triage items. The `shortPositions` cockpit panel (palette) and the Diagnostics reconciliation ledger are journey-independent readouts.

## J2 — A company published a report

- **Trigger:** autopilot/alert notification of a new periodic report.
- **Steps:** open the run card → review extracted KPIs (drift/diff views) → (v0.52: expectation-vs-actual review) → resolve claims-to-verify → capture a note / update the assessment.
- **v0.59 ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)):** the AI KPI-extraction launcher and its modal are gone with the in-app AI layer — extraction is deterministic and runs unattended, so the journey opens on *results*, not on triggering a run. Facts a deterministic tier cannot produce appear as explicit flagged gaps rather than an on-demand AI extraction the user starts. The journey's marked primary action moved to the Notebook's "New note" (the durable artifact of the journey); the claims-review Delivered/Missed pair is a two-peer-primary surface and needs an owner-approved multi-primary `reason` in its experience contract before it can carry the mark. The J2 budget floor was re-baselined in the same milestone — a shortening caused by feature removal, not by a UX improvement.
- **Screens:** Today, Inbox, Companies, Spółka (Fundamentals / Claims / Notebook workshop tools, F3a, ADR 0107).
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
- **Steps:** `Otwórz ekran: Report Season` → per-company pre-report card (open questions, unresolved claims, last KPIs, evidence) → write expectations (stance + optional metric expectations) → mark prepared.
- **Screens:** Report Season, Spółka.
- **Budget:** floor re-based 2026-08-26 at first measurement +1 (consent 5, ADR 0107) — the old ≤13 measured a flow whose 3 modal opens were view-creation overhead the F3a freeze removed.
- **Done well:** every near-report company has a reviewed card and recorded expectations before results land.

## J5 — Claim verification

- **Trigger:** the "claims to verify" queue resurfaces a due claim.
- **Steps:** open queue → claim beside its evidence (matching confirmed fact for quantitative claims) → set verdict → optional note.
- **Screens:** Claims (company workspace / review queue).
- **Budget:** ≤6 interactions per claim.
- **Done well:** verdict recorded against evidence; nothing left silently overdue.

## J6 — Buy / pass decision (full from v0.52, enriched through v0.64)

- **Trigger:** research maturity or a price condition (v0.54 alert: price enters my range).
- **Steps:** Company synthesis (fundamentals, quality score, red flags, analyst-recommendation context — attributed third-party opinions with a vs-target readout (v0.58), valuation range, thesis when available) → record the decision in the journal (kind + rationale + evidence links) → (v0.64: link to thesis, plan the review). The journal is reached as `Spółka → Otwórz dziennik decyzji` (F3a, ADR 0107; the old Add-panel path is frozen); budget floor re-based 2026-08-26 at first measurement +1 (consent 5).
- **Relative position:** the Compare screen and its `J6-compare` sub-flow were removed 2026-08-10 (#351, ADR 0089 amendment — unused in real practice); peer context lives in the Fundamentals periods × deltas table and, for agents, the MCP comparison/valuation reads.
- **Screens:** Spółka (quality + decision-journal workshop tools).
- **Budget:** ≤15 interactions for the recording flow (the thinking is not budgeted).
- **Done well:** the decision has a date, a rationale, provenance — and will come back for outcome review (NS2 calibration).

## J7 — Weekly review

- **Trigger:** weekend / recurring ritual.
- **Steps:** `Otwórz ekran: Events` (week calendar — what's coming) → `Otwórz ekran: Watchlists` (overview: heatmap + leaderboard) → `Otwórz ekran: Research` (review queue + gaps) → Spółka (deepening, via the review queue row's own "Open company" action — owner decision 2026-08-26, ADR 0107) → plan the week.
- **Screens:** Events, Watchlists, Research, Spółka.
- **Budget:** floor re-based 2026-08-26 at first measurement +1 (consent 5, ADR 0107) — the view-creation leg the old ≤9 measured is frozen; all four task legs stay, now entered through their own screens. Re-measured 2026-08-27: the deepening leg's row-level "Open company" action (watchlist and research rows, owner decision 2026-08-26) replaced the ⌘K palette round-trip, dropping a modal open; floor tightened to the new measurement +1.
- **Done well:** next week's dates are known; the backlog of research debts is explicit, not vague guilt.

## Journey-independent utilities

Settings, Diagnostics, Sources administration, import/export, the **MCP server** section (ADR 0078 — enable/port/token + connection snippets), and global search serve all journeys; capabilities there are declared `utility` in the DoD check rather than forced into a journey.
