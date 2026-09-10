//! Real-data probe for #465 (ADR 0025 amendment): migrating a throwaway copy
//! of the maintainer's database closes every automatic event/signal reminder
//! and leaves the investor's own follow-ups open — the Research review queue
//! starts clean on real data, and a later list call (the lazy claim/question
//! sync) re-creates nothing.
//!
//! **Inert in CI** — skips unless `BRAWLER_REAL_DB_SCRATCH` points at a
//! throwaway copy that MAY be migrated (never the live file, and never the
//! read-only copy the other `real_data_*` probes open — `open_database`
//! migrates on open).

use crate::storage::{open_database, AppState, ResearchReminderListInput};

const AUTOMATIC_KINDS: [&str; 2] = ["event_review", "signal_review"];

#[test]
fn migration_0155_closes_every_automatic_reminder_on_the_owner_copy() {
    let probe = "migration_0155_closes_every_automatic_reminder_on_the_owner_copy";
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB_SCRATCH") else {
        eprintln!(
            "SKIP {probe}: set BRAWLER_REAL_DB_SCRATCH to a THROWAWAY copy of the owner's database"
        );
        return;
    };
    if !std::path::Path::new(&db_path).is_file() {
        eprintln!("SKIP {probe}: no database at {db_path}");
        return;
    }

    let connection = open_database(&db_path).expect("open + migrate the scratch copy");
    let mut statement = connection
        .prepare(
            "SELECT reminder_kind, status, COUNT(*), SUM(dismissed_at IS NOT NULL)
             FROM research_reminders GROUP BY 1, 2 ORDER BY 1, 2",
        )
        .expect("prepare");
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("rows");
    drop(statement);
    for (kind, status, count, dated) in &rows {
        eprintln!("reminders probe: {kind} {status} = {count} (dismissed_at set: {dated})");
        if AUTOMATIC_KINDS.contains(&kind.as_str()) {
            assert_ne!(
                status, "open",
                "{kind} rows must be closed by migration 0155"
            );
            if status == "dismissed" {
                assert_eq!(
                    count, dated,
                    "every dismissed {kind} row carries dismissed_at"
                );
            }
        } else if status == "open" {
            assert!(
                kind == "manual_research" || kind == "question_review" || kind == "claim_follow_up",
                "unexpected open kind {kind}"
            );
        }
    }
    let automatic_open_before: i64 = rows
        .iter()
        .filter(|(kind, status, _, _)| AUTOMATIC_KINDS.contains(&kind.as_str()) && status == "open")
        .map(|(_, _, count, _)| *count)
        .sum();
    assert_eq!(automatic_open_before, 0);

    // The worst scope of the audit (87 open rows) — listing runs the lazy
    // claim/question sync; it must re-create no event reminder, and the queue
    // must hold no automatic row at all.
    let state = AppState::new(connection);
    for scope_id in ["company_gpw_dvl", "company_gpw_xtb", "company_gpw_kgh"] {
        for _ in 0..2 {
            let reminders = state
                .list_research_reminders(ResearchReminderListInput {
                    scope_type: "company".to_owned(),
                    scope_id: scope_id.to_owned(),
                    status: Some("open".to_owned()),
                })
                .expect("list open reminders");
            assert!(
                reminders
                    .iter()
                    .all(|reminder| !AUTOMATIC_KINDS.contains(&reminder.reminder_kind.as_str())),
                "{scope_id}: an automatic reminder is still open after migration + list"
            );
            eprintln!(
                "reminders probe: {scope_id} open queue = {} rows",
                reminders.len()
            );
        }
    }
}
