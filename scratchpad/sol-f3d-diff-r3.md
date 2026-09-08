Inspected the current tip, `6af58911` (including `4511bce1`), against `master...HEAD`. Read-only review; I did not run test/build commands or modify files.

## R2 findings: current disposition

1. **FIXED — failed settle could leave the claim and occurrence running.**  
   `settle_with_recovery` retries once and disarms only after `Ok` or the specially committed missing-row outcome; otherwise `ClaimGuard::drop` remains armed ([queue.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue.rs:214), [queue.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue.rs:175)). The corrected test now requires both rows to be terminal ([queue_dispatch_tests.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue_dispatch_tests.rs:218)).

2. **FIXED — outer/scaffolding panic bypassed terminal hooks.**  
   The panic arm performs the ordinary failure settle, distinguishes retry from terminal, and calls both failure surfaces only on `Some(false)` ([queue.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue.rs:322)). The max-attempt scaffolding-panic test verifies one `on_terminal_failure` call ([queue_dispatch_tests.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue_dispatch_tests.rs:435)). A separate recovery-path hole remains below.

3. **PARTIAL — completed mixed-member parents inherited the wrong outcome.**  
   Succeeded+failed members now produce `partial` independently of the parent occurrence ordering ([activity.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity.rs:248), [activity_read_model_tests.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity_read_model_tests.rs:447)). However, zero-member failed parents, enqueue failures, skipped candidates, and partial child runs remain incorrectly derived; see new finding 1.

4. **FIXED — direct work could start without registry registration after identity checkout failure.**  
   Source, registry and aggregator identities are connection-free ([activity_identity.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/activity_identity.rs:126)); backfill has an explicit fallback ([activity_identity.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/activity_identity.rs:170)) and always calls `activity_registry::start` before its core ([backfill.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/backfill.rs:97)).  
   The fallback’s raw company ID is degraded but honest subject data. Its `history-fetch:<company_id>` key is identical to the full identity’s key ([activity_identity.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/activity_identity.rs:316)), so collapse identity is stable.

5. **PARTIAL — nested read errors were treated as absent data.**  
   `parent_status`, parent progress, and member queries now return `StorageResult` ([activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:470), [activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:516)). But report-reading and KPI status lookups still erase all SQL errors with `.ok()` ([activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:454), [activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:482)). See new finding 3.

6. **FIXED — outcome labels lied, and registry refresh carried Polish backend prose.**  
   Registry refresh now uses an empty raw subject ([activity_identity.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/activity_identity.rs:145)). The seven status labels distinguish exact states ([activityLabels.ts](/home/wojtas/projects/brawler/src/shared/components/activity/activityLabels.ts:67)).  
   The English phrases are acceptable product copy. They are accurate and avoid collisions in the repository’s English-source-string `text()` catalog. Plain `Failed`/`Succeeded` would require a namespaced-key localization redesign. “Partially completed” would be slightly more idiomatic than “Partially finished,” but the current phrase is not false or release-blocking.

7. **FIXED — contract/data-model/wiki drift.**  
   The contract now documents `members`, member-derived parent status, and real progress semantics ([contracts.md](/home/wojtas/projects/brawler/docs/contracts.md:1598)); the data model describes the bounded candidate scan and Rust collapse ([data-model.md](/home/wojtas/projects/brawler/docs/data-model.md:1136)); the wiki describes the built destinations and behavior ([activity.md](/home/wojtas/projects/brawler/wiki/activity.md:25)).

8. **FIXED — several tests were vacuous or missing.**  
   The cap test is now duplicate-heavy ([activity_tests.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity_tests.rs:194)); all five real stages plus a retry are exercised ([activity_tests.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity_tests.rs:314)); reconciliation retention and real-wrapper panic coverage exist ([tests.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/activity_reconcile/tests.rs:569), [source_refresh_tests.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/source_refresh_tests.rs:582)).  
   `queue_dispatch_tests.rs` is genuinely compiled through the nested module declaration ([queue_tests.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue_tests.rs:575)). Its remaining assertion gap is described below.

## Remaining/new findings

1. **BLOCKER — parent status and progress still lie for legitimate terminal states.**  
   `parent_progress` reads only `candidates_total` and member IDs; it ignores durable `runs_failed` and `skipped_existing`, treats malformed member JSON as an empty list, and returns `(0,total,0,0)` whenever no IDs decode ([activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:656)). It also counts child `partial` as fully done/successful ([activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:680)).

   Consequences:

   - A storage-level `status='failed'` sweep with zero members is overridden to `succeeded`.
   - A completed sweep/batch with only `runs_failed` is shown as `succeeded`.
   - Mixed successful members plus enqueue failures are shown as fully succeeded.
   - Skipped candidates remain included in `total` but never in `done`; `inFlight = total-done-failed` therefore reports already-skipped work as still running ([activity.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity.rs:163)).
   - A parent whose only child is `partial` becomes `succeeded`.

   The decisive branch is the unconditional terminal fallback to `succeeded` when the derived member failure count is zero ([activity.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity.rs:253)). This contradicts the stored counters documented in [data-model.md](/home/wojtas/projects/brawler/docs/data-model.md:1184) and the “all members” contract.

   **Fix:** return a typed aggregate containing parent status/error, enqueued count, live count, succeeded/partial/failed member counts, `runs_failed`, and—for sweeps—`skipped_existing`. Compute `inFlight` strictly from non-terminal enqueued members. Preserve a failed zero-member parent as failed; zero-candidate completed is succeeded; any partial child or enqueue failure mixed with success/skip is partial. Reject malformed `enqueued_run_ids_json` as a storage/data error. Add tests for all five cases.

2. **BLOCKER — successful Drop recovery loses the real error and skips terminal failure hooks.**  
   After both ordinary settle attempts fail, `settle_with_recovery` returns `None` with the guard armed ([queue.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue.rs:214)). `Drop` then performs a different failure settle using the fabricated message `"panic: job worker unwound before settling"` even when no panic occurred ([queue.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue.rs:175)).

   If that fallback successfully terminalizes a max-attempt job:

   - the handler’s real error is overwritten by a false panic diagnosis;
   - `surface_terminal_failure` and `on_terminal_failure` never run, because those calls occur only when the original helper returns `Some(false)` ([queue.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue.rs:470));
   - the new test explicitly exercises this path but asserts only queue/occurrence statuses, not the preserved error or hook count ([queue_dispatch_tests.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue_dispatch_tests.rs:268)).

   **Fix:** make ordinary recovery retain and repeat the intended transition/error and return a typed result such as `Settled(T) | MissingRow | Unrecovered`. If a final explicit recovery returns terminal failure, invoke hooks once. Reserve `ClaimGuard::drop`’s panic message for actual unwinding. Add assertions for the original error and exactly-one terminal hooks on the twice-failed-then-recovered path.

   The normal retry itself is otherwise safe: queue updates are repeatable and occurrence settlement is idempotent via `WHERE status='running'` ([job_runs.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/job_runs.rs:104)). `JobQueueRowMissingDuringSettle` is also handled correctly: the occurrence transaction has already committed, the guard disarms, and no queue-terminal hook is fabricated ([jobs.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/jobs.rs:407)).

3. **MAJOR — report-reading and KPI domain override failures remain fail-open.**  
   Both status helpers return `Option` and collapse missing tables, schema errors, type errors, and absent rows into the same `None` ([activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:454), [activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:482)). Their callers then retain the ledger status, potentially showing `succeeded` when the domain lookup actually failed ([activity.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity.rs:215), [activity.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity.rs:264)).

   **Fix:** return `StorageResult<Option<(status,error)>>`, use `.optional()?`, and propagate with `?`. Keep `None` only for a genuinely absent row. Apply the same distinction to any summary-critical occurrence lookup currently using `.ok()`.

## Verdict

**DO-NOT-SHIP**

The queue’s ordinary and panic paths are substantially safer, the fallback identities are stable, the split tests are compiled, and the copy is acceptable. But the current tree can still report failed parent work as successfully completed, and its last-resort settle path can silently suppress the product’s required terminal-failure surfaces while replacing the real error with a false panic message.

Codex session ID: 01a06b71-69c0-7293-868b-410699df3638
Resume in Codex: codex resume 01a06b71-69c0-7293-868b-410699df3638
