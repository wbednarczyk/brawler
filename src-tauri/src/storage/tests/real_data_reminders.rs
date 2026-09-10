//! Real-data probe for #465 (ADR 0025 amendment): migrating a throwaway copy
//! of the maintainer's database closes every automatic event/signal reminder
//! (dated) and leaves the investor's own follow-ups untouched — the Research
//! review queue starts clean on real data, and later list calls (the lazy
//! claim/question sync) create nothing.
//!
//! **Inert in CI** — skips unless `BRAWLER_REAL_DB_SCRATCH` points at a
//! throwaway copy that MAY be migrated (never the live file, and never the
//! read-only copy the other `real_data_*` probes open — `open_database`
//! migrates on open).

use crate::storage::{open_database, open_database_readonly, AppState, ResearchReminderListInput};
use std::collections::BTreeMap;

/// `id → (kind, status, dismissed_at)` for every reminder row.
fn reminder_rows(
    connection: &rusqlite::Connection,
) -> BTreeMap<String, (String, String, Option<String>)> {
    let mut statement = connection
        .prepare("SELECT id, reminder_kind, status, dismissed_at FROM research_reminders")
        .expect("prepare");
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                (
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ),
            ))
        })
        .expect("query")
        .collect::<Result<BTreeMap<_, _>, _>>()
        .expect("rows");
    rows
}

fn is_automatic(connection: &rusqlite::Connection, id: &str) -> bool {
    // The migration's own signatures (0155), evaluated per row.
    connection
        .query_row(
            "SELECT (reminder_kind = 'event_review' AND id GLOB 'reminder_event_*')
                 OR (reminder_kind = 'signal_review' AND source_type = 'company_signal'
                     AND body LIKE 'High-signal disclosure classified as %')
             FROM research_reminders WHERE id = ?1",
            [id],
            |row| row.get::<_, bool>(0),
        )
        .expect("signature")
}

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

    // Snapshot BEFORE the migration (read-only open never migrates).
    let readonly = open_database_readonly(&db_path).expect("open read-only");
    let before = reminder_rows(&readonly);
    let automatic_ids: Vec<String> = before
        .keys()
        .filter(|id| is_automatic(&readonly, id))
        .cloned()
        .collect();
    drop(readonly);
    let automatic_open_before = automatic_ids
        .iter()
        .filter(|id| before[*id].1 == "open")
        .count();
    let personal_before: BTreeMap<_, _> = before
        .iter()
        .filter(|(id, _)| !automatic_ids.contains(id))
        .map(|(id, row)| (id.clone(), row.clone()))
        .collect();
    assert!(
        automatic_open_before > 0,
        "the copy carries no open automatic reminder — nothing to prove (owner snapshot 2026-09-10: 833)"
    );

    // Migrate (open_database applies 0155) and compare.
    let connection = open_database(&db_path).expect("open + migrate the scratch copy");
    let after = reminder_rows(&connection);
    assert_eq!(
        after.len(),
        before.len(),
        "the migration neither adds nor deletes rows"
    );
    for id in &automatic_ids {
        let (kind, status, dismissed_at) = &after[id];
        assert!(
            status != "open",
            "{id} ({kind}) is still open after migration 0155"
        );
        if before[id].1 == "open" {
            assert_eq!(
                status, "dismissed",
                "{id}: an open automatic row closes as dismissed"
            );
            assert!(dismissed_at.is_some(), "{id}: dismissal is dated");
        } else {
            assert_eq!(
                before[id], after[id],
                "{id}: an already-closed row is untouched"
            );
        }
    }
    for (id, row) in &personal_before {
        assert_eq!(
            &after[id], row,
            "{id}: a personal follow-up must be untouched"
        );
    }
    let dismissed_after = after
        .values()
        .filter(|(_, status, _)| status == "dismissed")
        .count();
    let open_after = after
        .values()
        .filter(|(_, status, _)| status == "open")
        .count();
    eprintln!(
        "reminders probe: {} automatic rows ({automatic_open_before} were open) → dismissed {dismissed_after}, open {open_after} (personal {})",
        automatic_ids.len(),
        personal_before.len()
    );

    // Listing runs the lazy claim/question sync — twice per scope, on the three
    // worst scopes of the audit: the row set must not change and the open
    // queue must hold no automatic row.
    let state = AppState::new(connection);
    for scope_id in ["company_gpw_dvl", "company_gpw_xtb", "company_gpw_kgh"] {
        for _ in 0..2 {
            let open_queue = state
                .list_research_reminders(ResearchReminderListInput {
                    scope_type: "company".to_owned(),
                    scope_id: scope_id.to_owned(),
                    status: Some("open".to_owned()),
                })
                .expect("list open reminders");
            assert!(
                open_queue
                    .iter()
                    .all(|reminder| !automatic_ids.contains(&reminder.id)),
                "{scope_id}: an automatic reminder is in the open queue after migration"
            );
            assert!(
                open_queue
                    .iter()
                    .all(|reminder| reminder.reminder_kind != "event_review"
                        && reminder.reminder_kind != "signal_review"),
                "{scope_id}: an event/signal reminder is in the open queue"
            );
            eprintln!(
                "reminders probe: {scope_id} open queue = {} rows",
                open_queue.len()
            );
        }
    }
    let guard = state.checkout_for_tests().expect("checkout");
    let final_rows = reminder_rows(&guard);
    assert_eq!(
        final_rows, after,
        "list calls must create, delete or change no reminder row"
    );
}
