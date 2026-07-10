# Trusted-extraction epic — session handoff (2026-07-10, F0–F6 COMPLETE incl. live-drive + T-C1/T-C2; T-C3 finalization in progress)

Fresh-session entry point for continuing the epic. Normative spec stays in
[trusted-extraction-foundations.md](trusted-extraction-foundations.md) and
[ADR 0077](../adr/0077-trusted-extraction-foundations.md) (now carrying dated
kickoff/F3b/§6 annotations — read them, they amended several original lines);
this file is only the state snapshot + pointers. Ground first per
[plans README](README.md): CLAUDE.md + engineering-workflow.md, then
`rad issue show 25cd300` (every task's QG evidence is a comment there).

## Where things stand (all UNCOMMITTED on `feat/trusted-extraction-foundations`)

Owner decision: **one commit at epic end** — edit, never commit/push. Branch
tip is still `261571e`; ~170 modified/new files carry F1–F5. Full `make check`
under Nix was green (`EXIT=0` grepped from the log) after every task; last
green: post-T5.3.

- **F0 + T5.1** — in `261571e` (ground truth, metrics ratchet, 429 pacing fix).
- **F1 taxonomy** — done. `doc_kind` (+ESEF-package amendment: bare `.xbri` =
  `periodic_ssf`), G-2 corpus `src-tauri/testdata/doc_titles_labeled.json`,
  migration 0061, idempotent `reclassify_report_documents`,
  canonical-report-per-period (`fundamentals/extraction/classify.rs`).
- **F2 coverage map + Panel B** — done (mockups approved + committed).
  `get_fundamentals_coverage` (computed read model), `CompanyCoveragePanel`,
  grouped `CompanyReportDocumentsPanel`, `get_report_documents_view`.
- **F3 history sweep** — done, live-verified on the owner's app (CBF).
  `jobs/history_sweep.rs` (selector = projection of the coverage read model),
  migration 0062 (`history_sweeps` + `autopilot_run.trigger` CHECK rebuild),
  shared `enqueue_extraction_run`, backfill chaining, Coverage footer
  (Backfill history / Extract missing periods, poll-until-settled),
  `backfill_years` setting, `truncated` honesty, reclassify→revision bump.
- **F4 vision tier-4** — done, live-verified with the owner's real Mistral key
  (bootstrap → 4 proposals → confirm bumped profile to v2 → second run
  committed 3 validated facts). Mistral provider (`providers/analysis/
  mistral.rs`; OCR stitches `pages[].tables`), pure OCR parser + profile
  (`fundamentals/extraction/ocr/`), migrations 0063/0064,
  `AiCapability::VisionExtraction` (pool is EXPLICIT-only),
  `run_tier4_extraction` + `Tier4Gate` hook, confirm-path validates (end of
  `validation_status='none'`, G-1 guard), manual KPI job rewired to the same
  implementation. **Facts require a CONFIRMED (v≥2) profile; confirming an
  `ocr_bootstrap`/`ocr_pending_profile` proposal is what confirms it.**
- **F5 T5.2+F3b + T5.3** — done. Per-sweep AI budget (setting
  `history_sweep_ai_call_limit` 0–500, default 30, 0=off; migration 0065;
  limit snapshotted on the sweep row; atomic guarded-UPDATE consume, 2-thread
  race test; **G-4**: never exceeds, never silent — `skipped_budget` delta
  reason), F3b lifted (sweeps enter tier-4 under budget; the
  no-vision-provider pre-check does NOT charge), coverage `skipped_budget`
  projected from run deltas, Settings→AI budget control, Coverage footer
  "AI: used/limit".

## Next steps, in order

F0–F6 are complete and live-verified (F6 shipped period-derivation fallback for
non-iXBRL XHTML, best-extractable-document sweep selection, association guard +
startup repair, `unsupported_market` honesty, tier-4 degrade log trail +
un-flattened run-summary reasons, migration 0066 annual→FY, build-freshness
guard, positional `html_positional` tier (33/33 vs ground truth), and run
re-arm on capability upgrades). T-C1 (recall/precision re-measure + ratchet
re-pin) and T-C2 (permanent live journey spec) are done. Remaining:

1. **T-C3 finalization** (this pass): docs de-`planned` sweep, wiki/ update,
   CHANGELOG `## Unreleased` draft, this handoff refresh — DONE in this pass.
2. **Retro** — both domains (app + dev loop) × well / wrong / stop / improve,
   each item marked closed or still-open honestly; feed still-open into the
   guardrail-harvest loop.
3. **Owner sign-off** — the ground-truth pass-2 value spot-check (below) plus
   the closure verdict.
4. **ONE squash commit → `make release`** via the brawler-release skill
   (closes as `v0.51.0`; the roadmap trusted-extraction line is removed at
   release, its delivered detail curated into the `## Unreleased` CHANGELOG
   draft).

## F5 completion record (2026-07-10, this session)

- **T5.3b done** (mockup approved → subagent C): read model
  `list_pending_kpi_proposals`, pinned kind `review`, coverage To-review cell
  → panel, i18n, mock-fidelity pin, visual baselines (`review-queue` ×4).
- **Confirm is slot-aware** (defect found live, fixed in-branch, subagent D):
  identical value → re-observed (links the EXISTING fact, no dupe, provenance
  untouched, still bumps a pending OCR profile); different value → typed
  `value_conflict` (both values), nothing written; panel shows a localized
  message. Spec: data-model.md; guard: storage tests.
- **Live evidence** (owner app + real Mistral): budget snapshot (limit 2 on
  sweep row), LPP backfill+sweep consumed exactly 1 unit (`AI: 1/2` footer),
  `bootstrap_failed` degraded honestly; CBF panel journey confirmed an OCR
  proposal through the panel (queue 28→27, re-observed, facts count
  unchanged). `skipped_budget` not exercisable on current real data — G-4
  unit tests + race test stand as that branch's evidence (per approved plan).
- **Findings → cards**: 276ecd2 (interim non-iXBRL XHTML unextractable +
  tier-4 PDF-only, HIGH), tier-4 degradations leave no production diagnostics
  (medium), fresh mis-association evidence on 45fcece (Energa/Vercom hold the
  only CBF candidates; Gobarto holds CDR 2023 H1).
- **Owner-requested validation matrix** (12 companies live): all reporting
  types covered — ESEF `.xbri`/`.zip` (deterministic), PDF text-layer tier,
  tier-4 bootstrap under budget (7 proposals landed), `skipped_budget` lit
  (GPW/VRC/SNT/KGH), `bootstrap_failed` + 429 `provider_limit` degrades,
  interim non-iXBRL XHTML (276ecd2), NC/EBI backfill fails without
  diagnostics (card). Two defects fixed in-branch red-first: static
  ineligibility charged a budget unit (pre-check before consume); coverage
  poll false-settle/give-up → explicit `chainedSweepId` threaded through
  `BackfillProgress`, poll tracks that sweep. Probe-contamination lesson:
  unscoped pane locators can read a NEIGHBOUR company's pinned pane —
  `data-company-id` added to coverage+review panels, live specs must scope.
- Permanent live spec: `tests/live/t5-budget-sweep.live.spec.ts`
  (BRAWLER_LIVE_AI-gated, envelope assertions, company-scoped panes). Probes
  deleted.
- Inspiration cards from the owner's X find: 03cfba1 (adversarial
  challenge-my-thesis, NS1), c4cf5c1 (epistemic tiers on AI conclusions).
- **Process rule (owner, 2026-07-10)**: verify a live-defect fix LIVE first;
  the full `make check` runs ONCE at the end (scoped tests guard the inner
  loop).

## Process rules active in this epic (beyond CLAUDE.md)

- **Delegation standing rule (owner, 2026-07-10; in CLAUDE.md + memory):**
  orchestrator plans + quality-gates; implementation goes to cheaper-model
  subagents (Opus; Sonnet for routine slices) from written contracts that
  enumerate tests-that-redden and REQUIRE pasted red evidence; **up to 3
  subagents concurrently** — never stacking heavy builds.
- **WSL OOM guardrail** (testing.md § Resource discipline; a test run killed
  the owner's WSL once): one heavy build/test invocation at a time,
  `CARGO_BUILD_JOBS=8`, nextest scoped to touched modules. Post-crash SIGSEGV
  in `nextest --list` = corrupted `target/debug/deps` binary — delete+relink.
- **Gate evidence**: only `EXIT=` grepped from the make-check log counts; read
  it BEFORE posting any claim (one on-card correction taught this).
- **Tests bar (owner)**: right quantity+quality of reddening tests, not
  ritual; per-behavior enumeration in every contract.
- Visual baselines: regenerate deliberately on a fresh server; `shootPanel`
  maximizes the dockview group (occlusion fix, T3.2). New-panel work is
  mockup-first.
- Live-drive: standing authorization (`make live-up`/`live-cycle`; CDP url in
  `/tmp/brawler-live-cdp-url`). Real DB read-only:
  `/mnt/d/Brawler/Builds/latest/data/brawler.sqlite3` (+`-wal`) — copy both to
  the scratchpad, `PRAGMA wal_checkpoint` on the copy. Live specs:
  `tests/live/` (`t32-history-sweep`, `t4-vision-tier4` gated
  `BRAWLER_LIVE_AI=1`, `t7-round2`).

## Owner-pending (do not block; remind once if relevant)

- **Mockup approval for T5.3b** (step 1 above).
- **Ground-truth pass-2 value spot-check** — needed before T6.2 writes final
  numbers into ADR 0077 (the 3 pass-2 decisions are already applied).
- Coverage-invalidation card 3579234 is fixed + `state:review` — owner closes.
- GitHub release binaries green-check for v0.50.0 (if still unverified).

## Open cards filed during the epic (beyond `25cd300` / parent `971aff6`)

- **HIGH** tier-4 proposals unreachable for review → closes with T5.3b.
- **HIGH** live Inbox shows 0 items on "Wszystko" while feed has current rows
  (found in F4 live-drive; NOT diagnosed — likely regression, needs repro).
- **HIGH** Playwright `reuseExistingServer` stale-build false-green.
- Mis-associations steal canonical slots (45fcece, fresh F3 evidence: Energa/
  Vercom PDFs held CBF 2024 Q3/H1 canonical slots at sweep time).
- `annual` period-label repair migration (medium).
- Retire/repoint orphaned `AiCapability::KpiExtraction` (low, post-F4).
- Flaky-tests card `b6b866f` (TodayScreen assertion, Notebooks a11y).

## Live app state (owner's machine)

The running build is F4-final (`live-up`); T5.2/T5.3 need a rebuild before the
F5 live-drive. CBF real data now carries: OCR profile v2 (Millions, Polish
label map), 3 tier-4 facts + 1 confirmed (all `ai`/real statuses), 3 pending
OCR proposals, sweep rows incl. `trigger='backfill'`. The `vision_extraction`
pool routes to Mistral; the owner's key is in the keychain.
`backfill_years`/`history_sweep_ai_call_limit` untouched (defaults 3/30).

## Session gotchas (details: testing.md)

Pre-validate commit subjects (`scripts/release/validate-commit-message.sh
--message "…"`, 72-byte cap) before any gated commit; regenerate
`docs/adr/INDEX.md` via `docs-drift --write-adr-index`; CLAUDE.md and
`engineering-workflow.md` sit near their ADR 0063 byte budgets (add content
elsewhere — the gate catches overruns); generated TS only via `rtk make types`
(raw export flips i64→bigint across ~44 files); `rtk grep` misfires on pipes/
alternation — use `grep -F` or `-nE`; run `make mutants` as a `systemd-run
--user` unit, never concurrent with a full gate on this box.
