//! Real-data probe for #465 (ADR 0025 amendment): migrating a throwaway copy
//! of the maintainer's database closes every automatic event/signal reminder
//! (dated, touching only status/dismissed_at/updated_at) and leaves every
//! other row byte-identical — the Research review queue starts clean on real
//! data, and later list calls (the lazy claim/question sync) change nothing.
//!
//! **Inert in CI** — skips unless `BRAWLER_REAL_DB_SCRATCH` points at a
//! throwaway copy that MAY be migrated (never the live file, and never the
//! read-only copy the other `real_data_*` probes open — `open_database`
//! migrates on open). Owner snapshot 2026-09-10: 833 automatic open rows,
//! 2 personal — the assertions below are generic invariants; the counts are
//! printed as evidence.

use crate::storage::{open_database, open_database_readonly, AppState, ResearchReminderListInput};
use std::collections::BTreeMap;

/// Every column of `research_reminders`, in table order (all TEXT).
const COLUMNS: &str = "id, scope_type, scope_id, company_id, reminder_kind, source_type, source_id, \
     title, body, due_at, status, snoozed_until, completed_at, dismissed_at, created_at, updated_at";
const IDX_STATUS: usize = 10;
const IDX_DISMISSED_AT: usize = 13;
const IDX_UPDATED_AT: usize = 15;
/// The only fields migration 0155 may change on an automatic open row.
const DISMISS_FIELDS: [usize; 3] = [IDX_STATUS, IDX_DISMISSED_AT, IDX_UPDATED_AT];

type Row = Vec<Option<String>>;

fn reminder_rows(connection: &rusqlite::Connection) -> BTreeMap<String, Row> {
    let mut statement = connection
        .prepare(&format!("SELECT {COLUMNS} FROM research_reminders"))
        .expect("prepare");
    statement
        .query_map([], |row| {
            let values = (0..16)
                .map(|index| row.get::<_, Option<String>>(index))
                .collect::<Result<Row, _>>()?;
            Ok((values[0].clone().expect("id"), values))
        })
        .expect("query")
        .collect::<Result<BTreeMap<_, _>, _>>()
        .expect("rows")
}

/// Migration 0155's own signatures, evaluated per row; a NULL `source_type`
/// makes the SQL predicate NULL, which is "not automatic".
fn is_automatic(connection: &rusqlite::Connection, id: &str) -> bool {
    connection
        .query_row(
            "SELECT COALESCE(
                 (reminder_kind = 'event_review' AND id GLOB 'reminder_event_*')
                 OR (reminder_kind = 'signal_review' AND source_type = 'company_signal'
                     AND body LIKE 'High-signal disclosure classified as %'),
                 0)
             FROM research_reminders WHERE id = ?1",
            [id],
            |row| row.get::<_, bool>(0),
        )
        .expect("signature")
}

fn except_dismiss_fields(row: &Row) -> Row {
    row.iter()
        .enumerate()
        .map(|(index, value)| {
            if DISMISS_FIELDS.contains(&index) {
                None
            } else {
                value.clone()
            }
        })
        .collect()
}

#[test]
fn signature_classifier_treats_a_null_source_type_as_personal() {
    // Hermetic: the near-miss row of the migration test (classifier body, no
    // source_type) yields SQL NULL — must read as "not automatic", never panic.
    let connection = crate::storage::open_in_memory_database().expect("db");
    connection
        .execute(
            "INSERT INTO research_reminders (id, scope_type, scope_id, reminder_kind, source_type, title, body)
             VALUES ('r_null', 'company', 'c', 'signal_review', NULL, 'Profit warning',
                     'High-signal disclosure classified as profit warning.'),
                    ('reminder_event_1', 'company', 'c', 'event_review', 'company_event', 'Event', '')",
            [],
        )
        .expect("seed");
    assert!(!is_automatic(&connection, "r_null"));
    assert!(is_automatic(&connection, "reminder_event_1"));
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

    // Snapshot BEFORE the migration (a read-only open never migrates).
    let readonly = open_database_readonly(&db_path).expect("open read-only");
    let before = reminder_rows(&readonly);
    let automatic_ids: Vec<String> = before
        .keys()
        .filter(|id| is_automatic(&readonly, id))
        .cloned()
        .collect();
    drop(readonly);
    let is_open = |row: &Row| row[IDX_STATUS].as_deref() == Some("open");
    let automatic_open_before = automatic_ids
        .iter()
        .filter(|id| is_open(&before[*id]))
        .count();
    let personal_before: BTreeMap<&String, &Row> = before
        .iter()
        .filter(|(id, _)| !automatic_ids.contains(id))
        .collect();
    let personal_open_before = personal_before.values().filter(|row| is_open(row)).count();
    assert!(
        automatic_open_before > 0,
        "the copy carries no open automatic reminder — nothing to prove"
    );

    // Migrate (open_database applies 0155) and compare every column.
    let connection = open_database(&db_path).expect("open + migrate the scratch copy");
    let after = reminder_rows(&connection);
    assert_eq!(
        after.len(),
        before.len(),
        "the migration neither adds nor deletes rows"
    );
    for id in &automatic_ids {
        let (was, now) = (&before[id], &after[id]);
        if is_open(was) {
            assert_eq!(
                now[IDX_STATUS].as_deref(),
                Some("dismissed"),
                "{id}: closes as dismissed"
            );
            assert!(now[IDX_DISMISSED_AT].is_some(), "{id}: dismissal is dated");
            assert_ne!(
                now[IDX_UPDATED_AT], was[IDX_UPDATED_AT],
                "{id}: updated_at moves"
            );
            assert_eq!(
                except_dismiss_fields(now),
                except_dismiss_fields(was),
                "{id}: only status/dismissed_at/updated_at may change"
            );
        } else {
            assert_eq!(
                now, was,
                "{id}: an already-closed automatic row is untouched"
            );
        }
    }
    for (id, row) in &personal_before {
        assert_eq!(
            &&after[*id], row,
            "{id}: a personal follow-up is byte-identical"
        );
    }
    let open_after = after.values().filter(|row| is_open(row)).count();
    let dismissed_after = after
        .values()
        .filter(|row| row[IDX_STATUS].as_deref() == Some("dismissed"))
        .count();
    assert_eq!(
        open_after, personal_open_before,
        "only personal rows stay open"
    );
    eprintln!(
        "reminders probe: {} automatic rows ({automatic_open_before} were open) → dismissed {dismissed_after}, open {open_after} (personal {})",
        automatic_ids.len(),
        personal_before.len()
    );

    // Listing runs the lazy claim/question sync — twice per scope, on the three
    // worst scopes of the audit: the open queue holds exactly the personal
    // open rows of that scope, and the table is unchanged afterwards.
    let state = AppState::new(connection);
    for scope_id in ["company_gpw_dvl", "company_gpw_xtb", "company_gpw_kgh"] {
        let expected_open = personal_before
            .values()
            .filter(|row| is_open(row) && row[2].as_deref() == Some(scope_id))
            .count();
        for _ in 0..2 {
            let open_queue = state
                .list_research_reminders(ResearchReminderListInput {
                    scope_type: "company".to_owned(),
                    scope_id: scope_id.to_owned(),
                    status: Some("open".to_owned()),
                })
                .expect("list open reminders");
            assert_eq!(
                open_queue.len(),
                expected_open,
                "{scope_id}: the open queue must hold exactly the personal open rows"
            );
            assert!(
                open_queue
                    .iter()
                    .all(|reminder| !automatic_ids.contains(&reminder.id)),
                "{scope_id}: an automatic reminder is in the open queue"
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
