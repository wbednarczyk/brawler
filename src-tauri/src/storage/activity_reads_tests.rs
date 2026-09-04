//! `activity_reads.rs`'s own unit tests — split out to stay under the
//! file-size ratchet (ADR 0103) once the sol diff R4 #2 member-verification
//! logic pushed the parent file over the threshold.

use super::*;
use crate::storage::open_in_memory_database;

fn seed_occurrence(connection: &Connection, activity_key: &str, finished_at: &str) {
    connection
        .execute(
            "INSERT INTO job_runs
                (activity_key, run_key, kind, family, subject, target_json, status,
                 attempt, started_at, finished_at)
             VALUES (?1, ?1, 'k', 'sourceRefresh', 's', '{\"kind\":\"sources\"}',
                 'succeeded', 1, ?2, ?2)",
            rusqlite::params![activity_key, finished_at],
        )
        .expect("seed occurrence");
}

/// sol diff R1 #10: the window compare used to be a lexical text
/// comparison, mixing the stored `T…Z` format against `datetime()`'s
/// space-separated one. Since the 'T' separator sorts after a plain
/// space, any row sharing the cutoff's calendar date passed regardless
/// of its actual time-of-day — up to nearly a day too old. Exercise the
/// exact boundary now that both sides share one format.
#[test]
fn window_boundary_is_exact_not_a_lexical_false_positive() {
    let connection = open_in_memory_database().expect("db");
    let now = "2026-09-04T12:00:00.000Z";

    // The historical bug's exact false positive: same calendar date as
    // the cutoff (2026-08-28), but 6 hours EARLIER in the day — genuinely
    // 7 days + 6 hours old. The old lexical compare wrongly admitted it.
    seed_occurrence(
        &connection,
        "past-cutoff-same-date",
        "2026-08-28T06:00:00.000Z",
    );
    // Exactly at the cutoff instant: inclusive `>=`, must be admitted.
    seed_occurrence(&connection, "exactly-7-days", "2026-08-28T12:00:00.000Z");
    // One second older than the cutoff: just outside the window.
    seed_occurrence(&connection, "7-days-plus-1s", "2026-08-28T11:59:59.000Z");
    // One millisecond younger than the cutoff: inside, exercising
    // fractional-second precision.
    seed_occurrence(&connection, "7-days-minus-1ms", "2026-08-28T12:00:00.001Z");
    // Comfortably inside the window.
    seed_occurrence(&connection, "well-inside", "2026-09-01T00:00:00.000Z");

    let rows = recent_occurrences(&connection, now, 7, 40).expect("recent");
    let keys: std::collections::BTreeSet<String> =
        rows.into_iter().map(|row| row.activity_key).collect();
    assert_eq!(
        keys,
        std::collections::BTreeSet::from([
            "exactly-7-days".to_owned(),
            "7-days-minus-1ms".to_owned(),
            "well-inside".to_owned(),
        ]),
        "only rows at/after the exact 7-day cutoff instant pass — never a same-date lexical false positive"
    );
}

/// ADR 0109 dec. 2/D3 volume gate: `recent_occurrences` reads a BOUNDED
/// set at 100k finished rows — asserted via the query plan (it must use an
/// index, never a full table scan), never wall-clock (ADR 0049).
#[test]
fn list_activity_reads_bounded_rows_at_100k() {
    let connection = open_in_memory_database().expect("db");
    let tx =
        rusqlite::Transaction::new_unchecked(&connection, rusqlite::TransactionBehavior::Immediate)
            .expect("tx");
    for i in 0..100_000 {
        tx.execute(
            "INSERT INTO job_runs
                (activity_key, run_key, kind, family, subject, target_json, status,
                 attempt, started_at, finished_at)
             VALUES (?1, ?1, 'k', 'sourceRefresh', 's', '{\"kind\":\"sources\"}',
                 'succeeded', 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            [format!("k:{i}")],
        )
        .expect("seed row");
    }
    tx.commit().expect("commit");

    // sol diff R1 #16: EXPLAIN the EXACT production candidate-selection
    // query (`recent_candidates_sql()`, the same string
    // `recent_occurrences` runs) — never a hand-copied stand-in that can
    // drift from what actually executes.
    let sql = recent_candidates_sql();
    let mut statement = connection
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .expect("prepare plan");
    let plan: Vec<String> = statement
        .query_map(
            params!["2026-09-03T00:00:00Z", "-7 days", RECENT_FETCH_CAP],
            |row| row.get::<_, String>(3),
        )
        .expect("plan rows")
        .map(|row| row.expect("plan row"))
        .collect();
    let plan_text = plan.join(" | ");
    assert!(
        plan_text.contains(&format!("USING INDEX {RECENT_INDEX}"))
            || plan_text.contains(&format!("USING COVERING INDEX {RECENT_INDEX}")),
        "the candidate scan must use the intended index {RECENT_INDEX} — not `idx_job_runs_status` \
         (selective-looking on the status IN-list, but touches almost the whole table at scale \
         since most terminal rows share one of three statuses) and not a temp-b-tree GROUP BY: {plan_text}"
    );
    assert!(
        !plan_text.contains("SCAN job_runs") && !plan_text.contains("TEMP B-TREE"),
        "no full table/index scan of job_runs, and no temp-b-tree materialization, at 100k rows: {plan_text}"
    );

    // Rows VISITED/DECODED must stay bounded by the index range scan —
    // never proportional to the 100k table (sol diff R1 #16: the old
    // `rows.len() <= 40` assertion was guaranteed by `LIMIT` alone and
    // proved nothing about how much work got done to produce it).
    // `sqlite3_stmt_status`'s fullscan-step counter on the prepared
    // statement gives a real measurement, independent of wall clock.
    let mut statement = connection.prepare(&sql).expect("prepare recent");
    let decoded = {
        let rows = statement
            .query_map(
                params!["2026-09-03T00:00:00Z", "-7 days", RECENT_FETCH_CAP],
                map_occurrence_row,
            )
            .expect("query recent");
        let mut decoded = 0;
        for row in rows {
            row.expect("row").expect("decodes");
            decoded += 1;
        }
        decoded
    };
    assert!(
        decoded as i64 <= RECENT_FETCH_CAP,
        "rows actually decoded must stay bounded by the fetch cap, not the 100k table: {decoded}"
    );
    let fullscan_steps = statement.get_status(rusqlite::StatementStatus::FullscanStep);
    assert!(
        fullscan_steps < 1_000,
        "the index range scan must touch a small fraction of the 100k rows, never approach a \
         full-table walk: {fullscan_steps} fullscan steps"
    );

    let recent = recent_occurrences(&connection, "2026-09-03T00:00:00Z", 7, 40).expect("recent");
    assert!(
        recent.len() <= 40,
        "the cap is applied AFTER the collapse, never bypassed"
    );
}
