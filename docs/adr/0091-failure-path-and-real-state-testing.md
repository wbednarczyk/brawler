# ADR 0091: Failure-Path & Real-State Test Layer — chaos seam, job-failure visibility, honesty harness

Status: Accepted (2026-07-29, owner sign-off at epic #40 planning — Polish plan approved in chat). Decision 4 narrowly amended by [ADR 0094](0094-committed-public-espi-report-samples.md) (official ESPI/EBI filing files as committed test samples; everything else unchanged). Cadence language ("closure/epic-gate runs") superseded by [ADR 0096](0096-quality-gate-architecture-under-continuous-release.md) — audits are risk-triggered or on-demand, the PR is the only gate.

Deciders: maintainer. Area: testing, jobs, storage, frontend.

The epic #40 ADR. Execution detail (non-normative): `docs/plans/failure-path-real-state.md`.
Complements the dogfooding loop (ADR 0081) — it does not replace human judgment; it moves the
classes of bug only the owner was catching (silent job failure, dishonest zero-effect success,
poor-data-state rendering) into automated gates.

## Context

Harvest of the post-v0.50 defect record: every owner-caught bug lived where automation doesn't
look. The mock runtime knows only the happy path (a one-shot `failNext` seam exists in
`src/test/scenarios/runtime.ts` but is not reachable from browser specs); there are no poor-state
scenario seeds; and nothing measures the app's honesty on the real database. Recon (2026-07-29)
confirmed the sharpest gap is product, not test-only: **5 of 12 registered job kinds have no
user-visible failure path at all** (`quote_backfill`, `morning_briefing`, `history_sweep`,
`ownership_extraction`, `management_holdings_extraction`), 2 more only incidentally. `job_queue`
has no read model, no IPC command, no frontend surface. `attention_events.company_id` is `NOT NULL`
(migrations 0077/0081), so a system-scoped event needs a table rebuild.

## Decisions

1. **One generic failure surface for jobs.** A terminal job failure (retries exhausted — the single
   `mark_failed` → `Ok(false)` point in `jobs/queue.rs dispatch()`) emits a system `job_failed`
   attention event (severity Notable for all kinds; transient hiccups never fire). Kinds with a
   richer domain surface (Sources adapter health, autopilot run card) keep it **exclusively** via a
   classification map `jobs::failure_surface(kind)` — no double-firing. Failure subjects are raw
   specifics (document title, ticker), never prose (ADR 0087 decision 4).
2. **`attention_events.company_id` becomes nullable** for system-scoped events, via an append-only
   rebuild migration (data-model.md rules; designated the first citizen of the #151 migration
   corpus).
3. **The class rule is enforceable, not aspirational.** Every registered job kind must (a) classify
   its failure surface — an enumeration gate over `registered_kinds()` reddens on an unclassified
   kind — and (b) ship a visibility test asserting its failure actually reaches that surface.
   Dev-gated Diagnostics is explicitly NOT an acceptable surface. A new job kind reddens twice:
   unclassified, then classified-without-a-test.
4. **Real data never enters the public repo or default CI** (personal investment research; 121 MB
   forever in git history). Honesty measurement splits: a **local ratchet on the real DB** (harness
   keyed on `BRAWLER_REAL_DB`, runs in `check-epic` with a loud SKIP elsewhere, never in
   `make check`; metrics via the real read models, never parallel SQL) plus a **public synthetic
   shape corpus** in CI (seed factory grown to cover an anonymized shape inventory of the real DB;
   hard invariants, no ratchet, no secrets). Explicitly deferred option, separate decision if ever
   needed: a DB copy in `brawler-private` driven by a manual workflow like mutants.yml.
5. **One source of truth for the filename pattern across languages**: canonical TS
   (`src/screens/Today/documentTitle.ts`), Rust mirror guarded by an include_str parity gate
   (idiom: `mcp/registry.rs`). Prevents the specificity metric and the UI from disagreeing on what
   counts as a filename-as-statement.

## Consequences

- Chaos becomes reachable end-to-end: bridge + persistent rules + `?chaos=` URL param; browser
  specs assert failures are NAMED, not blank (kills the drifted hand-copied overlay union as a
  side effect).
- Poor-state overlays + key-flow walks become part of the browser suite (Today, Sources triage,
  company cockpit).
- Honesty metrics with committed baselines: `specificity_pct` (floor), `orphaned_evidence`
  (ceiling), `filename_as_statement` (hard zero), `zero_effect_successes` (hard zero),
  `silent_missing_metrics` (downward ratchet) — regressions fail `check-epic` on the owner's
  machine; the synthetic corpus keeps the invariant class alive in public CI.
- Delivery is six slices, one PR each (S1 chaos seam → S2 poor states → S3 job-failure visibility →
  S4 real-DB harness → S5 effects honesty → S6 synthetic corpus); cards #105, #74, #234–#237 under
  epic #40. Follow-ons feed #139, #151, #181, #182, #201.

**Amendment (2026-07-29, S5 as-built).** `zero_effect_successes` ships as a **ratcheted ceiling**,
not the hard zero above. The first real measurement found 82 of 334 recorded extraction outcomes
already in the dishonest state — a success recording `fact_count = 0` beside
`reason_code = "emitted"` — because a re-run upserts the outcome row and overwrote a healthy count
with the *newly produced* count. S5 fixes the producing path (the row now records the facts **at**
the slot: produced plus re-observed) and the invariant is enforced hard where it belongs — a
`make check` unit gate over every run-summary shape (`effects_honesty::ExplainsEffect`). A hard
bound seeded above zero is not a bound, so the stored residue is ratcheted down instead and the
metric becomes the hard zero this ADR asks for on the run that first measures it. Repairing the
stored rows needs a forward migration and is tracked separately (#243), not smuggled into a test slice.
`silent_missing_metrics` measured `0` on the first run — the health read model already names every
missing input — so it is a ceiling at zero and no read-model field had to be added.

**Amendment (2026-09-03, [ADR 0109](0109-activity-center-occurrence-ledger.md), #133).** Decision 1's
exclusivity governs *notification*: a terminal failure is announced once, on the surface
`jobs::failure_surface` names. The Activity panel is a *status ledger* — it may list the same
failure as the task's outcome (raw error, an "Otwórz" to the item's home) without being a second
announcement. Dev-gated Diagnostics remains no surface at all.
