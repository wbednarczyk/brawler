use super::*;
use crate::storage::migrations::{apply_migrations_up_to, count_applied_migrations, count_rows};

#[test]
fn rerunning_migrations_is_idempotent_and_preserves_data() {
    let mut connection = open_in_memory_database().expect("database should initialize");
    let expected = expected_migration_count();

    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed a company");

    // Re-running the runner must be a safe no-op: no error, no duplicated
    // migration rows, and existing tables + data survive. This is the v0.40.0
    // "no such table" / silent-skip class encoded as a guard.
    apply_migrations(&mut connection).expect("re-running migrations should be safe");

    assert_eq!(
        count_applied_migrations(&connection).expect("count applied"),
        expected,
        "re-run must not add or drop migration rows",
    );
    assert_eq!(
        count_rows(&connection, "companies").expect("count companies"),
        1,
        "re-running migrations must not wipe existing data",
    );
}

#[test]
fn older_database_upgrades_to_latest_without_losing_data() {
    // Corpus-style upgrade path: a historical database stuck at the initial
    // schema, with data, must upgrade cleanly to the latest schema with the
    // pre-existing data intact — without needing a binary .sqlite snapshot.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 1).expect("apply initial migration");
    assert_eq!(
        count_applied_migrations(&connection).expect("count applied"),
        1,
        "database should be pinned to the initial schema",
    );

    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('legacy', 'GPW', 'PKN', 'GPW:PKN', 'ORLEN S.A.')",
            [],
        )
        .expect("seed legacy data on the old schema");

    apply_migrations(&mut connection).expect("upgrade to the latest schema");

    assert_eq!(
        count_applied_migrations(&connection).expect("count applied"),
        expected_migration_count(),
        "upgrade should reach the latest migration",
    );
    let survived: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM companies WHERE id = 'legacy'",
            [],
            |row| row.get(0),
        )
        .expect("query legacy company");
    assert_eq!(survived, 1, "legacy data must survive the upgrade");
}

#[test]
fn upgrades_committed_v1_snapshot_to_latest() {
    // A REAL historical-schema snapshot captured at migration v1 (see
    // corpus/legacy_v1.sqlite, generated with sqlite3 from 0001_initial.sql +
    // a seeded company). Upgrading it with the full runner must reach the
    // latest schema with the pre-existing data intact — the strongest guard for
    // the "edited/incompatible migration breaks real old data" class (ADR 0048).
    const SNAPSHOT: &[u8] = include_bytes!("corpus/legacy_v1.sqlite");
    let path =
        std::env::temp_dir().join(format!("brawler_corpus_v1_{}.sqlite", std::process::id()));
    std::fs::write(&path, SNAPSHOT).expect("materialize the snapshot to a temp file");

    let connection = open_database(&path).expect("open + upgrade the v1 snapshot");

    assert_eq!(
        count_applied_migrations(&connection).expect("count applied"),
        expected_migration_count(),
        "the committed v1 snapshot should upgrade to the latest migration",
    );
    let legacy: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM companies WHERE id = 'legacy_corpus'",
            [],
            |row| row.get(0),
        )
        .expect("query the seeded legacy company");
    assert_eq!(
        legacy, 1,
        "the snapshot's pre-existing data must survive the upgrade"
    );

    drop(connection);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn migration_0062_widens_autopilot_run_trigger_and_preserves_rows() {
    // ADR 0077 §3 amendment (a): migration 0062 rebuilds `autopilot_run` to
    // widen its `trigger` CHECK to include 'history_sweep'. A pre-0062 run must
    // survive the rebuild with every column intact, a 'history_sweep' insert must
    // then succeed, and a bogus trigger must still be rejected by the CHECK.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 61).expect("apply schema through 0061");

    // Seed a company + report document + a detection run on the pre-0062 schema
    // (trigger CHECK still only allows 'detection' | 'manual').
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed a company");
    connection
        .execute(
            "INSERT INTO report_documents (id, company_id, source_type, url, fetch_status)
             VALUES ('doc1', 'c1', 'user_url', 'https://x/ssf-2025.pdf', 'fetched'),
                    ('doc2', 'c1', 'user_url', 'https://x/ssf-2024.pdf', 'fetched'),
                    ('doc3', 'c1', 'user_url', 'https://x/ssf-2023.pdf', 'fetched')",
            [],
        )
        .expect("seed report documents");
    connection
        .execute(
            "INSERT INTO autopilot_run
                (id, company_id, report_document_id, trigger, mode, status, stage,
                 produced_fact_ids_json)
             VALUES ('run1', 'c1', 'doc1', 'detection', 'autopilot', 'succeeded',
                 'notify', '[\"fact_a\"]')",
            [],
        )
        .expect("seed a pre-0062 detection run");

    apply_migrations(&mut connection).expect("apply migration 0062");

    // The pre-existing run survives with all columns intact.
    let (trigger, mode, status, stage, produced): (String, String, String, String, String) =
        connection
            .query_row(
                "SELECT trigger, mode, status, stage, produced_fact_ids_json
                 FROM autopilot_run WHERE id = 'run1'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("the pre-0062 run must survive the rebuild");
    assert_eq!(trigger, "detection");
    assert_eq!(mode, "autopilot");
    assert_eq!(status, "succeeded");
    assert_eq!(stage, "notify");
    assert_eq!(produced, "[\"fact_a\"]");

    // A 'history_sweep' insert now succeeds against the widened CHECK.
    connection
        .execute(
            "INSERT INTO autopilot_run
                (id, company_id, report_document_id, trigger, mode)
             VALUES ('run2', 'c1', 'doc2', 'history_sweep', 'assist')",
            [],
        )
        .expect("a history_sweep-triggered run must be storable after 0062");

    // A bogus trigger is still rejected by the CHECK.
    let bogus = connection.execute(
        "INSERT INTO autopilot_run
            (id, company_id, report_document_id, trigger, mode)
         VALUES ('run3', 'c1', 'doc3', 'nonsense', 'assist')",
        [],
    );
    assert!(
        bogus.is_err(),
        "a trigger outside the CHECK set must still be rejected"
    );

    // The history_sweeps table is created and enforces its own trigger CHECK.
    connection
        .execute(
            "INSERT INTO history_sweeps (id, company_id, trigger)
             VALUES ('sweep1', 'c1', 'backfill')",
            [],
        )
        .expect("a backfill sweep row must be storable");
    let sweep_bogus = connection.execute(
        "INSERT INTO history_sweeps (id, company_id, trigger)
         VALUES ('sweep2', 'c1', 'detection')",
        [],
    );
    assert!(
        sweep_bogus.is_err(),
        "history_sweeps.trigger must reject a value outside ('backfill','manual')"
    );
}

#[test]
fn migration_0082_creates_ownership_tables_and_seeds_dictionary_idempotently() {
    // ADR 0072 / plan v0.56 T2: migration 0082 adds `ownership_stakes` (append-only
    // stake snapshots) + `ownership_holder_dictionary` (seeded as data). Both tables
    // use IF NOT EXISTS and the dictionary seed is an idempotent upsert, so re-running
    // the runner is a safe no-op that neither errors nor re-seeds duplicates.
    let mut connection = open_in_memory_database().expect("database should initialize");

    let seeded_before: i64 = count_rows(&connection, "ownership_holder_dictionary").expect("count");
    assert!(seeded_before > 0, "the holder dictionary must be seeded");

    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("company insert");
    connection
        .execute(
            "INSERT INTO ownership_stakes
                (id, company_id, holder_name_raw, holder_name_normalized, capital_pct, votes_pct, as_of, source)
             VALUES ('s1', 'c1', 'Marcin Iwiński', 'MARCIN IWIŃSKI', '10', '10', '2025-06-30', 'report_document')",
            [],
        )
        .expect("an ownership stake row must be storable");

    // Re-running the runner is a safe no-op on the new tables and seed.
    apply_migrations(&mut connection).expect("re-running migrations should be safe");

    let seeded_after: i64 = count_rows(&connection, "ownership_holder_dictionary").expect("count");
    assert_eq!(
        seeded_before, seeded_after,
        "dictionary re-seed must not duplicate"
    );
    assert_eq!(
        count_rows(&connection, "ownership_stakes").expect("count"),
        1,
        "re-running migrations must not disturb ownership data"
    );
}

#[test]
fn migration_0083_creates_ownership_residual_table_idempotently() {
    // ADR 0072 / plan v0.56 T3: migration 0083 adds `ownership_extraction_residual`
    // (the pending queue for periodic reports the deterministic parser could not
    // turn into stakes). IF NOT EXISTS + an idempotent per-document upsert, so
    // re-running the runner is a safe no-op that neither errors nor duplicates.
    let mut connection = open_in_memory_database().expect("database should initialize");

    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("company insert");
    connection
        .execute(
            "INSERT INTO report_documents (id, company_id, source_type, url, fetch_status)
             VALUES ('d1', 'c1', 'user_url', 'https://x/ssf.xhtml', 'fetched')",
            [],
        )
        .expect("report document insert");
    connection
        .execute(
            "INSERT INTO ownership_extraction_residual
                (report_document_id, company_id, parse_state, detected_as_of)
             VALUES ('d1', 'c1', 'glyph_encoded', '2025-12-31')",
            [],
        )
        .expect("an extraction residual row must be storable");

    // Re-running the runner is a safe no-op on the new table.
    apply_migrations(&mut connection).expect("re-running migrations should be safe");

    assert_eq!(
        count_rows(&connection, "ownership_extraction_residual").expect("count"),
        1,
        "re-running migrations must not disturb residual data"
    );

    // The parse_state CHECK rejects an out-of-taxonomy value.
    let bad = connection.execute(
        "INSERT INTO ownership_extraction_residual (report_document_id, company_id, parse_state)
         VALUES ('d2', 'c1', 'nonsense')",
        [],
    );
    assert!(bad.is_err(), "parse_state CHECK must reject unknown states");
}

#[test]
fn migration_0084_creates_holder_type_proposals_table_idempotently() {
    // ADR 0072 §3 / plan v0.56 T5: migration 0084 adds
    // `ownership_holder_type_proposals` (AI classify-with-confirm). IF NOT EXISTS +
    // a deterministic id UNIQUE per (company, holder), so re-running the runner is a
    // safe no-op that neither errors nor disturbs data.
    let mut connection = open_in_memory_database().expect("database should initialize");

    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("company insert");
    connection
        .execute(
            "INSERT INTO ownership_holder_type_proposals
                (id, company_id, holder_name_normalized, proposed_type, confidence, status)
             VALUES ('p1', 'c1', 'ULTRO S.A.R.L.', 'other_institutional', 0.7, 'pending')",
            [],
        )
        .expect("a holder-type proposal row must be storable");

    // Re-running the runner is a safe no-op on the new table.
    apply_migrations(&mut connection).expect("re-running migrations should be safe");
    assert_eq!(
        count_rows(&connection, "ownership_holder_type_proposals").expect("count"),
        1,
        "re-running migrations must not disturb proposal data"
    );

    // The proposed_type CHECK rejects an out-of-taxonomy value.
    let bad_type = connection.execute(
        "INSERT INTO ownership_holder_type_proposals (id, company_id, holder_name_normalized, proposed_type)
         VALUES ('p2', 'c1', 'X', 'nonsense')",
        [],
    );
    assert!(
        bad_type.is_err(),
        "proposed_type CHECK must reject unknown types"
    );

    // The status CHECK rejects an out-of-taxonomy value.
    let bad_status = connection.execute(
        "INSERT INTO ownership_holder_type_proposals (id, company_id, holder_name_normalized, proposed_type, status)
         VALUES ('p3', 'c1', 'Y', 'tfi', 'nonsense')",
        [],
    );
    assert!(
        bad_status.is_err(),
        "status CHECK must reject unknown states"
    );
}

#[test]
fn migration_0063_creates_ocr_profile_table_idempotently() {
    // ADR 0077 §4: migration 0063 adds the per-company OCR-markdown extraction
    // profile table. It must be idempotent (CREATE TABLE IF NOT EXISTS) and
    // accept an insert round-trip.
    let mut connection = open_in_memory_database().expect("database should initialize");

    // Re-running the runner is a safe no-op on the new table.
    apply_migrations(&mut connection).expect("re-running migrations should be safe");

    connection
        .execute(
            "INSERT INTO company_ocr_extraction_profile
                (company_id, template_hash, scale, profile_json, version)
             VALUES ('GPW:LPP', 'abc123', 'Millions', '{\"company_id\":\"GPW:LPP\"}', 1)",
            [],
        )
        .expect("an OCR profile row must be storable");

    let (scale, version): (String, i64) = connection
        .query_row(
            "SELECT scale, version FROM company_ocr_extraction_profile WHERE company_id = 'GPW:LPP'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("the inserted OCR profile row must read back");
    assert_eq!(scale, "Millions");
    assert_eq!(version, 1);
}

#[test]
fn migration_0065_adds_sweep_budget_columns_with_defaults_on_old_rows() {
    // ADR 0077 §6: migration 0065 adds `history_sweeps.ai_calls_used` /
    // `ai_call_limit` and `autopilot_run.sweep_id` as plain ADD COLUMNs (no table
    // rebuild). A row written on the pre-0065 schema must read the defaults for
    // the new columns, the new columns must be writable, and the runner must be
    // idempotent by version.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 64).expect("apply schema through 0064");

    // Seed a company + report doc + a pre-0065 sweep and autopilot_run — neither
    // carries the columns 0065 introduces.
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed a company");
    connection
        .execute(
            "INSERT INTO report_documents (id, company_id, source_type, url, fetch_status)
             VALUES ('doc1', 'c1', 'user_url', 'https://x/ssf-2025.pdf', 'fetched')",
            [],
        )
        .expect("seed a report document");
    connection
        .execute(
            "INSERT INTO history_sweeps (id, company_id, trigger) VALUES ('sweep1', 'c1', 'manual')",
            [],
        )
        .expect("seed a pre-0065 sweep");
    connection
        .execute(
            "INSERT INTO autopilot_run (id, company_id, report_document_id, trigger, mode)
             VALUES ('run1', 'c1', 'doc1', 'history_sweep', 'assist')",
            [],
        )
        .expect("seed a pre-0065 run");

    apply_migrations(&mut connection).expect("apply migration 0065");

    // Old rows read the defaults for the new columns.
    let (used, limit): (i64, i64) = connection
        .query_row(
            "SELECT ai_calls_used, ai_call_limit FROM history_sweeps WHERE id = 'sweep1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("old sweep row must read the new columns");
    assert_eq!(used, 0, "ai_calls_used defaults to 0");
    assert_eq!(limit, 0, "ai_call_limit defaults to 0");
    let sweep_id: Option<String> = connection
        .query_row(
            "SELECT sweep_id FROM autopilot_run WHERE id = 'run1'",
            [],
            |row| row.get(0),
        )
        .expect("old run row must read the new column");
    assert_eq!(sweep_id, None, "sweep_id defaults to NULL on a legacy run");

    // The new columns are writable.
    connection
        .execute(
            "UPDATE history_sweeps SET ai_call_limit = 5, ai_calls_used = 2 WHERE id = 'sweep1'",
            [],
        )
        .expect("the new sweep-budget columns must be writable");
    connection
        .execute(
            "UPDATE autopilot_run SET sweep_id = 'sweep1' WHERE id = 'run1'",
            [],
        )
        .expect("sweep_id must be writable");
    let (used, limit): (i64, i64) = connection
        .query_row(
            "SELECT ai_calls_used, ai_call_limit FROM history_sweeps WHERE id = 'sweep1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read back");
    assert_eq!((used, limit), (2, 5));

    // Re-running the runner is a safe no-op (idempotent by version).
    let before = count_applied_migrations(&connection).expect("count");
    apply_migrations(&mut connection).expect("re-running migrations should be safe");
    let after = count_applied_migrations(&connection).expect("count");
    assert_eq!(before, after, "0065 must not re-apply");
}

/// Seed a company, a KPI definition alias is already seeded by 0034 (canonical
/// pack). Insert a period of `period_type` for `fiscal_year` and return its id.
fn seed_period(connection: &Connection, id: &str, company: &str, year: i64, period_type: &str) {
    connection
        .execute(
            "INSERT INTO financial_periods (id, company_id, fiscal_year, period_type)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, company, year, period_type],
        )
        .expect("seed a financial period");
}

fn seed_fact(
    connection: &Connection,
    id: &str,
    company: &str,
    period: &str,
    def: &str,
    value: &str,
) {
    connection
        .execute(
            "INSERT INTO financial_facts (id, company_id, period_id, definition_id, value_numeric)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, company, period, def, value],
        )
        .expect("seed a financial fact");
}

/// T-A6 (card f64cea2): migration 0066 merges a legacy out-of-spec
/// `period_type='annual'` row into its canonical `(company, fiscal_year, FY)`
/// sibling — repointing facts (dropping the annual-side duplicate on a slot
/// collision so the canonical FY value wins), then deleting the annual row.
#[test]
fn migration_0066_merges_annual_into_fy_sibling_and_repoints_facts() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("enable FKs");
    apply_migrations_up_to(&mut connection, 65).expect("apply schema through 0065");

    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'ABE', 'GPW:ABE', 'AB S.A.')",
            [],
        )
        .expect("seed a company");

    // Both an out-of-spec 'annual' row and its canonical 'FY' sibling for 2024.
    seed_period(&connection, "p_annual", "c1", 2024, "annual");
    seed_period(&connection, "p_fy", "c1", 2024, "FY");

    // A slot-colliding fact on both periods (same definition/default slot): the
    // canonical FY value must win, the annual duplicate must be dropped.
    seed_fact(
        &connection,
        "f_fy_np",
        "c1",
        "p_fy",
        "kpidef_net_profit",
        "100",
    );
    seed_fact(
        &connection,
        "f_an_np",
        "c1",
        "p_annual",
        "kpidef_net_profit",
        "999",
    );
    // A fact only on the annual row: it must be repointed onto FY.
    seed_fact(
        &connection,
        "f_an_ta",
        "c1",
        "p_annual",
        "kpidef_total_assets",
        "500",
    );

    // A report document + management claim pointing at the annual period must be
    // repointed onto FY, not nulled out.
    connection
        .execute(
            "INSERT INTO report_documents (id, company_id, period_id, source_type, url, fetch_status)
             VALUES ('doc1', 'c1', 'p_annual', 'user_url', 'https://x/y.pdf', 'fetched')",
            [],
        )
        .expect("seed a report document on the annual period");

    apply_migrations(&mut connection).expect("apply migration 0066");

    // Exactly one period remains for (c1, 2024) and it is FY.
    let periods: Vec<(String, String)> = {
        let mut stmt = connection
            .prepare("SELECT id, period_type FROM financial_periods WHERE company_id='c1' AND fiscal_year=2024")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(periods.len(), 1, "annual must be merged away: {periods:?}");
    assert_eq!(periods[0].0, "p_fy");
    assert_eq!(periods[0].1, "FY");

    // Net profit: only the canonical FY value survives (annual duplicate dropped).
    let np: Vec<(String, String)> = {
        let mut stmt = connection
            .prepare("SELECT value_numeric, period_id FROM financial_facts WHERE definition_id='kpidef_net_profit'")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(np, vec![("100".to_owned(), "p_fy".to_owned())]);

    // Total assets: the annual-only fact is repointed onto FY.
    let ta: (String, String) = connection
        .query_row(
            "SELECT value_numeric, period_id FROM financial_facts WHERE definition_id='kpidef_total_assets'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("total-assets fact must survive");
    assert_eq!(ta, ("500".to_owned(), "p_fy".to_owned()));

    // The report document is repointed onto FY, not nulled.
    let doc_period: Option<String> = connection
        .query_row(
            "SELECT period_id FROM report_documents WHERE id='doc1'",
            [],
            |r| r.get(0),
        )
        .expect("doc must exist");
    assert_eq!(doc_period.as_deref(), Some("p_fy"));

    // Idempotent: a second run changes nothing and does not re-apply the row.
    let before = count_applied_migrations(&connection).expect("count");
    apply_migrations(&mut connection).expect("re-run is safe");
    let after = count_applied_migrations(&connection).expect("count");
    assert_eq!(before, after, "0066 must not re-apply");
    let remaining: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM financial_periods WHERE period_type='annual'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(remaining, 0, "no annual rows may remain after a second run");
}

/// ADR 0071 (J1): migration 0067 creates the immutable decision journal. An
/// older database must upgrade cleanly, the immutability triggers must be live
/// (raw UPDATE/DELETE rejected), the kind CHECK must hold, deleting the company
/// must still cascade the journal away, and the runner must stay idempotent.
#[test]
fn migration_0067_decision_entries_upgrade_triggers_and_idempotence() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("enable FKs");
    apply_migrations_up_to(&mut connection, 66).expect("apply schema through 0066");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed a company");

    apply_migrations(&mut connection).expect("upgrade to 0067+");

    connection
        .execute(
            "INSERT INTO decision_entries (id, company_id, kind, rationale_md, decided_at)
             VALUES ('de1', 'c1', 'buy', 'thesis holds', '2026-06-01')",
            [],
        )
        .expect("an entry must be storable");

    // Immutability triggers reject direct mutation.
    assert!(
        connection
            .execute(
                "UPDATE decision_entries SET rationale_md = 'rewrite' WHERE id = 'de1'",
                [],
            )
            .is_err(),
        "UPDATE must be rejected"
    );
    assert!(
        connection
            .execute("DELETE FROM decision_entries WHERE id = 'de1'", [])
            .is_err(),
        "DELETE must be rejected"
    );

    // The kind CHECK rejects values outside the recorded-judgment set.
    assert!(
        connection
            .execute(
                "INSERT INTO decision_entries (id, company_id, kind, rationale_md, decided_at)
                 VALUES ('de2', 'c1', 'moon', 'x', '2026-06-01')",
                [],
            )
            .is_err(),
        "an unknown kind must be rejected by the CHECK"
    );

    // Removing the company still works: the trigger carves out the FK cascade
    // (entries die with their company; only direct deletes are immutable).
    connection
        .execute("DELETE FROM companies WHERE id = 'c1'", [])
        .expect("deleting a company with journal entries must still work");
    let remaining: i64 = connection
        .query_row("SELECT COUNT(*) FROM decision_entries", [], |row| {
            row.get(0)
        })
        .expect("count");
    assert_eq!(remaining, 0, "the cascade must remove the journal entries");

    // Idempotent by version.
    let before = count_applied_migrations(&connection).expect("count");
    apply_migrations(&mut connection).expect("re-run is safe");
    assert_eq!(
        before,
        count_applied_migrations(&connection).expect("count"),
        "0067 must not re-apply"
    );
}

/// ADR 0071 (J1): migration 0068 creates report_expectations + the metrics
/// child. The occurrence UNIQUE must hold, the comparator CHECK must hold, and
/// child rows must CASCADE with their parent. Runner idempotent by version.
#[test]
fn migration_0068_report_expectations_unique_check_and_cascade() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("enable FKs");
    apply_migrations_up_to(&mut connection, 66).expect("apply schema through 0066");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed a company");

    apply_migrations(&mut connection).expect("upgrade to 0068+");

    connection
        .execute(
            "INSERT INTO report_expectations
                (id, company_id, event_key, fiscal_year, period_type, stance_md)
             VALUES ('re1', 'c1', 'evt-1', 2026, 'H1', 'stance')",
            [],
        )
        .expect("an expectation must be storable");
    assert!(
        connection
            .execute(
                "INSERT INTO report_expectations
                    (id, company_id, event_key, fiscal_year, period_type, stance_md)
                 VALUES ('re2', 'c1', 'evt-1', 2026, 'H1', 'duplicate')",
                [],
            )
            .is_err(),
        "UNIQUE (company_id, event_key) must reject a duplicate occurrence"
    );

    connection
        .execute(
            "INSERT INTO report_expectation_metrics
                (id, expectation_id, metric_key, comparator, expected_value)
             VALUES ('rem1', 're1', 'revenue', 'gte', '100')",
            [],
        )
        .expect("a metric row must be storable");
    assert!(
        connection
            .execute(
                "INSERT INTO report_expectation_metrics
                    (id, expectation_id, metric_key, comparator, expected_value)
                 VALUES ('rem2', 're1', 'revenue', 'approx', '100')",
                [],
            )
            .is_err(),
        "a comparator outside ('lt','lte','eq','gte','gt') must be rejected"
    );

    // Child rows cascade with their parent expectation.
    connection
        .execute("DELETE FROM report_expectations WHERE id = 're1'", [])
        .expect("expectations are deletable (only decision entries are immutable)");
    let orphans: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM report_expectation_metrics",
            [],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!(orphans, 0, "metric rows must cascade with the parent");

    // Idempotent by version.
    let before = count_applied_migrations(&connection).expect("count");
    apply_migrations(&mut connection).expect("re-run is safe");
    assert_eq!(
        before,
        count_applied_migrations(&connection).expect("count"),
        "0068 must not re-apply"
    );
}

/// T-A6: a lone out-of-spec 'annual' row with no FY sibling is relabeled to FY
/// in place (no merge, no orphans), and re-running is a no-op.
#[test]
fn migration_0066_relabels_lone_annual_to_fy() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 65).expect("apply schema through 0065");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'ABE', 'GPW:ABE', 'AB S.A.')",
            [],
        )
        .expect("seed a company");
    seed_period(&connection, "p_annual", "c1", 2025, "annual");
    seed_fact(
        &connection,
        "f1",
        "c1",
        "p_annual",
        "kpidef_net_profit",
        "42",
    );

    apply_migrations(&mut connection).expect("apply migration 0066");

    let period_type: String = connection
        .query_row(
            "SELECT period_type FROM financial_periods WHERE company_id='c1' AND fiscal_year=2025",
            [],
            |r| r.get(0),
        )
        .expect("the period must survive");
    assert_eq!(period_type, "FY", "lone annual must be relabeled to FY");
    // The fact stays put (its period was relabeled, not moved).
    let fact_period: String = connection
        .query_row(
            "SELECT period_id FROM financial_facts WHERE id='f1'",
            [],
            |r| r.get(0),
        )
        .expect("fact survives");
    assert_eq!(fact_period, "p_annual");

    apply_migrations(&mut connection).expect("re-run is safe");
    let annuals: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM financial_periods WHERE period_type='annual'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(annuals, 0);
}

#[test]
fn migration_0069_drops_content_embeddings_and_purges_embedding_jobs() {
    // ADR 0080 decision 4: the embedding model is retired; the disposable vector
    // index is dropped and its queued jobs purged. A real pre-removal state —
    // embedding rows, a queued content_embedding job, and the legacy
    // `similarity_strategy='embedding'` setting — must upgrade cleanly, and
    // unrelated queued jobs must survive.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 68).expect("apply schema through 0068");

    connection
        .execute(
            "INSERT INTO content_embeddings
                 (content_type, content_id, model_id, dim, vector, content_hash)
             VALUES ('feed_item', 'feed_01', 'intfloat/multilingual-e5-small', 2,
                     X'0000803F00000000', 'hash-1')",
            [],
        )
        .expect("seed an embedding row on the old schema");
    connection
        .execute(
            "INSERT INTO job_queue (id, kind, status)
             VALUES ('content_embedding', 'content_embedding', 'pending'),
                    ('unrelated-job', 'source_refresh', 'pending')",
            [],
        )
        .expect("seed queued jobs on the old schema");
    connection
        .execute(
            "INSERT INTO settings (key, value, value_type)
             VALUES ('similarity_strategy', 'embedding', 'string')
             ON CONFLICT (key) DO UPDATE SET value = 'embedding'",
            [],
        )
        .expect("seed the legacy embedding strategy");

    apply_migrations(&mut connection).expect("upgrade to the latest schema");

    let table_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master
              WHERE type = 'table' AND name = 'content_embeddings')",
            [],
            |row| row.get(0),
        )
        .expect("table existence check");
    assert!(!table_exists, "content_embeddings must be dropped");

    let embedding_jobs: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM job_queue WHERE kind = 'content_embedding'",
            [],
            |row| row.get(0),
        )
        .expect("count embedding jobs");
    assert_eq!(embedding_jobs, 0, "content_embedding jobs must be purged");

    let unrelated_jobs: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM job_queue WHERE id = 'unrelated-job'",
            [],
            |row| row.get(0),
        )
        .expect("count unrelated jobs");
    assert_eq!(unrelated_jobs, 1, "unrelated queued jobs must survive");

    // Re-run is a safe no-op (self-heal / idempotence on an already-clean DB).
    apply_migrations(&mut connection).expect("re-run is safe");
    assert_eq!(
        count_applied_migrations(&connection).expect("count applied"),
        crate::storage::migrations::expected_migration_count(),
    );
}

#[test]
fn migration_0070_drops_feed_items_story_key_and_index() {
    // ADR 0080 decision 3: the write-only story-key path is removed. A real old
    // database with story_key values must upgrade cleanly: the column and its
    // index go, the feed items themselves survive untouched.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 68).expect("apply schema through 0068");

    let adapter_id: String = connection
        .query_row("SELECT id FROM source_adapters LIMIT 1", [], |row| {
            row.get(0)
        })
        .expect("a seeded source adapter");
    connection
        .execute(
            "INSERT INTO feed_items
                 (id, type, source_adapter_id, source_name, source_url, title,
                  fetched_at, dedupe_key, story_key)
             VALUES ('feed_old', 'Public media', ?1, 'Seed', 'https://example.com/a',
                     'Spolka ABC podpisala znaczaca umowe', '2026-06-08T10:00:00Z',
                     'dk-1', 'story:ABC:GPW:2026-06-08:spolka-abc')",
            [&adapter_id],
        )
        .expect("seed a feed item with a story_key on the old schema");

    apply_migrations(&mut connection).expect("upgrade to the latest schema");

    let has_story_key: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('feed_items')
              WHERE name = 'story_key')",
            [],
            |row| row.get(0),
        )
        .expect("column existence check");
    assert!(!has_story_key, "feed_items.story_key must be dropped");

    let index_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master
              WHERE type = 'index' AND name = 'idx_feed_items_story_key')",
            [],
            |row| row.get(0),
        )
        .expect("index existence check");
    assert!(!index_exists, "idx_feed_items_story_key must be dropped");

    let survived: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM feed_items WHERE id = 'feed_old'",
            [],
            |row| row.get(0),
        )
        .expect("count seeded feed items");
    assert_eq!(survived, 1, "feed items must survive the column drop");

    apply_migrations(&mut connection).expect("re-run is safe");
}

#[test]
fn migration_0088_deletes_aggregator_stakes_only() {
    // Repair migration 0088 (parser-defect reset, 2026-07-16): the old aggregator
    // parser ingested summary/"razem" rows and the sub-5% fund table, so every
    // aggregator basis is garbage. 0088 deletes ALL `aggregator` stakes (the fixed
    // parser rewrites clean bases on the next refresh) and touches nothing else.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 87).expect("apply schema through 0087");

    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed a company");
    connection
        .execute(
            "INSERT INTO ownership_stakes
                (id, company_id, holder_name_raw, holder_name_normalized, source, as_of, capital_pct)
             VALUES
                ('rep1', 'c1', 'Holder A', 'HOLDER A', 'report_document', '2026-06-30', '20'),
                ('esp1', 'c1', 'Holder D', 'HOLDER D', 'espi_filing',     '2026-07-05', '9'),
                ('agg1', 'c1', 'Holder B', 'HOLDER B', 'aggregator',      '2026-07-10', '18'),
                ('agg2', 'c1', 'Holder C', 'HOLDER C', 'aggregator',      '2026-07-10', '6')",
            [],
        )
        .expect("seed stakes across sources");

    apply_migrations(&mut connection).expect("apply migration 0088");

    let aggregator: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM ownership_stakes WHERE source = 'aggregator'",
            [],
            |row| row.get(0),
        )
        .expect("count aggregator");
    assert_eq!(aggregator, 0, "0088 deletes every aggregator stake");

    let others: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM ownership_stakes WHERE source != 'aggregator'",
            [],
            |row| row.get(0),
        )
        .expect("count non-aggregator");
    assert_eq!(
        others, 2,
        "report_document and espi_filing stakes are untouched"
    );

    // Idempotent re-run leaves the non-aggregator rows in place.
    apply_migrations(&mut connection).expect("re-run must be safe");
    let others_after: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM ownership_stakes WHERE source != 'aggregator'",
            [],
            |row| row.get(0),
        )
        .expect("count non-aggregator after re-run");
    assert_eq!(others_after, 2, "re-run keeps the non-aggregator rows");
}

#[test]
fn migration_0095_maps_financial_sectors_only_from_industrial_default() {
    // ADR 0083 D4 amendment: map unambiguous financial-issuer registry sectors to
    // their sector `statement_type` — but ONLY where the column still holds the
    // 'industrial' default (a manual value is authoritative). Conservative
    // allow-list: banks / insurers / brokers-and-exchanges; borderline sectors
    // (debt collectors, investment holdings) stay untouched.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 94).expect("apply schema through 0094");

    connection
        .execute_batch(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name, sector) VALUES
                ('bank', 'GPW', 'PKO', 'GPW:PKO', 'PKO BP',    'banki komercyjne'),
                ('ins',  'GPW', 'PZU', 'GPW:PZU', 'PZU',       'firmy ubezpieczeniowe'),
                ('brk',  'GPW', 'XTB', 'GPW:XTB', 'XTB',       'giełdy i biura maklerskie'),
                ('exch', 'GPW', 'GPW', 'GPW:GPW', 'GPW',       'giełdy i biura maklerskie'),
                ('ind',  'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT','oprogramowanie'),
                ('debt', 'GPW', 'KRU', 'GPW:KRU', 'KRUK',      'Wierzytelności'),
                ('none', 'GPW', 'ABC', 'GPW:ABC', 'No Sector', NULL);
             -- A manually-set statement_type on a bank-sector company: never overwritten.
             INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name, sector, statement_type) VALUES
                ('manual', 'GPW', 'MAN', 'GPW:MAN', 'Manual Co', 'banki komercyjne', 'insurance');",
        )
        .expect("seed companies with sectors");

    // Isolate to 0095 — the later 0098 maps the debt-collector sector, so applying
    // the full chain here would flip `debt`; 0095's own contract leaves it alone.
    apply_migrations_up_to(&mut connection, 95).expect("apply migration 0095");

    fn st(conn: &rusqlite::Connection, id: &str) -> String {
        conn.query_row(
            "SELECT statement_type FROM companies WHERE id = ?1",
            [id],
            |row| row.get::<_, String>(0),
        )
        .expect("statement_type")
    }

    assert_eq!(
        st(&connection, "bank"),
        "banking",
        "commercial banks → banking"
    );
    assert_eq!(st(&connection, "ins"), "insurance", "insurers → insurance");
    assert_eq!(
        st(&connection, "brk"),
        "specialty_finance",
        "brokers → specialty_finance"
    );
    assert_eq!(
        st(&connection, "exch"),
        "specialty_finance",
        "exchanges → specialty_finance"
    );
    assert_eq!(
        st(&connection, "ind"),
        "industrial",
        "a real-sector industrial stays industrial"
    );
    assert_eq!(
        st(&connection, "debt"),
        "industrial",
        "borderline debt collectors stay untouched (conservative)"
    );
    assert_eq!(
        st(&connection, "none"),
        "industrial",
        "no sector → untouched"
    );
    assert_eq!(
        st(&connection, "manual"),
        "insurance",
        "a manually-set statement_type is authoritative, never overwritten"
    );

    // Idempotent: a re-run flips nothing further (mapped rows are no longer
    // 'industrial').
    apply_migrations_up_to(&mut connection, 95).expect("re-run must be safe");
    assert_eq!(st(&connection, "bank"), "banking");
    assert_eq!(st(&connection, "manual"), "insurance");
    // 0095's contract: the debt-collector sector is left at the default (0098 is
    // what later maps it — asserted in migration_0098_…).
    assert_eq!(st(&connection, "debt"), "industrial");
}

#[test]
fn migration_0098_maps_debt_collectors_but_not_investment_holdings() {
    // ADR 0083 D4 amendment (owner decision 2026-07-18): the debt-collector
    // sector ('Wierzytelności' — KRU) that 0095 conservatively left alone is now
    // mapped to specialty_finance, so Altman Z″ / Piotroski F return NotApplicable.
    // Investment holdings ('Działalność Inwestycyjna' — GKI) STAY untouched (still
    // an open owner decision). Only rows at the 'industrial' default are rewritten.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 97).expect("apply schema through 0097");

    connection
        .execute_batch(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name, sector) VALUES
                ('kru',  'GPW', 'KRU', 'GPW:KRU', 'KRUK',          'Wierzytelności'),
                ('kru2', 'GPW', 'KR2', 'GPW:KR2', 'Other Debt Co', 'Wierzytelności'),
                ('gki',  'GPW', 'GKI', 'GPW:GKI', 'GK Immobile',   'Działalność Inwestycyjna'),
                ('ind',  'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT',    'oprogramowanie');
             -- A manually-set statement_type on a debt collector: never overwritten.
             INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name, sector, statement_type) VALUES
                ('man', 'GPW', 'MAN', 'GPW:MAN', 'Manual Co', 'Wierzytelności', 'banking');",
        )
        .expect("seed companies");

    apply_migrations(&mut connection).expect("apply migration 0098");

    fn st(conn: &rusqlite::Connection, id: &str) -> String {
        conn.query_row(
            "SELECT statement_type FROM companies WHERE id = ?1",
            [id],
            |row| row.get::<_, String>(0),
        )
        .expect("statement_type")
    }

    assert_eq!(
        st(&connection, "kru"),
        "specialty_finance",
        "debt collectors → specialty_finance (Altman/Piotroski not applicable)"
    );
    assert_eq!(
        st(&connection, "kru2"),
        "specialty_finance",
        "every industrial-default debt collector is mapped, not just KRU"
    );
    assert_eq!(
        st(&connection, "gki"),
        "industrial",
        "investment holdings stay untouched (open owner decision)"
    );
    assert_eq!(
        st(&connection, "ind"),
        "industrial",
        "a real-sector industrial stays industrial"
    );
    assert_eq!(
        st(&connection, "man"),
        "banking",
        "a manually-set statement_type is authoritative, never overwritten"
    );

    // Idempotent: a re-run flips nothing further.
    apply_migrations(&mut connection).expect("re-run must be safe");
    assert_eq!(st(&connection, "kru"), "specialty_finance");
    assert_eq!(st(&connection, "gki"), "industrial");
}

#[test]
fn migration_0099_repairs_only_the_misscaled_cdr_q3_2023_facts() {
    // Card e6ebda3: the v0.57 backfill mis-scaled CDR Q3 2023 current_assets /
    // current_liabilities ×1000 (a bare "mln zł" prose token wrongly flipped the
    // scale to Millions). This forward repair divides them back to thousands,
    // touching ONLY the exact mis-scaled auto_unreviewed facts and nothing else.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 98).expect("apply schema through 0098");

    // The canonical current_assets / current_liabilities definitions are already
    // seeded (migration 0089); reference their ids rather than re-inserting.
    let def_id = |conn: &rusqlite::Connection, key: &str| -> String {
        conn.query_row(
            "SELECT id FROM kpi_definitions WHERE metric_key = ?1 AND scope = 'canonical'",
            [key],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| panic!("seeded canonical definition for {key}"))
    };
    let def_ca = def_id(&connection, "current_assets");
    let def_cl = def_id(&connection, "current_liabilities");

    connection
        .execute_batch(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                VALUES ('cdr', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.');
             INSERT INTO financial_periods (id, company_id, fiscal_year, period_type, period_end_date)
                VALUES ('per_q3', 'cdr', 2023, 'Q3', '2023-09-30'),
                       ('per_fy', 'cdr', 2023, 'FY', '2023-12-31');",
        )
        .expect("seed company + periods");

    connection
        .execute(
            "INSERT INTO financial_facts
                (id, company_id, period_id, definition_id, value_numeric, extraction_method,
                 confirmation_state, source_document_ref, statement_basis)
             VALUES
                -- The two mis-scaled facts (×1000 too big), from the Polish Q3 2023 doc.
                ('f_ca', 'cdr', 'per_q3', ?1, '1137807000000', 'html_positional', 'auto_unreviewed',
                 'doc_gpw_cdr_skonsolidowane_sprawozdanie_finansowe_grupy_cd_projekt_za_3q_2023_x.xhtml', 'consolidated'),
                ('f_cl', 'cdr', 'per_q3', ?2, '164335000000',  'html_positional', 'auto_unreviewed',
                 'doc_gpw_cdr_skonsolidowane_sprawozdanie_finansowe_grupy_cd_projekt_za_3q_2023_x.xhtml', 'consolidated'),
                -- Controls that must stay untouched: an already-correct fact (same
                -- metric/period, different basis), and a FY-period fact from another doc.
                ('f_ok', 'cdr', 'per_q3', ?1, '1137807000', 'html_positional', 'auto_unreviewed',
                 'doc_gpw_cdr_skonsolidowane_sprawozdanie_finansowe_grupy_cd_projekt_za_3q_2023_x.xhtml', 'standalone'),
                ('f_fy', 'cdr', 'per_fy', ?1, '1137807000000', 'html_positional', 'auto_unreviewed',
                 'doc_gpw_cdr_annual_2023.xhtml', 'consolidated')",
            rusqlite::params![def_ca, def_cl],
        )
        .expect("seed facts");

    apply_migrations(&mut connection).expect("apply migration 0099");

    fn val(conn: &rusqlite::Connection, id: &str) -> String {
        conn.query_row(
            "SELECT value_numeric FROM financial_facts WHERE id = ?1",
            [id],
            |row| row.get::<_, String>(0),
        )
        .expect("value")
    }

    assert_eq!(
        val(&connection, "f_ca"),
        "1137807000",
        "current_assets ÷1000"
    );
    assert_eq!(
        val(&connection, "f_cl"),
        "164335000",
        "current_liabilities ÷1000"
    );
    assert_eq!(
        val(&connection, "f_ok"),
        "1137807000",
        "an already-correct fact is untouched"
    );
    assert_eq!(
        val(&connection, "f_fy"),
        "1137807000000",
        "a fact in a different (FY) period is not matched"
    );

    // Idempotent: a re-run corrects nothing further (the mis-scaled source values
    // no longer match).
    apply_migrations(&mut connection).expect("re-run must be safe");
    assert_eq!(val(&connection, "f_ca"), "1137807000");
    assert_eq!(val(&connection, "f_cl"), "164335000");
}

#[test]
fn migration_0096_dismisses_stale_unseen_attention_events_only() {
    // v0.57 fix wave 2 (ADR 0068 amendment): the repair clears the pre-existing
    // wall of stale, unseen attention events a history backfill wrote, WITHOUT
    // touching fresh events or events the user already engaged with.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 95).expect("apply schema through 0095");

    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed company");
    connection
        .execute(
            "INSERT INTO alert_rules (id, trigger_type, signal_category, scope_type, scope_ref)
             VALUES ('r1', 'signal_category', 'insider_transaction', 'company', 'c1')",
            [],
        )
        .expect("seed rule");
    // stale + unseen (the backlog to clear); fresh + unseen (keep); stale + seen (keep).
    connection
        .execute(
            "INSERT INTO attention_events
                (id, rule_id, company_id, evidence_type, evidence_ref, fired_at, seen, dismissed)
             VALUES
                ('e_stale', 'r1', 'c1', 'company_signal', 's_stale', '2020-01-01T00:00:00Z', 0, 0),
                ('e_fresh', 'r1', 'c1', 'company_signal', 's_fresh',
                    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 0, 0),
                ('e_seen',  'r1', 'c1', 'company_signal', 's_seen',  '2019-06-01T00:00:00Z', 1, 0)",
            [],
        )
        .expect("seed events");

    apply_migrations(&mut connection).expect("apply 0096/0097");

    // A free helper (not a capturing closure) so the immutable read borrow is
    // released before the idempotency re-run's `&mut connection`.
    fn dismissed(connection: &rusqlite::Connection, id: &str) -> i64 {
        connection
            .query_row(
                "SELECT dismissed FROM attention_events WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .expect("read dismissed")
    }
    assert_eq!(
        dismissed(&connection, "e_stale"),
        1,
        "the stale unseen event is dismissed"
    );
    assert_eq!(
        dismissed(&connection, "e_fresh"),
        0,
        "the fresh event is untouched"
    );
    assert_eq!(
        dismissed(&connection, "e_seen"),
        0,
        "an already-seen stale event is untouched (predicate requires seen = 0)"
    );

    // Idempotent: a re-run flips nothing further.
    apply_migrations(&mut connection).expect("re-run must be safe");
    assert_eq!(dismissed(&connection, "e_stale"), 1);
    assert_eq!(dismissed(&connection, "e_fresh"), 0);
}

#[test]
fn migration_0097_backfills_null_trigger_type_from_the_rule() {
    // v0.57 fix wave 2 (W4): legacy rule-backed events left trigger_type NULL
    // (derived only via COALESCE at read). The repair stamps the column from the
    // owning rule so a direct read / grouping sees the trigger; system events and
    // already-stamped rows are untouched.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 95).expect("apply schema through 0095");

    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed company");
    connection
        .execute(
            "INSERT INTO alert_rules (id, trigger_type, signal_category, scope_type, scope_ref)
             VALUES ('r1', 'autopilot_run_completed', NULL, 'company', 'c1')",
            [],
        )
        .expect("seed rule");
    // Legacy rule-backed event with NULL trigger_type; a system event already carrying one.
    connection
        .execute(
            "INSERT INTO attention_events
                (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at)
             VALUES
                ('e_rule', 'r1', NULL, 'c1', 'autopilot_run', 'run1',
                    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                ('e_sys',  NULL, 'source_reconciliation', 'c1', 'source_reconciliation', 'rec1',
                    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            [],
        )
        .expect("seed events");

    apply_migrations(&mut connection).expect("apply 0097");

    let trigger = |id: &str| -> Option<String> {
        connection
            .query_row(
                "SELECT trigger_type FROM attention_events WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .expect("read trigger_type")
    };
    assert_eq!(
        trigger("e_rule").as_deref(),
        Some("autopilot_run_completed"),
        "the legacy rule-backed event is backfilled from its rule"
    );
    assert_eq!(
        trigger("e_sys").as_deref(),
        Some("source_reconciliation"),
        "a system event keeps its own trigger_type"
    );
}

#[test]
fn migration_0100_creates_analyst_recommendations_and_seeds_catalog_idempotently() {
    // ADR 0073 / plan v0.58 A1: migration 0100 adds `analyst_recommendations`
    // (append-only recommendation history), seeds the `recommendation_change`
    // signal category, and seeds the `biznesradar-rekomendacje` catalog rows. The
    // table uses IF NOT EXISTS and every seed is an idempotent upsert, so re-running
    // the runner is a safe no-op that neither errors nor duplicates.
    let mut connection = open_in_memory_database().expect("database should initialize");

    let category_before: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM signal_categories WHERE key = 'recommendation_change'",
            [],
            |row| row.get(0),
        )
        .expect("count category");
    assert_eq!(
        category_before, 1,
        "the signal category must be seeded once"
    );

    let adapter_before: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM source_adapters WHERE id = 'biznesradar-rekomendacje'",
            [],
            |row| row.get(0),
        )
        .expect("count adapter");
    assert_eq!(adapter_before, 1, "the catalog row must be seeded once");

    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("company insert");
    connection
        .execute(
            "INSERT INTO analyst_recommendations
                (id, company_id, firm, rating, direction, target_price, published_at, source_url)
             VALUES ('r1', 'c1', 'DM BOŚ', 'akumuluj', 'initiate', '120.00', '2026-06-18T08:40:00Z', 'https://x/rekomendacje')",
            [],
        )
        .expect("a recommendation row must be storable");

    // Re-running the runner is a safe no-op on the new table and seeds.
    apply_migrations(&mut connection).expect("re-running migrations should be safe");

    assert_eq!(
        count_rows(&connection, "analyst_recommendations").expect("count"),
        1,
        "re-running migrations must not disturb recommendation data"
    );
    let category_after: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM signal_categories WHERE key = 'recommendation_change'",
            [],
            |row| row.get(0),
        )
        .expect("count category");
    assert_eq!(
        category_before, category_after,
        "category re-seed must not duplicate"
    );

    // The direction CHECK rejects an out-of-taxonomy value.
    let bad = connection.execute(
        "INSERT INTO analyst_recommendations
            (id, company_id, firm, rating, direction, published_at, source_url)
         VALUES ('r2', 'c1', 'DM BOŚ', 'akumuluj', 'nonsense', '2026-06-18T08:40:00Z', 'https://x')",
        [],
    );
    assert!(bad.is_err(), "direction CHECK must reject unknown values");
}
