Verdict: **DO-NOT-SHIP**

I kept the audit read-only using the CLI write guard and used repository-context caller tracing. No files changed; the pre-existing untracked `scratchpad/` remains untouched. I did not rerun `cargo nextest` because that would write build/test artifacts, so the claimed 2302/2302 result is not independently verified.

## Blockers

1. **Queue lifecycle panics outside the handler can permanently kill a worker and strand a claimed job.**

   Identity lookup and `begin_attempt` run before `catch_unwind` in [queue.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue.rs:183); settlement runs after it at [queue.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue.rs:254). Only validation/handler execution is protected at line 238. A panic in identity resolution, `begin_attempt`, defer, or settle escapes the worker loop; `spawn_pools` only handles returned `Err` at [queue.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue.rs:389).

   Fix: wrap the entire post-claim lifecycle in unwind containment and add a claim/occurrence RAII guard that defers or terminalizes on unwind. Add injected panics at identity, begin, defer, and settle—not only inside `handler.run`.

2. **`mark_failed_with_run` leaves an occurrence `running` when its queue row has disappeared.**

   [jobs.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/jobs.rs:380) reads the queue row and returns `Ok(false)` at lines 387–390 when absent, before settling the occurrence at lines 420–427. Dispatch then treats that as terminal failure and invokes terminal hooks at [queue.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue.rs:258).

   Fix: when an exact `run_id` exists, settle it in the same transaction even if the queue row is missing, and return a distinct invariant error so terminal hooks do not pretend the queue transition occurred. Add a row-deletion fault test. `mark_succeeded_with_run` should likewise verify that exactly one queue row was updated.

3. **The direct-activity guard can hide a durable `running` occurrence while the app remains alive.**

   The guard sets `settled = true` before attempting storage settlement, logs and ignores failure, then removes the registry entry at [activity_registry.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_registry.rs:108). Drop therefore cannot retry. In addition:

   - `start` returns `None` on `begin_attempt` failure and callers continue the real work unrecorded at [activity_registry.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_registry.rs:141).
   - The registry is one `HashMap<String, i64>` entry per key; concurrent same-task attempts overwrite each other at line 160, and either guard removes the other attempt at line 120.
   - Direct settlement and retention are separate autocommit statements, not the documented transaction, at [job_runs.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/job_runs.rs:209).

   Fix: track a set/refcount of occurrence IDs per activity key, remove only the guard’s own ID, and retain a recoverable live entry until durable settlement succeeds. Make settle+prune transactional and test concurrent same-key work plus forced settle failure.

4. **`recent` never takes final status from the domain row; `partial` is effectively unreachable.**

   [activity.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity.rs:412) copies `job_runs.status` directly. There is no run/sweep/batch resolver, despite the normative rule in [data-model.md](/home/wojtas/projects/brawler/docs/data-model.md:1136). Since `job_runs` cannot store `partial`, a partial report run or mixed sweep/batch can never appear as partial; a succeeded queue occurrence can mask a failed domain result.

   Fix: resolve and override status/error from `autopilot_run`, `history_sweeps`, re-extraction batches, and KPI runs on the borrowed connection. For sweeps, distinguish fan-out completion from member completion. Add succeeded-occurrence/failed-domain and partial-domain tests.

5. **The “one item per task” invariant breaks across active, queued, recent, and summary.**

   Active and queued are collapsed independently at [activity.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity.rs:420), while recent is not excluded by live keys. A sweep with one running child, pending children, and its completed fan-out occurrence can therefore appear in all three sections. Summary counts raw occurrence/job rows, not activity keys, at [activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:481), and adds stalled rows to `active` at [activity.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity.rs:436). Consequently the icon can spin forever for stalled—not executing—work at [ActivityIndicator.tsx](/home/wojtas/projects/brawler/src/shared/components/activity/ActivityIndicator.tsx:22).

   KPI queue rows plus the KPI lease projection create the same duplicate-count risk.

   Fix: compose all sources into one keyed task map with explicit precedence (`running > stalled > queued > recent`), derive summary from those unique keys, and do not use stalled count to drive the work-in-progress spinner.

6. **Transcript post-provider storage failures leave the domain job `running` until restart.**

   The transcript row becomes running before its activity guard at [transcript_runner.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/transcript_runner.rs:31). Any segment insert failure at lines 45–56, completion write failure at lines 59–61, or failure-write error at lines 69–71 returns early. The guard records `interrupted`, but the transcript domain row remains `running`; the live read model only projects queued transcript rows and has no stalled-transcript derivation.

   Fix: funnel every post-running exit through one finalizer that best-effort terminalizes both the transcript row and occurrence. Add stalled transcript detection and injected failures for every early-return branch.

## High severity

7. **A failed queued backfill is recorded as a successful occurrence.**

   `backfill_company_history` records failure in `BackfillProgress`, but `run_company_backfill_job` discards that result and always returns `Ok(())` at [backfill.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/backfill.rs:348). Dispatch therefore settles the occurrence succeeded.

   Fix: return `Err(progress.error…)` when status is failed, allowing the queue’s retry/terminal path to record the truthful outcome.

8. **A real direct registry-refresh caller bypasses instrumentation, and the claimed awaited-path gate does not exist.**

   `lookup_company` invokes the unwrapped registry core at [companies.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/companies.rs:45), specifically line 59, while the instrumented wrapper is [source_refresh.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/source_refresh.rs:760). Thus a potentially long lookup-triggered refresh has no occurrence.

   The identity gate only compares `registered_kinds()` with queue payload fixtures at [activity_identity/tests.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/activity_identity/tests.rs:183); it does not enumerate direct paths.

   Fix: route lookup through the direct wrapper and create a single compile-time awaited-path registry consumed by wrappers and the gate. Test actual Tauri and MCP entry points.

9. **KPI identity trusts the duplicated payload instead of the authoritative claimed job ID.**

   [activity_identity.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/activity_identity.rs:407) reads `runId` from JSON before preflight. The KPI subsystem explicitly defines the job ID as authoritative and already parses it at [kpi_ingest_queue.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/kpi_ingest_queue.rs:78). A tampered payload can therefore attribute the occurrence to the wrong run before validation rejects the job.

   The “real payload” fixtures use impossible IDs and mismatched `jobId: "x"` values at [activity_identity/tests.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/activity_identity/tests.rs:167).

   Fix: expose/reuse the canonical job-ID parser for identity; treat payload fields only as coherence checks. Seed exact production validate/commit IDs and add tampering tests.

10. **The seven-day window admits rows up to almost a day too old.**

   Stored timestamps use `T…Z`, while `datetime()` returns `YYYY-MM-DD HH:MM:SS`. The lexical comparison at [activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:211) therefore treats any timestamp on the cutoff date as greater than the space-form cutoff. For example, at noon, 01:00 on the cutoff date incorrectly passes.

   Fix: compare `julianday(finished_at)` values or format the cutoff identically. Add exact boundary, one-second-old, and timezone/fractional-second cases.

11. **Closing or disabling the panel does not cancel a pending coalesced follow-up.**

   `useCoalescedResource` only gates calls by the controller-wide license `enabled`; its `finally` recursively runs a pending request at [useActivityController.ts](/home/wojtas/projects/brawler/src/app/useActivityController.ts:80). Closing clears the interval but neither clears `pendingRef` nor invalidates the in-flight response at lines 134–142. A tick queued before close can therefore fetch and update the view while closed; an old generation can flash after reopen.

   Fix: gate the view resource by `enabled && open`, maintain a request generation/epoch, clear pending on close/disable, and ignore results from earlier generations. Add the required slow-response close/reopen test.

12. **Startup reconciliation is not fail-closed and its “exact prefix” match is not exact.**

   Open occurrences are settled one by one without a transaction; the first error aborts the loop at [activity_reconcile.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/activity_reconcile.rs:46). The caller logs the error and still starts worker pools at [lib.rs](/home/wojtas/projects/brawler/src-tauri/src/lib.rs:206), potentially leaving old occurrences running.

   Autopilot liveness uses an unescaped SQL `LIKE` at [activity_reconcile.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/activity_reconcile.rs:94) and [activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:369). `_` in real IDs is a wildcard, so an unrelated row can falsely make a stranded run look live.

   Fix: bulk-interrupt occurrences in one transaction and refuse to start pools if essential reconciliation fails. Match the five deterministic stage IDs with `IN`, or escape LIKE metacharacters explicitly.

13. **Several read-model failures are silently converted to empty data.**

   Pending and stalled queue reads use `unwrap_or_default` at [activity.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity.rs:169); domain rows and queued transcripts do likewise at lines 283–323 and 405–409. A storage/schema error can therefore look like the quiet empty state instead of activating the controller’s last-known-good error path.

   Fix: return `StorageResult` from these composition helpers and propagate errors with `?`. Test that a forced query failure returns an IPC error while retaining the previous frontend view.

## Medium severity

14. **Parent-task evidence promised by the experience contract is absent.**

   `ActivityItem` has no member-list field at [activity.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity.rs:39), and expanded detail renders only error and attempt at [ActivityPanel.tsx](/home/wojtas/projects/brawler/src/shared/components/activity/ActivityPanel.tsx:132). The panel also never renders `progress.failed`, and backend parent progress excludes `historyFetch` at [activity.rs](/home/wojtas/projects/brawler/src-tauri/src/commands/activity.rs:103). Pending queue attempts are hardcoded to zero at line 209 because `PendingJobRow` does not select `attempts`.

   Fix: add bounded member details, render failed/member counters, project live backfill progress, and select the queue’s real attempt count.

15. **The new UI violates the canonical language and primitive contracts in several places.**

   - Destination actions use verb-prefixed labels at [ActivityPanel.tsx](/home/wojtas/projects/brawler/src/shared/components/activity/ActivityPanel.tsx:46), contrary to noun-only destinations in [ADR 0104](/home/wojtas/projects/brawler/docs/adr/0104-frontend-v2-design-language.md:58).
   - Retry is incorrectly classified as `kind="control"` at [ActivityPanel.tsx](/home/wojtas/projects/brawler/src/shared/components/activity/ActivityPanel.tsx:265), contrary to [ADR 0104](/home/wojtas/projects/brawler/docs/adr/0104-frontend-v2-design-language.md:66).
   - Counts concatenate untranslated grammatical fragments instead of `pluralNoun` at lines 279–280.
   - Every document subject is forced into mono at lines 119 and 143–146, including human titles.
   - The indicator hand-rolls a raw standard icon button at [ActivityIndicator.tsx](/home/wojtas/projects/brawler/src/shared/components/activity/ActivityIndicator.tsx:32), despite `Button variant="icon"`.
   - Activity’s Polish dialog still exposes the inherited English `aria-label="Close dialog"` at [Modal.tsx](/home/wojtas/projects/brawler/src/ui/Modal.tsx:131).
   - Backend-composed Polish subjects such as `Poranny przegląd` render untranslated in English at [activity_identity.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/activity_identity.rs:240).

   Fix: use noun destination labels, `verb="refresh"` for retry, proper plural forms, UI-face document titles, the Button primitive, and a localized Modal close label. Remove composed backend prose from `subject`. I found no ordinary family/status label containing “autopilot”.

16. **The 100k-row performance gate is vacuous.**

   [activity_reads.rs](/home/wojtas/projects/brawler/src-tauri/src/storage/activity_reads.rs:510) explains a different inner query, accepts any `USING INDEX` plan—even a full scan of `idx_job_runs_activity_key`—and checks for the non-SQLite phrase `SCAN job_runs USING TEMP B-TREE`. The final `rows.len() <= 40` assertion is guaranteed by `LIMIT`, regardless of scanning all 100k rows.

   Fix: explain the exact production query, require the intended composite/partial index, reject full index/table scans and `USE TEMP B-TREE`, and independently measure candidate rows visited/decoded.

17. **Multiple required “tests that redden” are absent or materially weaker than named.**

   Concrete gaps include:

   - `settle_failure_leaves_queue_and_occurrence_consistent` induces no failure; it tests an ordinary successful transaction at [queue_tests.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue_tests.rs:574).
   - The panic test claims the worker survived but constructs a second worker at [queue_tests.rs](/home/wojtas/projects/brawler/src-tauri/src/jobs/queue_tests.rs:639).
   - No five-stage-plus-retries collapse test exists.
   - Manual refresh tests call one wrapper, not Tauri/MCP, and do not panic.
   - No domain-status override test, summary-collapse test, succeeded-stage/no-successor reconciliation test, or startup-order test exists.
   - The shortcut test only asserts that more than zero shortcuts fired before opening the modal at [shortcuts.test.tsx](/home/wojtas/projects/brawler/src/shared/shortcuts/shortcuts.test.tsx:117), not every registered ID.
   - The “every family/status” contract test asserts four family labels and no exhaustive status set at [ActivityPanel.contract.test.tsx](/home/wojtas/projects/brawler/src/shared/components/activity/ActivityPanel.contract.test.tsx:43).
   - Browser navigation explicitly admits it does not verify document highlighting at [activity.spec.ts](/home/wojtas/projects/brawler/tests/browser/activity.spec.ts:52).
   - The live spec promises every row but follows only the first at [f3d-activity.live.spec.ts](/home/wojtas/projects/brawler/tests/live/f3d-activity.live.spec.ts:74).
   - Fidelity adds only a queued transcript and a shape-only `{}` summary assertion at [fidelity-corpus.json](/home/wojtas/projects/brawler/src/test/scenarios/fidelity-corpus.json:1874), not the promised succeeded occurrence plus running queue row.

   Fix: implement the exact approved red-test matrix and ensure every test forces the fault or behavior its name claims.

18. **The AppStateRoot pin raise is transparent, but not compliant with the approved plan as written.**

   The branch raises the ratchet from 1859 to 1865 at [file-size-baseline.json](/home/wojtas/projects/brawler/file-size-baseline.json:37), exactly matching the net six-line growth in [AppStateRoot.tsx](/home/wojtas/projects/brawler/src/app/AppStateRoot.tsx:476). This is not stealth gate gaming—the commit names it—but the approved plan explicitly said the pin would remain respected and wiring would live outside the root.

   Fix: extract an Activity shell/lifecycle bridge and restore 1859, or obtain an explicit owner-reviewed exception and amend the plan rationale before PR.

Verified positive claims: startup ordering itself is correct; KPI rows are not terminalized by Activity reconciliation; `list_activity` structurally uses one database checkout; summary polling is independent of scheduler-status success; queue handlers generally call unwrapped cores; the palette entry, accessibility spec, `activity-open` `shootRegion` cell, and committed dark/light baselines exist.

**Final verdict: DO-NOT-SHIP.** The first six findings violate the feature’s core promise that work cannot run invisibly or remain falsely running, and several required regression gates currently cannot catch those failures.

Codex session ID: 01a06b71-69c0-7293-868b-410699df3638
Resume in Codex: codex resume 01a06b71-69c0-7293-868b-410699df3638
