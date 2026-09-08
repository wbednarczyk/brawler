Reviewed current `HEAD 2d6000e2` against `master...HEAD`. No blocker found, but three robustness issues remain.

## R3 findings

1. **FIXED — parent outcomes/progress ignored durable counters and edge states.**  
   `ParentAggregate` now carries parent status/error, live and terminal member counts, `runs_failed`, and `skipped_existing` ([activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:654)). `progress()` includes skipped candidates and enqueue/member failures ([activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:675)); `resolve()` handles running parents, live members, failed zero-member parents, partial members, and mixed outcomes in the right order ([activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:705)). `inFlight` comes directly from live enqueued members ([activity.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity.rs:151)).

   The requested cases behave correctly:

   - Parent `queued`/`running` with zero members remains `running`.
   - A terminal failed zero-member parent remains failed.
   - A completed zero-candidate parent succeeds.
   - A skipped-only completed sweep succeeds with `done == total` and `inFlight == 0`.
   - Any still-running member keeps an otherwise terminal parent running.
   - Malformed member JSON propagates an error ([activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:780)).

   The new six-case test module is genuinely included ([activity_read_model_tests.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity_read_model_tests.rs:863)). It tests skip-heavy rather than literally skipped-only, but the skipped-only branch is mechanically correct.

2. **FIXED — recovery fallback replaced the real error and skipped terminal hooks.**  
   `SettleOutcome` now distinguishes committed, missing-row, and unrecovered results ([queue.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue.rs:164)). The ordinary handler error is copied into the guard before settlement ([queue.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue.rs:583)); non-panic Drop recovery reuses it and fires hooks after a terminal settle ([queue.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue.rs:273)). The real twice-failed-then-recovered test verifies the original error and one hook invocation ([queue_dispatch_tests.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue_dispatch_tests.rs:247)).

   There is no double-settle path: `Settled` and `MissingRow` disarm immediately; only `Unrecovered` reaches Drop. `pending_error` owns its `String`, is overwritten immediately before each relevant recovery attempt, and has no borrowing/lifetime defect.

3. **PARTIAL — domain lookup failures were collapsed into absence.**  
   The named report-reading and KPI helpers are fixed with `StorageResult<Option<_>>` and `.optional()` ([activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:461), [activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:476)); callers propagate them with `?` ([activity.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity.rs:236)). The forced corrupt-status test is non-vacuous ([activity_read_model_parent_aggregate_tests.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity_read_model_parent_aggregate_tests.rs:304)).

   However, the summary-critical stalled-occurrence lookup still uses `.ok()` and silently drops real SQL/decode failures. See finding 1.

## Remaining findings

1. **MAJOR — the summary still fails open on a stalled occurrence lookup.**  
   `activity_key_for_occurrence` converts every lookup/decode failure into `None` ([activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:493)). `resolved_key_statuses` then silently omits that stalled registry key ([activity.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity.rs:780)). This is not a cosmetic ticker/title fallback: losing the stalled key can allow a lower-precedence queued row with the same key to contribute to the topbar queued count.

   **Fix:** return `StorageResult<Option<String>>`, use `.optional()`, propagate with `?`, and add a corrupt-`activity_key` test against `compute_activity_summary`.

2. **MAJOR — `ParentAggregate` does not verify that all declared members were actually observed.**  
   `enqueued` is the JSON array length, while member counts come from an `IN` query ([activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:799)). A missing referenced run, duplicate ID, or valid-but-unknown status contributes to none of `live/succeeded/partial/failed` ([activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:805)). A completed parent whose declared member is missing can therefore resolve as `succeeded`.

   It also never verifies:

   ```text
   candidates_total == enqueued + runs_failed + skipped_existing
   observed members == unique declared member IDs
   ```

   The producer normally maintains these invariants, but the new code explicitly adopts a fail-closed posture for corrupted JSON and status rows; silently succeeding on broken references is inconsistent with that posture.

   **Fix:** reject duplicate IDs, require every declared ID to yield one recognized member status, and validate the candidate-counter equation once the parent is terminal. Add missing-member, duplicate-member, and counter-drift tests.

3. **MAJOR — arbitrary terminal hooks can still panic outside the unwind boundary.**  
   A non-panic Drop fallback invokes `failure_context`/attention handling and `handler.on_terminal_failure` directly inside `Drop` ([queue.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue.rs:287)). If either panics, the destructor propagates a panic after the original `catch_unwind` has finished, killing the worker thread. The terminal hooks in the outer panic arm have the same exposure ([queue.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue.rs:446)).

   Ordinary-path hooks happen inside the dispatch catch boundary, so the protection is currently path-dependent.

   **Fix:** centralize terminal hooks in a no-unwind helper that catches and logs each callback independently, and use it from the ordinary branch, panic arm, and non-panicking Drop fallback. Keep the deliberate “no callbacks while already unwinding” rule. Add a handler whose `failure_context` and `on_terminal_failure` panic, verifying the worker survives and no second settle occurs.

## Verdict

**SHIP-WITH-FIXES**

The normal parent lifecycle and settle-recovery defects from R3 are fixed. The remaining issues concern corrupted-reference handling and panic containment in exceptional recovery paths; they should be corrected before treating the branch as fully hardened.

Static/read-only review only; I did not run builds or tests.

Codex session ID: 01a06b71-69c0-7293-868b-410699df3638
Resume in Codex: codex resume 01a06b71-69c0-7293-868b-410699df3638
