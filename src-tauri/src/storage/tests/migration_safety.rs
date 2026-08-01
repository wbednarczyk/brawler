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
fn migration_0114_adds_evidence_title_column_tolerant_of_legacy_rows() {
    // v0.60 D7: 0114 appends `attention_events.evidence_title`. Append-only +
    // tolerant-read guard — a row that predates the column must upgrade cleanly and
    // read back with a NULL snapshot (the read model then falls back to live joins).
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 113).expect("apply schema up to pre-0114");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed company");
    connection
        .execute(
            "INSERT INTO attention_events
                (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at)
             VALUES ('legacy_evt', NULL, 'source_reconciliation', 'c1',
                     'source_reconciliation', 'recon_x', '2026-07-14T00:00:00Z')",
            [],
        )
        .expect("seed a legacy attention event on the pre-0114 schema");

    apply_migrations(&mut connection).expect("upgrade to latest");
    assert_eq!(
        count_applied_migrations(&connection).expect("count applied"),
        expected_migration_count(),
        "upgrade should reach the latest migration",
    );
    let snapshot: Option<String> = connection
        .query_row(
            "SELECT evidence_title FROM attention_events WHERE id = 'legacy_evt'",
            [],
            |row| row.get(0),
        )
        .expect("the new column is present and readable on the legacy row");
    assert_eq!(
        snapshot, None,
        "a legacy row's snapshot is NULL (join fallback)"
    );
}

#[test]
fn migration_0118_makes_attention_events_company_nullable_and_preserves_rows() {
    // Epic #40 S3 / ADR 0091 dec. 2: 0118 REBUILDS `attention_events` so a system
    // event with no company scope (a terminally failed workspace-wide job) can be
    // stored. Guards the whole rebuild class (data-model.md): pre-existing rows —
    // rule-owned and system — survive with every column intact, the schema
    // constraints (system dedup, rule-scoped UNIQUE, cascade) still hold, and the
    // NULL company insert that the pre-0118 schema REJECTED now succeeds.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 117).expect("apply schema up to pre-0118");
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
             VALUES ('rule1', 'signal_category', 'profit_warning', 'company', 'c1')",
            [],
        )
        .expect("seed alert rule");
    connection
        .execute(
            "INSERT INTO attention_events
                (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at,
                 seen, dismissed, evidence_title)
             VALUES ('evt_rule', 'rule1', 'signal_category', 'c1', 'company_signal', 'sig1',
                     '2026-07-20T09:00:00Z', 1, 0, 'Raport bieżący 12/2026')",
            [],
        )
        .expect("seed a rule-owned event");
    connection
        .execute(
            "INSERT INTO attention_events
                (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at)
             VALUES ('evt_sys', NULL, 'source_reconciliation', 'c1', 'source_reconciliation',
                     'recon1', '2026-07-21T00:00:00Z')",
            [],
        )
        .expect("seed a system event");

    // The pre-0118 schema refuses a company-less event — the exact constraint the
    // rebuild exists to drop.
    let before = connection.execute(
        "INSERT INTO attention_events
            (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at)
         VALUES ('evt_job_early', NULL, 'job_failed', NULL, 'job', 'job1', '2026-07-22T10:00:00Z')",
        [],
    );
    assert!(
        before.is_err(),
        "pre-0118 `company_id` is NOT NULL — a system-scoped event cannot be stored"
    );

    apply_migrations(&mut connection).expect("upgrade to latest");
    assert_eq!(
        count_applied_migrations(&connection).expect("count applied"),
        expected_migration_count(),
        "upgrade should reach the latest migration",
    );

    // 1. Every pre-existing row survived the rebuild with its columns intact.
    let (rule_id, trigger, company, evidence_title, seen): (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        i64,
    ) = connection
        .query_row(
            "SELECT rule_id, trigger_type, company_id, evidence_title, seen
             FROM attention_events WHERE id = 'evt_rule'",
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
        .expect("the rule-owned event survived");
    assert_eq!(rule_id.as_deref(), Some("rule1"));
    assert_eq!(trigger.as_deref(), Some("signal_category"));
    assert_eq!(company.as_deref(), Some("c1"));
    assert_eq!(
        evidence_title.as_deref(),
        Some("Raport bieżący 12/2026"),
        "the 0114 title snapshot is carried through the rebuild"
    );
    assert_eq!(seen, 1, "seen/dismissed flags survive");
    assert_eq!(
        count_rows(&connection, "attention_events").expect("count events"),
        2,
        "both pre-existing events survived, nothing was duplicated"
    );

    // 2. A company-less system event now stores.
    connection
        .execute(
            "INSERT INTO attention_events
                (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at)
             VALUES ('evt_job', NULL, 'job_failed', NULL, 'job', 'job1', '2026-07-22T10:00:00Z')",
            [],
        )
        .expect("a system-scoped event with no company is storable after 0118");

    // 3. The system dedup index came back with the table: the same
    //    (trigger, evidence_type, evidence_ref) cannot fire twice.
    let duplicate = connection.execute(
        "INSERT INTO attention_events
            (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at)
         VALUES ('evt_job_dup', NULL, 'job_failed', NULL, 'job', 'job1', '2026-07-22T11:00:00Z')",
        [],
    );
    assert!(
        duplicate.is_err(),
        "the system dedup partial index survives the rebuild"
    );

    // 4. The company cascade still works — deleting the company clears its events
    //    while the company-less system event stays.
    connection
        .execute("PRAGMA foreign_keys = ON", [])
        .expect("enable FK enforcement");
    connection
        .execute("DELETE FROM companies WHERE id = 'c1'", [])
        .expect("delete the company");
    let remaining: Vec<String> = {
        let mut statement = connection
            .prepare("SELECT id FROM attention_events ORDER BY id")
            .expect("prepare");
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect");
        rows
    };
    assert_eq!(
        remaining,
        vec!["evt_job".to_owned()],
        "company-scoped events cascade with their company; the system event survives"
    );
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
fn migration_0110_copies_witness_pages_as_income_and_is_idempotent() {
    // ADR 0086 dec. 2 / plan C2: the single-page witness cache
    // (`fundamentals_witness_pages`, PK company_id) becomes a per-(company,
    // page_kind) cache. The existing rows must carry forward as the `income` page
    // kind, the old table must survive (a migration never deletes user data), and
    // re-running must not duplicate or clobber.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 104).expect("apply schema through 0104");

    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed a company");
    connection
        .execute(
            "INSERT INTO fundamentals_witness_pages (company_id, page_url, html, status, fetched_at)
             VALUES ('c1', 'https://www.biznesradar.pl/raporty-finansowe-rachunek-zyskow-i-strat/CDR',
                     '<html>income</html>', 'ok', '2026-07-20T10:00:00.000Z')",
            [],
        )
        .expect("seed a pre-0110 witness page");

    apply_migrations(&mut connection).expect("upgrade to the latest schema");

    // The income row carried forward with its body and timestamp intact.
    let (kind, url, html, status, fetched): (String, String, Option<String>, String, String) =
        connection
            .query_row(
                "SELECT page_kind, page_url, html, status, fetched_at
                 FROM fundamentals_aggregator_pages WHERE company_id = 'c1'",
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
            .expect("the witness income row must be copied into the new table");
    assert_eq!(kind, "income");
    assert_eq!(html.as_deref(), Some("<html>income</html>"));
    assert_eq!(status, "ok");
    assert_eq!(fetched, "2026-07-20T10:00:00.000Z");
    assert!(url.ends_with("/CDR"));

    // The old table is kept (append-only), its row untouched.
    assert_eq!(
        count_rows(&connection, "fundamentals_witness_pages").expect("count old table"),
        1,
        "migration 0110 must not delete the legacy witness cache",
    );

    // The `inventories` catalog metric is seeded so aggregator inventory facts persist.
    let inventories: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM kpi_definitions WHERE metric_key = 'inventories'",
            [],
            |row| row.get(0),
        )
        .expect("query inventories definition");
    assert_eq!(
        inventories, 1,
        "the inventories KPI definition must be seeded"
    );

    // The CHECK rejects an unknown page kind.
    let bad = connection.execute(
        "INSERT INTO fundamentals_aggregator_pages (company_id, page_kind, page_url, status)
         VALUES ('c1', 'notes', 'https://x', 'ok')",
        [],
    );
    assert!(bad.is_err(), "page_kind CHECK must reject an unknown kind");

    // Idempotent re-run: no duplicated migration rows, the income row preserved.
    apply_migrations(&mut connection).expect("re-running migrations is safe");
    assert_eq!(
        count_rows(&connection, "fundamentals_aggregator_pages").expect("count new table"),
        1,
        "re-running 0110 must not duplicate the copied income row",
    );
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
fn migration_0121_adds_detected_container_null_and_leaves_old_rows_untouched() {
    // Epic #229 T2 (#193): 0121 appends `report_documents.detected_container`.
    // Append-only + tolerant-read guard — a row stored before the column must
    // upgrade cleanly, keep every other value, and read back NULL ("not yet
    // sniffed"), which the resolver falls back from and the startup self-heal
    // fills. There is deliberately NO SQL backfill: SQL cannot read file bytes.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 120).expect("apply schema through 0120");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed company");
    connection
        .execute(
            "INSERT INTO report_documents
                (id, company_id, source_type, url, local_path, content_type, fetch_status, doc_kind)
             VALUES ('legacy_doc', 'c1', 'espi_attachment', 'https://x/ssf.pdf',
                     'report_documents/legacy_doc.pdf', 'application/octet-stream',
                     'fetched', 'periodic_ssf')",
            [],
        )
        .expect("seed a legacy report document on the pre-0121 schema");

    apply_migrations(&mut connection).expect("upgrade to latest");
    assert_eq!(
        count_applied_migrations(&connection).expect("count applied"),
        expected_migration_count(),
        "upgrade should reach the latest migration",
    );

    let (container, local_path, content_type, kind): (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = connection
        .query_row(
            "SELECT detected_container, local_path, content_type, doc_kind
             FROM report_documents WHERE id = 'legacy_doc'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("the new column is present and readable on the legacy row");
    assert_eq!(
        container, None,
        "a legacy row is NULL — not yet sniffed, never a guessed value"
    );
    assert_eq!(
        local_path,
        Some("report_documents/legacy_doc.pdf".to_owned()),
        "the append must not disturb existing columns",
    );
    assert_eq!(content_type, Some("application/octet-stream".to_owned()));
    assert_eq!(kind, Some("periodic_ssf".to_owned()));

    // Idempotent: re-running the runner neither errors nor rewrites the column.
    apply_migrations(&mut connection).expect("re-run must be safe");
    let after: Option<String> = connection
        .query_row(
            "SELECT detected_container FROM report_documents WHERE id = 'legacy_doc'",
            [],
            |row| row.get(0),
        )
        .expect("read back after re-run");
    assert_eq!(after, None);
}

#[test]
fn migration_0120_repairs_eps_shares_currency_and_spares_every_other_fact() {
    // #93 (epic #229 T4): the ESEF divide-unit bug stored 76 EPS facts (38
    // eps_basic + 38 eps_diluted) with currency = 'shares' — the denominator of
    // the `iso4217:PLN / xbrli:shares` ratio unit. The repair rewrites exactly
    // that class to PLN and touches nothing else.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 119).expect("apply schema through 0119");

    let def_id = |conn: &rusqlite::Connection, key: &str| -> String {
        conn.query_row(
            "SELECT id FROM kpi_definitions WHERE metric_key = ?1 AND scope = 'canonical'",
            [key],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| panic!("seeded canonical definition for {key}"))
    };
    let def_basic = def_id(&connection, "eps_basic");
    let def_diluted = def_id(&connection, "eps_diluted");
    let def_revenue = def_id(&connection, "revenue");

    connection
        .execute_batch(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                VALUES ('cdr', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.');
             INSERT INTO financial_periods (id, company_id, fiscal_year, period_type, period_end_date)
                VALUES ('per_fy', 'cdr', 2024, 'FY', '2024-12-31'),
                       ('per_fy23', 'cdr', 2023, 'FY', '2023-12-31');",
        )
        .expect("seed company + periods");

    connection
        .execute(
            "INSERT INTO financial_facts
                (id, company_id, period_id, definition_id, value_numeric, currency, extraction_method)
             VALUES
                -- The bug class: both EPS metrics carrying the shares denominator.
                ('f_basic',   'cdr', 'per_fy', ?1, '3.21', 'shares', 'esef'),
                ('f_diluted', 'cdr', 'per_fy', ?2, '3.15', 'shares', 'esef'),
                -- Controls: an already-correct EPS fact, an EPS fact reported in
                -- another currency, and a non-EPS fact (whatever its currency).
                ('f_ok',      'cdr', 'per_fy23', ?1, '3.21', 'PLN',  'esef'),
                ('f_eur',     'cdr', 'per_fy23', ?2, '0.75', 'EUR',  'esef'),
                ('f_revenue', 'cdr', 'per_fy', ?3, '1200', 'PLN',    'esef')",
            rusqlite::params![def_basic, def_diluted, def_revenue],
        )
        .expect("seed facts");
    // A control fact that is NOT an EPS metric yet carries the same non-ISO unit:
    // the repair is deliberately scoped to the audited class and must skip it.
    connection
        .execute(
            "UPDATE financial_facts SET currency = 'shares' WHERE id = 'f_revenue'",
            [],
        )
        .expect("force the non-EPS control");

    apply_migrations(&mut connection).expect("apply migration 0120");

    fn currency(conn: &rusqlite::Connection, id: &str) -> Option<String> {
        conn.query_row(
            "SELECT currency FROM financial_facts WHERE id = ?1",
            [id],
            |row| row.get::<_, Option<String>>(0),
        )
        .expect("currency")
    }

    assert_eq!(currency(&connection, "f_basic").as_deref(), Some("PLN"));
    assert_eq!(currency(&connection, "f_diluted").as_deref(), Some("PLN"));
    assert_eq!(
        currency(&connection, "f_ok").as_deref(),
        Some("PLN"),
        "an already-correct EPS fact is untouched"
    );
    assert_eq!(
        currency(&connection, "f_eur").as_deref(),
        Some("EUR"),
        "a non-PLN EPS fact must never be rewritten"
    );
    assert_eq!(
        currency(&connection, "f_revenue").as_deref(),
        Some("shares"),
        "a non-EPS metric is outside the audited class"
    );

    // Idempotent: nothing matches 'shares' among the EPS metrics any more.
    apply_migrations(&mut connection).expect("re-run must be safe");
    assert_eq!(currency(&connection, "f_basic").as_deref(), Some("PLN"));
    assert_eq!(
        currency(&connection, "f_revenue").as_deref(),
        Some("shares")
    );
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

/// ADR 0084 decision 5: the in-app AI analysis layer is retired, so no handler
/// is registered for its job kinds any more. A durable queue carried over from
/// a pre-removal install still holds `pending`/`running` rows of those kinds —
/// work nothing can ever claim. Migration 0101 purges exactly those rows so the
/// queue cannot wedge on unknown work, while leaving every surviving kind and
/// every *terminal* row (the execution record) untouched.
///
/// Forward, idempotent, self-healing: re-running the runner is a no-op, and the
/// purge is scoped to unclaimable statuses so it never destroys history.
#[test]
fn migration_0101_purges_queued_jobs_of_removed_ai_kinds() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 100).expect("apply schema through 0100");

    // A realistic pre-removal snapshot: queued work for every retired kind,
    // plus surviving kinds that must be left completely alone.
    let seeded: [(&str, &str, &str); 11] = [
        ("q1", "ai_analysis", "pending"),
        ("q2", "claim_extraction", "pending"),
        ("q3", "kpi_extraction", "pending"),
        ("q4", "research_brief", "running"),
        ("q5", "research_digest", "pending"),
        ("q6", "qualitative_assessment", "pending"),
        // Terminal rows of retired kinds are execution history, not work.
        ("q7", "ai_analysis", "succeeded"),
        ("q8", "kpi_extraction", "failed"),
        // Survivors: still handled after the retirement.
        ("q9", "autopilot_stage", "pending"),
        ("q10", "source_refresh", "pending"),
        ("q11", "morning_briefing", "pending"),
    ];
    for (id, kind, status) in seeded {
        connection
            .execute(
                "INSERT INTO job_queue (id, kind, payload, status) VALUES (?1, ?2, '{}', ?3)",
                rusqlite::params![id, kind, status],
            )
            .expect("seed a pre-removal queue row");
    }

    apply_migrations(&mut connection).expect("apply migration 0101");

    let remaining: Vec<String> = connection
        .prepare("SELECT id FROM job_queue ORDER BY id")
        .expect("prepare")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("collect");

    // Unclaimable work of retired kinds is gone.
    for gone in ["q1", "q2", "q3", "q4", "q5", "q6"] {
        assert!(
            !remaining.iter().any(|id| id == gone),
            "queued job {gone} of a removed AI kind must be purged, remaining: {remaining:?}"
        );
    }
    // History and survivors are untouched.
    for kept in ["q7", "q8", "q9", "q10", "q11"] {
        assert!(
            remaining.iter().any(|id| id == kept),
            "row {kept} must survive the purge, remaining: {remaining:?}"
        );
    }

    // Idempotent: running the runner again changes nothing.
    let after_first = remaining.len();
    apply_migrations(&mut connection).expect("re-running migrations should be safe");
    assert_eq!(
        count_applied_migrations(&connection).expect("count applied"),
        expected_migration_count(),
        "re-run must not add or drop migration rows",
    );
    assert_eq!(
        count_rows(&connection, "job_queue").expect("count job_queue"),
        after_first as i64,
        "re-running the purge must be a no-op",
    );

    // Self-healing: a removed kind re-inserted by a stale binary is purged on
    // the next run of the same forward migration content, and — most
    // importantly — the queue still dispatches a surviving kind afterwards.
    let state = AppState::new(connection);
    let worker = crate::jobs::handlers::build_worker(state.clone());
    state
        .jobs()
        .enqueue("survivor", "source_refresh", "{\"adapterId\":\"x\"}", 1)
        .expect("enqueue a surviving kind");
    assert!(
        state.jobs().counts().expect("counts").pending > 0,
        "the queue must still accept and hold work for surviving kinds"
    );
    let _ = worker;
}

/// Seed a realistic pre-cut snapshot: one row in every table the clean cut drops,
/// an AI-tier fact + its provenance, and the deterministic data that MUST survive
/// (a deterministic fact + provenance, an engine-sourced `criterion_results` row,
/// a `company_extraction_profile`, the transcription setting). Returns the
/// `(ai_fact_id, deterministic_fact_id)` pair the assertions key on.
fn seed_pre_clean_cut_snapshot(connection: &rusqlite::Connection) -> (String, String) {
    connection
        .execute_batch(
            "
            INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.');
            INSERT INTO financial_periods (id, company_id, fiscal_year, period_type)
                VALUES ('p1', 'c1', 2025, 'FY');
            INSERT INTO financial_periods (id, company_id, fiscal_year, period_type)
                VALUES ('p2', 'c1', 2024, 'FY');
            INSERT INTO financial_periods (id, company_id, fiscal_year, period_type)
                VALUES ('p3', 'c1', 2023, 'FY');
            INSERT OR IGNORE INTO source_adapters (id, display_name, source_type, fetch_mode)
                VALUES ('bankier-company-komunikaty', 'Bankier', 'official_report', 'pull');
            INSERT INTO feed_items (id, type, source_adapter_id, source_name, source_url, title, fetched_at, dedupe_key)
                VALUES ('f1', 'official_report', 'bankier-company-komunikaty', 'Bankier',
                        'https://x/1', 'Report', '2026-01-01T00:00:00Z', 'dk1');
            INSERT INTO report_documents (id, company_id, source_type, url)
                VALUES ('d1', 'c1', 'user_url', 'https://x/doc.pdf');
            ",
        )
        .expect("seed base rows");

    // --- the two facts: one AI-sourced (must go), one deterministic (must stay) ---
    connection
        .execute_batch(
            "
            INSERT INTO financial_facts (id, company_id, period_id, definition_id, value_numeric)
                SELECT 'fact_ai', 'c1', 'p1', id, '111' FROM kpi_definitions
                 WHERE metric_key = 'total_assets' LIMIT 1;
            INSERT INTO financial_facts (id, company_id, period_id, definition_id, value_numeric)
                SELECT 'fact_det', 'c1', 'p2', id, '222' FROM kpi_definitions
                 WHERE metric_key = 'total_assets' LIMIT 1;
            INSERT INTO financial_fact_provenance (fact_id, source_tier, validation_status, citation)
                VALUES ('fact_ai', 'ai', 'unreviewed', 'Aktywa razem');
            INSERT INTO financial_facts (id, company_id, period_id, definition_id, value_numeric)
                SELECT 'fact_ai_text', 'c1', 'p3', id, '333' FROM kpi_definitions
                 WHERE metric_key = 'total_assets' LIMIT 1;
            INSERT INTO financial_fact_provenance (fact_id, source_tier, validation_status, citation)
                VALUES ('fact_ai_text', 'ai_text', 'unreviewed', 'Aktywa razem');
            INSERT INTO financial_fact_provenance (fact_id, source_tier, validation_status, citation)
                VALUES ('fact_det', 'esef', 'passed', 'ifrs-full:Assets');
            ",
        )
        .expect("seed facts + provenance");

    // A deterministic fact that supersedes the AI one, and a manual claim verified
    // by it: both are KEPT rows holding a reference the cut must resolve, not orphan.
    connection
        .execute(
            "UPDATE financial_facts SET supersedes_id = 'fact_ai' WHERE id = 'fact_det'",
            [],
        )
        .expect("link supersedes");
    connection
        .execute(
            "INSERT INTO management_claims (id, company_id, statement, verifying_fact_id)
             VALUES ('claim1', 'c1', 'Revenue will grow.', 'fact_ai')",
            [],
        )
        .expect("seed a manual claim verified by the AI fact");

    // --- one row in every dropped table ---
    connection
        .execute_batch(
            "
            INSERT INTO ai_analysis_jobs (id, feed_item_id, provider_id, model, prompt_version, status)
                VALUES ('aj1', 'f1', 'provider_gemini', 'm', 'v1', 'succeeded');
            INSERT INTO ai_analysis_results (id, feed_item_id, provider_id, model, summary, significance, reasoning)
                VALUES ('ar1', 'f1', 'provider_gemini', 'm', 's', 'medium', 'r');
            INSERT INTO ai_analysis_tags (ai_analysis_result_id, tag) VALUES ('ar1', 'earnings');
            INSERT INTO ai_analysis_source_references (id, ai_analysis_result_id, source_url)
                VALUES ('asr1', 'ar1', 'https://x/1');

            INSERT INTO ai_research_brief_jobs (id, scope_type, scope_id, provider_id, model, prompt_version, evidence_collector_version, renderer_version, status)
                VALUES ('bj1', 'company', 'c1', 'p', 'm', 'v', 'v', 'v', 'succeeded');
            INSERT INTO ai_research_briefs (id, job_id, scope_type, scope_id, provider_id, model, prompt_version, evidence_collector_version, renderer_version, title, summary, content_markdown)
                VALUES ('b1', 'bj1', 'company', 'c1', 'p', 'm', 'v', 'v', 'v', 't', 's', 'c');
            INSERT INTO ai_research_brief_citations (id, brief_id, citation_key, evidence_type, evidence_id, label)
                VALUES ('bc1', 'b1', 'b1', 'feed_item', 'f1', 'l');

            INSERT INTO ai_research_digest_jobs (id, scope_type, scope_id, provider_id, model, prompt_version, evidence_collector_version, renderer_version, status)
                VALUES ('dj1', 'company', 'c1', 'p', 'm', 'v', 'v', 'v', 'succeeded');
            INSERT INTO ai_research_digests (id, job_id, scope_type, scope_id, provider_id, model, prompt_version, evidence_collector_version, renderer_version, title, summary, content_markdown)
                VALUES ('dg1', 'dj1', 'company', 'c1', 'p', 'm', 'v', 'v', 'v', 't', 's', 'c');
            INSERT INTO ai_research_digest_citations (id, digest_id, citation_key, evidence_type, evidence_id, label)
                VALUES ('dc1', 'dg1', 'b1', 'feed_item', 'f1', 'l');

            INSERT INTO claim_extraction_jobs (id, company_id, source_type, source_id, provider_id, model, prompt_version, status)
                VALUES ('cj1', 'c1', 'report_document', 'd1', 'p', 'm', 'v', 'succeeded');
            INSERT INTO claim_extraction_proposals (id, job_id, statement)
                VALUES ('cp1', 'cj1', 'Management expects growth.');

            INSERT INTO kpi_extraction_jobs (id, company_id, report_document_id, provider_id, model, prompt_version, status)
                VALUES ('kj1', 'c1', 'd1', 'p', 'm', 'v', 'succeeded');
            INSERT INTO kpi_extraction_proposals (id, job_id, metric_key, label, value_numeric)
                VALUES ('kp1', 'kj1', 'total_assets', 'Aktywa razem', '111');

            INSERT INTO ownership_ocr_proposals (report_document_id, company_id, source_document_id, as_of)
                VALUES ('d1', 'c1', 'd1', '2025-12-31');
            INSERT INTO ownership_ocr_proposal_rows (id, report_document_id, row_index, holder_name_raw)
                VALUES ('or1', 'd1', 0, 'Fundusz X');

            INSERT INTO ownership_holder_type_proposals (id, company_id, holder_name_normalized, proposed_type)
                VALUES ('hp1', 'c1', 'fundusz x', 'tfi');

            INSERT INTO company_ocr_extraction_profile (company_id, template_hash, scale, profile_json, version)
                VALUES ('c1', 'hash', 'Thousands', '{}', 1);
            ",
        )
        .expect("seed every dropped table");

    // --- deterministic data that MUST survive ---
    connection
        .execute_batch(
            "
            INSERT INTO history_sweeps
                (id, company_id, trigger, status, candidates_total, runs_enqueued,
                 ai_calls_used, ai_call_limit)
                VALUES ('sweep1', 'c1', 'manual', 'completed', 7, 5, 3, 30);

            INSERT INTO company_extraction_profile (company_id, template_hash, profile_json)
                VALUES ('c1', 'det-hash', '{\"label_map\":{}}');

            INSERT INTO quality_frameworks (id, name, version)
                VALUES ('qf1', 'Quality', 1);
            INSERT INTO framework_evaluations (id, framework_id, framework_version, company_id, engine_version)
                VALUES ('fe1', 'qf1', 1, 'c1', 'e1');
            INSERT INTO criterion_results (id, evaluation_id, ordinal, label, expression, verdict)
                VALUES ('cr1', 'fe1', 0, 'ROE > 10%', 'roe > 10', 'pass');

            INSERT INTO morning_briefings (id, since, narrative_markdown, narrative_provider_id, narrative_model)
                VALUES ('mb1', '2026-01-01', '## Narrative', 'provider_gemini', 'm');
            ",
        )
        .expect("seed deterministic survivors");

    // The settings rows the cut deletes, plus the one it must KEEP.
    connection
        .execute_batch(
            "
            INSERT OR REPLACE INTO settings (key, value, value_type) VALUES
                ('ai_analysis_mode', 'source_grounded', 'string'),
                ('ai_workers', '2', 'integer'),
                ('ai_provider_concurrency', '2', 'integer'),
                ('capability_providers', '{}', 'json'),
                ('general_analysis_provider', 'provider_gemini', 'string'),
                ('espi_ai_fallback_enabled', 'false', 'string'),
                ('history_sweep_ai_call_limit', '30', 'integer'),
                ('youtube_transcription_provider', 'provider_gemini', 'string');
            ",
        )
        .expect("seed settings rows");

    ("fact_ai".to_owned(), "fact_det".to_owned())
}

fn table_exists(connection: &rusqlite::Connection, name: &str) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [name],
            |row| row.get::<_, bool>(0),
        )
        .expect("query sqlite_master")
}

fn setting_exists(connection: &rusqlite::Connection, key: &str) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM settings WHERE key = ?1)",
            [key],
            |row| row.get::<_, bool>(0),
        )
        .expect("query settings")
}

/// Every table the clean cut drops (ADR 0084 decision 5, revised 2026-07-20).
const CLEAN_CUT_DROPPED_TABLES: [&str; 18] = [
    "ai_analysis_results",
    "ai_analysis_jobs",
    "ai_analysis_tags",
    "ai_analysis_source_references",
    "ai_research_briefs",
    "ai_research_brief_citations",
    "ai_research_brief_jobs",
    "ai_research_digests",
    "ai_research_digest_citations",
    "ai_research_digest_jobs",
    "claim_extraction_jobs",
    "claim_extraction_proposals",
    "kpi_extraction_jobs",
    "kpi_extraction_proposals",
    "ownership_ocr_proposals",
    "ownership_ocr_proposal_rows",
    "ownership_holder_type_proposals",
    "company_ocr_extraction_profile",
];

/// ADR 0084 decision 5 (revised — **clean cut**): the AI artifacts are removed,
/// not orphaned. Applied to a realistic pre-cut snapshot, the migration must drop
/// every AI table, strip the narrative columns, delete the retired settings rows,
/// and delete the `source_tier='ai'` facts **together with** their provenance.
///
/// The negative assertions carry the weight here: an over-broad DELETE that also
/// took deterministic facts, `criterion_results` (all `source='engine'` — named
/// like AI, measured as deterministic), `company_extraction_profile`, the briefing
/// rows, or the transcription setting would be the expensive, silent failure.
#[test]
fn migration_0102_clean_cut_removes_ai_artifacts_and_spares_deterministic_data() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 101).expect("apply schema through 0101");
    let (ai_fact, det_fact) = seed_pre_clean_cut_snapshot(&connection);

    for table in CLEAN_CUT_DROPPED_TABLES {
        assert!(
            table_exists(&connection, table),
            "precondition: {table} must exist before the cut"
        );
    }

    apply_migrations(&mut connection).expect("apply the clean-cut migration");

    // ---- removed ----
    for table in CLEAN_CUT_DROPPED_TABLES {
        assert!(
            !table_exists(&connection, table),
            "{table} must be dropped by the clean cut"
        );
    }
    for key in [
        "ai_analysis_mode",
        "ai_workers",
        "ai_provider_concurrency",
        "capability_providers",
        "general_analysis_provider",
        "espi_ai_fallback_enabled",
        "history_sweep_ai_call_limit",
    ] {
        assert!(
            !setting_exists(&connection, key),
            "settings row {key} must be deleted by the clean cut"
        );
    }
    let narrative_columns: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('morning_briefings')
             WHERE name IN ('narrative_markdown', 'narrative_provider_id', 'narrative_model')",
            [],
            |row| row.get(0),
        )
        .expect("inspect morning_briefings columns");
    assert_eq!(
        narrative_columns, 0,
        "the three narrative columns must be dropped from morning_briefings"
    );

    // The AI fact AND its provenance are gone — together.
    let ai_fact_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM financial_facts WHERE id = ?1",
            [&ai_fact],
            |row| row.get(0),
        )
        .expect("count ai fact");
    assert_eq!(ai_fact_rows, 0, "the source_tier='ai' fact must be deleted");
    let ai_prov_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM financial_fact_provenance
             WHERE source_tier IN ('ai', 'ai_text')",
            [],
            |row| row.get(0),
        )
        .expect("count ai provenance");
    assert_eq!(
        ai_prov_rows, 0,
        "the AI provenance rows must be deleted with their facts"
    );
    // `ai_text` is the other AI-sourced provenance value (the retired tier-4
    // path wrote it); it must be cut on the same terms as `ai`.
    let ai_text_fact_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM financial_facts WHERE id = 'fact_ai_text'",
            [],
            |row| row.get(0),
        )
        .expect("count ai_text fact");
    assert_eq!(
        ai_text_fact_rows, 0,
        "the source_tier='ai_text' fact must be deleted too"
    );

    // The vestigial AI columns are gone from the surviving tables.
    let sweep_ai_columns: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('history_sweeps')
             WHERE name IN ('ai_calls_used', 'ai_call_limit')",
            [],
            |row| row.get(0),
        )
        .expect("inspect history_sweeps columns");
    assert_eq!(
        sweep_ai_columns, 0,
        "the tier-4 AI budget columns must be dropped from history_sweeps"
    );
    let claim_proposal_column: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('management_claims')
             WHERE name = 'extraction_proposal_id'",
            [],
            |row| row.get(0),
        )
        .expect("inspect management_claims columns");
    assert_eq!(
        claim_proposal_column, 0,
        "the dangling extraction_proposal_id column must be dropped"
    );

    // ---- NOT removed (the assertions that catch an over-broad DELETE) ----
    let det_fact_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM financial_facts WHERE id = ?1",
            [&det_fact],
            |row| row.get(0),
        )
        .expect("count deterministic fact");
    assert_eq!(det_fact_rows, 1, "deterministic facts must survive the cut");
    let det_prov: String = connection
        .query_row(
            "SELECT source_tier FROM financial_fact_provenance WHERE fact_id = ?1",
            [&det_fact],
            |row| row.get(0),
        )
        .expect("deterministic provenance must survive");
    assert_eq!(det_prov, "esef");

    assert_eq!(
        count_rows(&connection, "criterion_results").expect("count criterion_results"),
        1,
        "criterion_results is deterministic (source='engine') and must NOT be touched"
    );
    assert_eq!(
        count_rows(&connection, "company_extraction_profile").expect("count profiles"),
        1,
        "company_extraction_profile: kept append-only even though its read/write code is retired (ADR 0086 dec. 1) — a migration must never DROP user-data tables"
    );
    assert_eq!(
        count_rows(&connection, "morning_briefings").expect("count briefings"),
        1,
        "the briefing row itself is deterministic composition and must survive"
    );
    assert_eq!(
        count_rows(&connection, "management_claims").expect("count claims"),
        1,
        "the manual claims path must survive"
    );
    // The claim's own data is untouched — only the dead column went.
    let (claim_statement, claim_company): (String, String) = connection
        .query_row(
            "SELECT statement, company_id FROM management_claims WHERE id = 'claim1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("claim row must survive intact");
    assert_eq!(claim_statement, "Revenue will grow.");
    assert_eq!(claim_company, "c1");

    // The sweep ROW survives with every non-AI field intact — only its two AI
    // accounting columns were dropped.
    assert_eq!(
        count_rows(&connection, "history_sweeps").expect("count sweeps"),
        1,
        "history sweep rows are execution history and must survive"
    );
    let (sweep_status, sweep_candidates, sweep_enqueued, sweep_trigger): (
        String,
        i64,
        i64,
        String,
    ) = connection
        .query_row(
            "SELECT status, candidates_total, runs_enqueued, trigger
             FROM history_sweeps WHERE id = 'sweep1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("sweep row must survive intact");
    assert_eq!(sweep_status, "completed");
    assert_eq!(sweep_candidates, 7, "non-AI sweep counters are untouched");
    assert_eq!(sweep_enqueued, 5, "non-AI sweep counters are untouched");
    assert_eq!(sweep_trigger, "manual");
    assert!(
        setting_exists(&connection, "youtube_transcription_provider"),
        "the transcription setting must be KEPT"
    );

    // ---- referential integrity: no fact left without provenance ----
    let orphan_facts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM financial_facts f
             WHERE NOT EXISTS (SELECT 1 FROM financial_fact_provenance p WHERE p.fact_id = f.id)",
            [],
            |row| row.get(0),
        )
        .expect("count facts without provenance");
    assert_eq!(
        orphan_facts, 0,
        "no surviving fact may be left without its provenance row"
    );
    let dangling_prov: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM financial_fact_provenance p
             WHERE NOT EXISTS (SELECT 1 FROM financial_facts f WHERE f.id = p.fact_id)",
            [],
            |row| row.get(0),
        )
        .expect("count provenance without a fact");
    assert_eq!(
        dangling_prov, 0,
        "no provenance row may be left pointing at a deleted fact"
    );

    // References into the deleted fact are resolved, not left dangling.
    let claim_link: Option<String> = connection
        .query_row(
            "SELECT verifying_fact_id FROM management_claims WHERE id = 'claim1'",
            [],
            |row| row.get(0),
        )
        .expect("read claim link");
    assert!(
        claim_link.is_none(),
        "a claim verified by a deleted AI fact must be unlinked, not left dangling"
    );
    let supersedes: Option<String> = connection
        .query_row(
            "SELECT supersedes_id FROM financial_facts WHERE id = ?1",
            [&det_fact],
            |row| row.get(0),
        )
        .expect("read supersedes link");
    assert!(
        supersedes.is_none(),
        "a supersedes pointer at a deleted AI fact must be cleared"
    );

    // ---- idempotent ----
    apply_migrations(&mut connection).expect("re-running migrations must be safe");
    assert_eq!(
        count_applied_migrations(&connection).expect("count applied"),
        expected_migration_count(),
        "re-run must not add or drop migration rows"
    );
    assert_eq!(
        count_rows(&connection, "financial_facts").expect("count facts"),
        1,
        "re-running the clean cut must not delete anything further"
    );
}

/// Self-heal: the clean cut applied to a database where the AI tables were
/// already gone (a partially-cut or hand-repaired install) must succeed, not
/// abort the whole migration run.
#[test]
fn migration_0102_clean_cut_self_heals_when_the_tables_are_already_absent() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 101).expect("apply schema through 0101");

    // Simulate an install where the AI tables never survived to this point.
    for table in CLEAN_CUT_DROPPED_TABLES {
        connection
            .execute(&format!("DROP TABLE IF EXISTS {table}"), [])
            .expect("pre-drop the table");
    }

    apply_migrations(&mut connection).expect("the clean cut must self-heal on absent tables");
    assert_eq!(
        count_applied_migrations(&connection).expect("count applied"),
        expected_migration_count(),
        "the migration must still record as applied",
    );
}

/// ADR 0061 decision 2 / plan v0.59 A2: migration 0103 adds
/// `fundamentals_extraction_outcomes` — the durable record of what the
/// deterministic pipeline concluded about a period, INCLUDING the runs that
/// emitted nothing. Forward, idempotent, self-healing: `IF NOT EXISTS` on the
/// table and both indexes, so re-running the runner is a safe no-op.
#[test]
fn migration_0103_creates_extraction_outcomes_idempotently() {
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
            "INSERT INTO fundamentals_extraction_outcomes
                (id, company_id, report_document_id, fiscal_year, period_type, period_end,
                 tier, acceptance, reason_code)
             VALUES ('fxo_1', 'c1', 'doc_1', 2026, 'FY', '2026-12-31',
                     'pdf', 'flagged', 'validation_failed')",
            [],
        )
        .expect("a flagged outcome must be storable");

    // Re-running the runner is a safe no-op on the new table.
    apply_migrations(&mut connection).expect("re-running migrations should be safe");
    assert_eq!(
        count_rows(&connection, "fundamentals_extraction_outcomes").expect("count"),
        1,
        "re-running migrations must not disturb recorded outcomes"
    );

    // The slot is unique: the same (company, document, period) cannot be
    // recorded twice under a different id — a re-run updates, never accumulates.
    let duplicate_slot = connection.execute(
        "INSERT INTO fundamentals_extraction_outcomes
            (id, company_id, report_document_id, fiscal_year, period_type, period_end,
             acceptance, reason_code)
         VALUES ('fxo_2', 'c1', 'doc_1', 2026, 'FY', '2026-12-31', 'empty', 'no_deterministic_tier')",
        [],
    );
    assert!(
        duplicate_slot.is_err(),
        "the slot uniqueness index must reject a second row for the same period"
    );

    // The typed-reason CHECK rejects prose: a reason code out of the taxonomy is
    // exactly the "English diagnosis" ADR 0084 §6 forbids.
    let prose_reason = connection.execute(
        "INSERT INTO fundamentals_extraction_outcomes
            (id, company_id, report_document_id, fiscal_year, period_type, period_end,
             acceptance, reason_code)
         VALUES ('fxo_3', 'c1', 'doc_2', 2026, 'Q1', '2026-03-31', 'flagged',
                 'the numbers did not add up')",
        [],
    );
    assert!(
        prose_reason.is_err(),
        "reason_code CHECK must reject anything outside the typed vocabulary"
    );

    // Likewise for an out-of-taxonomy acceptance.
    let bad_acceptance = connection.execute(
        "INSERT INTO fundamentals_extraction_outcomes
            (id, company_id, report_document_id, fiscal_year, period_type, period_end,
             acceptance, reason_code)
         VALUES ('fxo_4', 'c1', 'doc_2', 2026, 'Q1', '2026-03-31', 'nonsense', 'emitted')",
        [],
    );
    assert!(
        bad_acceptance.is_err(),
        "acceptance CHECK must reject unknown verdicts"
    );

    // Deleting the company takes its outcomes with it (ON DELETE CASCADE).
    connection
        .execute("PRAGMA foreign_keys = ON", [])
        .expect("enable fks");
    connection
        .execute("DELETE FROM companies WHERE id = 'c1'", [])
        .expect("delete company");
    assert_eq!(
        count_rows(&connection, "fundamentals_extraction_outcomes").expect("count"),
        0,
        "outcomes must not outlive their company"
    );
}

#[test]
fn migration_0105_preserves_outcome_rows_and_admits_witness_fallback() {
    // ADR 0085 amendment: migration 0105 rebuilds `fundamentals_extraction_outcomes`
    // to widen its `reason_code` CHECK with `witness_fallback`. A rebuild is the
    // one migration shape that can silently LOSE rows (copy, drop, rename), and
    // this table is a durable record — a lost row reads as "never attempted",
    // which is precisely the confusion ADR 0061 decision 2 exists to prevent.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 104).expect("apply schema through 0104");

    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed a company");
    connection
        .execute(
            "INSERT INTO fundamentals_extraction_outcomes
                (id, company_id, report_document_id, fiscal_year, period_type, period_end,
                 tier, acceptance, reason_code, detail_json, fact_count, attempt_count)
             VALUES ('fxo_1', 'c1', 'doc1', 2026, 'FY', '2026-12-31',
                 'pdf', 'flagged', 'witness_disagreement', '{\"a\":1}', 0, 3)",
            [],
        )
        .expect("seed a pre-0105 outcome");

    // The pre-0105 CHECK must reject the new code — otherwise this test would
    // pass vacuously and prove nothing about the widening.
    assert!(
        connection
            .execute(
                "INSERT INTO fundamentals_extraction_outcomes
                    (id, company_id, report_document_id, fiscal_year, period_type, period_end,
                     acceptance, reason_code)
                 VALUES ('fxo_pre', 'c1', 'doc9', 2026, 'FY', '2026-12-31',
                     'accepted_unreviewed', 'witness_fallback')",
                [],
            )
            .is_err(),
        "the pre-0105 CHECK should not admit witness_fallback"
    );

    apply_migrations(&mut connection).expect("apply migration 0105");

    // Every column of the pre-existing row survives the rebuild.
    let (tier, acceptance, reason, detail, attempts): (String, String, String, String, i64) =
        connection
            .query_row(
                "SELECT tier, acceptance, reason_code, detail_json, attempt_count
             FROM fundamentals_extraction_outcomes WHERE id = 'fxo_1'",
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
            .expect("the pre-0105 outcome row must survive the rebuild");
    assert_eq!(tier, "pdf");
    assert_eq!(acceptance, "flagged");
    assert_eq!(reason, "witness_disagreement");
    assert_eq!(detail, "{\"a\":1}");
    assert_eq!(attempts, 3);

    // The new code is now accepted...
    connection
        .execute(
            "INSERT INTO fundamentals_extraction_outcomes
                (id, company_id, report_document_id, fiscal_year, period_type, period_end,
                 tier, acceptance, reason_code)
             VALUES ('fxo_2', 'c1', 'doc2', 2026, 'FY', '2026-12-31',
                 'html_aggregator', 'accepted_unreviewed', 'witness_fallback')",
            [],
        )
        .expect("witness_fallback must be admitted after 0105");

    // ...and the vocabulary is still closed (the widening is not a loosening).
    assert!(
        connection
            .execute(
                "INSERT INTO fundamentals_extraction_outcomes
                    (id, company_id, report_document_id, fiscal_year, period_type, period_end,
                     acceptance, reason_code)
                 VALUES ('fxo_3', 'c1', 'doc3', 2026, 'FY', '2026-12-31',
                     'accepted_unreviewed', 'whatever_i_feel_like')",
                [],
            )
            .is_err(),
        "reason_code must stay a closed typed vocabulary"
    );

    // The slot uniqueness index survived the rename.
    assert!(
        connection
            .execute(
                "INSERT INTO fundamentals_extraction_outcomes
                    (id, company_id, report_document_id, fiscal_year, period_type, period_end,
                     acceptance, reason_code)
                 VALUES ('fxo_dup', 'c1', 'doc2', 2026, 'FY', '2026-12-31',
                     'accepted_unreviewed', 'emitted')",
                [],
            )
            .is_err(),
        "the slot unique index must be recreated against the rebuilt table"
    );
}

// ---------------------------------------------------------------------------
// Migration 0107 — targeted data repairs (cards 45fcece / 22ac70c / 40281b3).
// ---------------------------------------------------------------------------

/// Seeds the pre-0107 shape both repairs act on, plus the near-miss rows each
/// must spare. Returns nothing — the test keys on fixed ids.
fn seed_pre_0107_snapshot(connection: &rusqlite::Connection) {
    connection
        .execute_batch(
            "
            INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name) VALUES
                ('company_gpw_cbf', 'GPW', 'CBF', 'GPW:CBF', 'cyber_Folks S.A.'),
                ('company_gpw_vrc', 'GPW', 'VRC', 'GPW:VRC', 'VERCOM S.A.'),
                ('company_gpw_atr', 'GPW', 'ATR', 'GPW:ATR', 'ATREM S.A.'),
                ('company_gpw_asb', 'GPW', 'ASB', 'GPW:ASB', 'ASSECO BUSINESS SOLUTIONS S.A.');

            -- Part A documents. The four cyber_Folks espi attachments whose Bankier
            -- URL names Energa are mis-associations; the cyber_Folks doc without
            -- 'energa' in the URL, and the Vercom doc that happens to carry the
            -- reused Energa slug, are legitimate and must survive.
            INSERT INTO report_documents (id, company_id, source_type, url, doc_kind) VALUES
                ('rd_cbf_energa_ssf', 'company_gpw_cbf', 'espi_attachment',
                 'https://www.bankier.pl/att/1.-SSF-Grupy-Energa-9m.pdf', 'periodic_ssf'),
                ('rd_cbf_energa_jsf', 'company_gpw_cbf', 'espi_attachment',
                 'https://www.bankier.pl/att/2.-JSF-Energa-SA-9m.pdf', 'periodic_jsf'),
                ('rd_cbf_energa_info', 'company_gpw_cbf', 'espi_attachment',
                 'https://www.bankier.pl/att/3.-Informacja-Grupy-Energa.pdf', 'other'),
                ('rd_cbf_energa_wdf', 'company_gpw_cbf', 'espi_attachment',
                 'https://www.bankier.pl/att/4.-Dane-Grupy-Energa.pdf', 'other'),
                ('rd_cbf_vercom', 'company_gpw_cbf', 'espi_attachment',
                 'https://www.bankier.pl/att/Vercom-2024-Q2-SSF.pdf', 'periodic_ssf'),
                ('rd_vrc_energa', 'company_gpw_vrc', 'espi_attachment',
                 'https://www.bankier.pl/att/1.-SSF-Grupy-Energa-9m.pdf', 'periodic_ssf');

            INSERT INTO report_document_sections (report_document_id, ordinal, heading, body) VALUES
                ('rd_cbf_energa_ssf', 0, 'bilans', 'body'),
                ('rd_cbf_vercom', 0, 'bilans', 'body');

            -- Part B periods.
            INSERT INTO financial_periods (id, company_id, fiscal_year, period_type) VALUES
                ('p_atr_2023fy', 'company_gpw_atr', 2023, 'FY'),
                ('p_atr_2024fy', 'company_gpw_atr', 2024, 'FY'),
                ('p_atr_2024q1', 'company_gpw_atr', 2024, 'Q1'),
                ('p_atr_2025q1', 'company_gpw_atr', 2025, 'Q1'),
                ('p_atr_2022fy', 'company_gpw_atr', 2022, 'FY'),
                ('p_asb_2024q3', 'company_gpw_asb', 2024, 'Q3'),
                ('p_asb_2025q1', 'company_gpw_asb', 2025, 'Q1'),
                ('p_asb_2026q1', 'company_gpw_asb', 2026, 'Q1');
            ",
        )
        .expect("seed 0107 base rows");

    // Part B facts. Targets carry the note-ref-misparse signature (kpidef_cash,
    // auto_unreviewed, pdf provenance, whole-number multiple of 1000 <= 60000).
    // Near-misses each break exactly one predicate clause and must survive.
    let facts: [(&str, &str, &str, &str, &str, &str); 9] = [
        // id, period, metric, value, confirmation_state, source_tier
        (
            "f_atr_2023fy",
            "p_atr_2023fy",
            "cash",
            "19000",
            "auto_unreviewed",
            "pdf",
        ),
        (
            "f_atr_2024fy",
            "p_atr_2024fy",
            "cash",
            "17000",
            "auto_unreviewed",
            "pdf",
        ),
        (
            "f_atr_2024q1",
            "p_atr_2024q1",
            "cash",
            "17000",
            "auto_unreviewed",
            "pdf",
        ),
        (
            "f_asb_2024q3",
            "p_asb_2024q3",
            "cash",
            "26000",
            "auto_unreviewed",
            "pdf",
        ),
        (
            "f_asb_2025q1",
            "p_asb_2025q1",
            "cash",
            "26000",
            "auto_unreviewed",
            "pdf",
        ),
        // Survives: > 60 000 (a correctly-scaled small cash position).
        (
            "f_atr_2025q1",
            "p_atr_2025q1",
            "cash",
            "3841000",
            "auto_unreviewed",
            "pdf",
        ),
        // Survives: > 60 000 comma-format value (a separate, unfixed class).
        (
            "f_asb_2026q1",
            "p_asb_2026q1",
            "cash",
            "206506",
            "auto_unreviewed",
            "pdf",
        ),
        // Survives: user-reviewed (never auto_unreviewed), even at the signature
        // value. PDF-tier so it is not an ESEF anchor: were this the company's
        // only esef cash fact, migration 0108 would read it as the anchor and
        // (wrongly, for this fiction) flag the 3 841 000 survivor as 100×-high.
        // ATREM carries no esef cash in this fixture, so 0108 never anchors it.
        (
            "f_atr_conf",
            "p_atr_2022fy",
            "cash",
            "19000",
            "confirmed",
            "pdf",
        ),
        // Survives: a different metric at a signature-shaped value.
        (
            "f_atr_equity",
            "p_atr_2023fy",
            "total_equity",
            "35000",
            "auto_unreviewed",
            "pdf",
        ),
    ];
    for (id, period, metric, value, state, tier) in facts {
        connection
            .execute(
                "INSERT INTO financial_facts (id, company_id, period_id, definition_id, value_numeric, confirmation_state)
                 SELECT ?1, (SELECT company_id FROM financial_periods WHERE id = ?2), ?2, id, ?3, ?4
                 FROM kpi_definitions WHERE metric_key = ?5 LIMIT 1",
                rusqlite::params![id, period, value, state, metric],
            )
            .unwrap_or_else(|e| panic!("seed fact {id}: {e}"));
        connection
            .execute(
                "INSERT INTO financial_fact_provenance (fact_id, source_tier, validation_status, citation)
                 VALUES (?1, ?2, 'unreviewed', 'środki pieniężne')",
                rusqlite::params![id, tier],
            )
            .unwrap_or_else(|e| panic!("seed provenance {id}: {e}"));
    }

    // A superseding pointer and a manual claim both reference a target fact, so
    // the reference-detach steps are exercised (an enforced FK would otherwise
    // block the delete).
    connection
        .execute(
            "UPDATE financial_facts SET supersedes_id = 'f_atr_2023fy' WHERE id = 'f_atr_conf'",
            [],
        )
        .expect("link supersedes to a target");
    connection
        .execute(
            "INSERT INTO management_claims (id, company_id, statement, verifying_fact_id)
             VALUES ('claim_0107', 'company_gpw_atr', 'Cash is stable.', 'f_atr_2024fy')",
            [],
        )
        .expect("seed a claim verified by a target fact");
}

#[test]
fn migration_0107_repairs_misassociation_and_note_ref_facts_and_spares_the_rest() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 106).expect("apply schema through 0106");
    seed_pre_0107_snapshot(&connection);

    apply_migrations(&mut connection).expect("apply migration 0107");

    let doc_ids = |conn: &rusqlite::Connection| -> Vec<String> {
        conn.prepare("SELECT id FROM report_documents ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    let fact_ids = |conn: &rusqlite::Connection| -> Vec<String> {
        conn.prepare("SELECT id FROM financial_facts ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };

    // Part A: exactly the four cyber_Folks Energa attachments are gone.
    let docs = doc_ids(&connection);
    for gone in [
        "rd_cbf_energa_ssf",
        "rd_cbf_energa_jsf",
        "rd_cbf_energa_info",
        "rd_cbf_energa_wdf",
    ] {
        assert!(
            !docs.contains(&gone.to_string()),
            "{gone} must be deleted; have {docs:?}"
        );
    }
    // The legitimate cyber_Folks doc and the Vercom doc carrying the reused Energa
    // slug survive — a global `url LIKE '%energa%'` would have wrongly deleted vrc.
    assert!(
        docs.contains(&"rd_cbf_vercom".to_string()),
        "legit cyber_Folks doc must survive"
    );
    assert!(
        docs.contains(&"rd_vrc_energa".to_string()),
        "vercom's Energa-slug doc must survive"
    );

    // Sections of a deleted doc are gone; sections of a survivor remain.
    let sections: Vec<String> = connection
        .prepare(
            "SELECT report_document_id FROM report_document_sections ORDER BY report_document_id",
        )
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        sections,
        vec!["rd_cbf_vercom".to_string()],
        "only the survivor's sections remain"
    );

    // Part B: the five note-ref-misparsed cash facts are gone; the near-misses stay.
    let facts = fact_ids(&connection);
    for gone in [
        "f_atr_2023fy",
        "f_atr_2024fy",
        "f_atr_2024q1",
        "f_asb_2024q3",
        "f_asb_2025q1",
    ] {
        assert!(
            !facts.contains(&gone.to_string()),
            "{gone} must be deleted; have {facts:?}"
        );
    }
    for kept in ["f_atr_2025q1", "f_asb_2026q1", "f_atr_conf", "f_atr_equity"] {
        assert!(
            facts.contains(&kept.to_string()),
            "{kept} must survive; have {facts:?}"
        );
    }

    // Every surviving fact still has its provenance; no deleted fact left one behind.
    let orphan_provenance: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM financial_fact_provenance
             WHERE fact_id NOT IN (SELECT id FROM financial_facts)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(orphan_provenance, 0, "no provenance may outlive its fact");
    let survivor_provenance: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM financial_fact_provenance WHERE fact_id = 'f_atr_2025q1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(survivor_provenance, 1, "a survivor keeps its provenance");

    // References into deleted facts are detached, not dangling.
    let supersedes: Option<String> = connection
        .query_row(
            "SELECT supersedes_id FROM financial_facts WHERE id = 'f_atr_conf'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        supersedes, None,
        "the superseding pointer to a deleted fact is cleared"
    );
    let claim_ref: Option<String> = connection
        .query_row(
            "SELECT verifying_fact_id FROM management_claims WHERE id = 'claim_0107'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        claim_ref, None,
        "the claim's verification link to a deleted fact is cleared"
    );

    // Idempotent + self-healing: re-running changes nothing further.
    let docs_after = docs.len();
    let facts_after = facts.len();
    apply_migrations(&mut connection).expect("re-running migrations must be safe");
    assert_eq!(
        count_applied_migrations(&connection).expect("count applied"),
        expected_migration_count(),
        "re-run must not add or drop migration rows",
    );
    assert_eq!(
        doc_ids(&connection).len(),
        docs_after,
        "re-run must not touch documents"
    );
    assert_eq!(
        fact_ids(&connection).len(),
        facts_after,
        "re-run must not touch facts"
    );
}

// ---------------------------------------------------------------------------
// Migration 0108 — ESEF-anchored delete-for-refill of misscaled PDF facts.
// ---------------------------------------------------------------------------

/// Seeds the pre-0108 shape reproducing the real footprint: per company+metric
/// an ESEF anchor plus PDF facts that are grossly off it (both high and low
/// side), alongside the near-miss rows that MUST survive — a `confirmed`
/// trillion fact, a within-100× unconfirmed fact, and a company with no esef
/// anchor. Keys on fixed ids.
fn seed_pre_0108_snapshot(connection: &rusqlite::Connection) {
    connection
        .execute_batch(
            "
            INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name) VALUES
                ('company_gpw_pkn', 'GPW', 'PKN', 'GPW:PKN', 'ORLEN S.A.'),
                ('company_gpw_mbr', 'GPW', 'MBR', 'GPW:MBR', 'MIRBUD S.A.'),
                ('company_gpw_cdr', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.'),
                ('company_gpw_dig', 'GPW', 'DIG', 'GPW:DIG', 'DIGITREE GROUP S.A.'),
                ('company_gpw_ent', 'GPW', 'ENT', 'GPW:ENT', 'ENTER AIR S.A.');

            INSERT INTO financial_periods (id, company_id, fiscal_year, period_type) VALUES
                ('p_pkn_2024fy', 'company_gpw_pkn', 2024, 'FY'),
                ('p_pkn_2025q1', 'company_gpw_pkn', 2025, 'Q1'),
                ('p_pkn_2025h1', 'company_gpw_pkn', 2025, 'H1'),
                ('p_mbr_2023fy', 'company_gpw_mbr', 2023, 'FY'),
                ('p_mbr_2024q3', 'company_gpw_mbr', 2024, 'Q3'),
                ('p_mbr_2025q1', 'company_gpw_mbr', 2025, 'Q1'),
                ('p_cdr_2023fy', 'company_gpw_cdr', 2023, 'FY'),
                ('p_cdr_2024q1', 'company_gpw_cdr', 2024, 'Q1'),
                ('p_dig_2024fy', 'company_gpw_dig', 2024, 'FY'),
                ('p_dig_2025q1', 'company_gpw_dig', 2025, 'Q1'),
                ('p_ent_2025q1', 'company_gpw_ent', 2025, 'Q1');
            ",
        )
        .expect("seed 0108 base rows");

    // (id, period, metric, value, confirmation_state, source_tier)
    let facts: [(&str, &str, &str, &str, &str, &str); 11] = [
        // --- ESEF anchors (the trusted, true-scale witnesses) ---
        (
            "f_pkn_rev_esef",
            "p_pkn_2024fy",
            "revenue",
            "100000000000",
            "confirmed",
            "esef",
        ),
        (
            "f_mbr_cash_esef",
            "p_mbr_2023fy",
            "cash",
            "34000000",
            "confirmed",
            "esef",
        ),
        (
            "f_cdr_fcf_esef",
            "p_cdr_2023fy",
            "financing_cash_flow",
            "127000000",
            "confirmed",
            "esef",
        ),
        (
            "f_dig_ta_esef",
            "p_dig_2024fy",
            "total_assets",
            "364000000",
            "confirmed",
            "esef",
        ),
        // --- Targets: deleted (unconfirmed pdf, grossly off the anchor) ---
        // HIGH: a large-cap can be misscaled too — 50 T vs a 100 bn anchor (500×).
        (
            "f_pkn_rev_bad",
            "p_pkn_2025q1",
            "revenue",
            "50000000000000",
            "auto_unreviewed",
            "pdf",
        ),
        // HIGH: 54 T vs a 34 M anchor (~1.6e6×), the trillion misscale class.
        (
            "f_mbr_cash_bad",
            "p_mbr_2024q3",
            "cash",
            "54000000000000",
            "auto_unreviewed",
            "pdf",
        ),
        // LOW: −1.139 M vs a 127 M anchor (1.139 M × 100 ≤ 127 M).
        (
            "f_cdr_fcf_bad",
            "p_cdr_2024q1",
            "financing_cash_flow",
            "-1139000",
            "auto_unreviewed",
            "pdf",
        ),
        // --- Survivors ---
        // A within-100× unconfirmed pdf fact is plausible: 95 bn vs a 100 bn anchor.
        (
            "f_pkn_rev_good",
            "p_pkn_2025h1",
            "revenue",
            "95000000000",
            "auto_unreviewed",
            "pdf",
        ),
        // A plausible unconfirmed pdf cash near the anchor (33 M vs 34 M).
        (
            "f_mbr_cash_good",
            "p_mbr_2025q1",
            "cash",
            "33000000",
            "auto_unreviewed",
            "pdf",
        ),
        // A CONFIRMED trillion fact — off the anchor but off-limits, surfaced for
        // manual review, never silently deleted.
        (
            "f_dig_ta_conf",
            "p_dig_2025q1",
            "total_assets",
            "90000000000000",
            "confirmed",
            "pdf",
        ),
        // A company with NO esef fact for the metric has no anchor: untouched.
        (
            "f_ent_rev_orphan",
            "p_ent_2025q1",
            "revenue",
            "1000000",
            "auto_unreviewed",
            "pdf",
        ),
    ];
    for (id, period, metric, value, state, tier) in facts {
        connection
            .execute(
                "INSERT INTO financial_facts (id, company_id, period_id, definition_id, value_numeric, confirmation_state)
                 SELECT ?1, (SELECT company_id FROM financial_periods WHERE id = ?2), ?2, id, ?3, ?4
                 FROM kpi_definitions WHERE metric_key = ?5 LIMIT 1",
                rusqlite::params![id, period, value, state, metric],
            )
            .unwrap_or_else(|e| panic!("seed fact {id}: {e}"));
        connection
            .execute(
                "INSERT INTO financial_fact_provenance (fact_id, source_tier, validation_status)
                 VALUES (?1, ?2, 'unreviewed')",
                rusqlite::params![id, tier],
            )
            .unwrap_or_else(|e| panic!("seed provenance {id}: {e}"));
    }

    // Soft references into a target fact (mbr cash 54 T) so every detach step is
    // exercised: a superseding pointer, a manual claim, and an autopilot run's
    // produced-id list. Enforced FKs would otherwise block the delete.
    connection
        .execute(
            "UPDATE financial_facts SET supersedes_id = 'f_mbr_cash_bad' WHERE id = 'f_mbr_cash_good'",
            [],
        )
        .expect("link supersedes to a target");
    connection
        .execute(
            "INSERT INTO management_claims (id, company_id, statement, verifying_fact_id)
             VALUES ('claim_0108', 'company_gpw_mbr', 'Cash is stable.', 'f_mbr_cash_bad')",
            [],
        )
        .expect("seed a claim verified by a target fact");
    connection
        .execute(
            "INSERT INTO autopilot_run (id, company_id, report_document_id, mode, produced_fact_ids_json)
             VALUES ('run_0108', 'company_gpw_mbr', 'rd_0108', 'autopilot',
                     json_array('f_mbr_cash_bad', 'f_mbr_cash_good'))",
            [],
        )
        .expect("seed an autopilot run that produced a target fact");
}

#[test]
fn migration_0108_deletes_esef_anchored_misscaled_pdf_facts_and_spares_the_rest() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 107).expect("apply schema through 0107");
    seed_pre_0108_snapshot(&connection);

    apply_migrations(&mut connection).expect("apply migration 0108");

    let fact_ids = |conn: &rusqlite::Connection| -> Vec<String> {
        conn.prepare("SELECT id FROM financial_facts ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };

    let facts = fact_ids(&connection);
    // The three grossly-off unconfirmed pdf facts (two high, one low) are gone.
    for gone in ["f_pkn_rev_bad", "f_mbr_cash_bad", "f_cdr_fcf_bad"] {
        assert!(
            !facts.contains(&gone.to_string()),
            "{gone} must be deleted; have {facts:?}"
        );
    }
    // Every near-miss survives: the esef anchors, the within-100× unconfirmed
    // facts, the CONFIRMED trillion fact, and the anchorless company's fact.
    for kept in [
        "f_pkn_rev_esef",
        "f_mbr_cash_esef",
        "f_cdr_fcf_esef",
        "f_dig_ta_esef",
        "f_pkn_rev_good",
        "f_mbr_cash_good",
        "f_dig_ta_conf",
        "f_ent_rev_orphan",
    ] {
        assert!(
            facts.contains(&kept.to_string()),
            "{kept} must survive; have {facts:?}"
        );
    }

    // No provenance outlives its fact; a survivor keeps its provenance row.
    let orphan_provenance: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM financial_fact_provenance
             WHERE fact_id NOT IN (SELECT id FROM financial_facts)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(orphan_provenance, 0, "no provenance may outlive its fact");
    let survivor_provenance: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM financial_fact_provenance WHERE fact_id = 'f_pkn_rev_good'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(survivor_provenance, 1, "a survivor keeps its provenance");

    // References into the deleted fact are detached, not dangling.
    let supersedes: Option<String> = connection
        .query_row(
            "SELECT supersedes_id FROM financial_facts WHERE id = 'f_mbr_cash_good'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(supersedes, None, "the superseding pointer is cleared");
    let claim_ref: Option<String> = connection
        .query_row(
            "SELECT verifying_fact_id FROM management_claims WHERE id = 'claim_0108'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(claim_ref, None, "the claim's verification link is cleared");
    let produced: String = connection
        .query_row(
            "SELECT produced_fact_ids_json FROM autopilot_run WHERE id = 'run_0108'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        produced, "[\"f_mbr_cash_good\"]",
        "the deleted id is pruned from the run's produced list; the survivor stays"
    );

    // Idempotent + self-healing: re-running changes nothing further.
    let facts_after = facts.len();
    apply_migrations(&mut connection).expect("re-running migrations must be safe");
    assert_eq!(
        count_applied_migrations(&connection).expect("count applied"),
        expected_migration_count(),
        "re-run must not add or drop migration rows",
    );
    assert_eq!(
        fact_ids(&connection).len(),
        facts_after,
        "re-run must not touch facts"
    );
}

/// Review finding (2026-07-22): the dictionary maps parent-attributable rows to
/// `wdf_net_profit_parent`/`wdf_equity_parent`, but no migration ever seeded
/// those catalog definitions — every such fact silently dropped at
/// `NoDefinition` (the pull's noDefinition counter). 0111 seeds them.
#[test]
fn migration_0111_seeds_the_parent_attributable_kpi_definitions() {
    let connection = crate::storage::open_in_memory_database().expect("db");
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM kpi_definitions
             WHERE metric_key IN ('wdf_net_profit_parent', 'wdf_equity_parent')
               AND scope = 'canonical'",
            [],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!(
        count, 2,
        "both parent-attributable definitions must be seeded"
    );
}

/// Guardrail G1 (review 2026-07-22, finding-1 class B): EVERY metric key any
/// extraction mapper can emit must resolve to a seeded canonical catalog
/// definition — an unseeded key makes `record_structured_fact` silently drop the
/// fact at its defensive `NoDefinition` skip (the bug that ate every
/// parent-attributable WDF metric until migration 0111). The keys are scanned
/// FROM THE MAPPER SOURCES at test time, so a future mapper return or dictionary
/// entry reddens this test until its seed migration lands.
#[test]
fn every_emittable_metric_key_has_a_canonical_definition() {
    use std::collections::BTreeSet;

    // Literals in mapper sources that look like metric keys but are not.
    const NON_METRIC_LITERALS: &[&str] = &["skonsolidowany", "jednostkowy"];

    let mut keys: BTreeSet<String> = BTreeSet::new();

    // 1. The shared Polish label dictionary (html aggregator + positional tiers):
    //    tuple pairs ("label", "metric_key").
    let dictionary_src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/fundamentals/extraction/text_numbers.rs"
    ));
    let tuple_re =
        regex::Regex::new(r#"\(\s*\n?\s*"[^"]+",\s*\n?\s*"([a-z][a-z0-9_]{2,})",?\s*\n?\s*\)"#)
            .expect("tuple regex");
    for capture in tuple_re.captures_iter(dictionary_src) {
        keys.insert(capture[1].to_owned());
    }

    // 2. The WDF cover-note row mapper. `classify` returns `Option<&'static str>`,
    //    so every key it can emit is a string literal in ITS BODY — which is what
    //    this scans, rather than the `Some("literal")` shape it used to match.
    //    That shape was blind to `Some(if cond { "a" } else { "b" })`, and three
    //    keys shipped unseeded behind the blind spot (issue #309:
    //    `wdf_book_value_per_share`, its rozwodniona twin, and
    //    `wdf_ebitda_przed_odpisami_aktualizujacymi_netto`). A body scan is
    //    shape-independent, so a future `match` arm, `unwrap_or`, or lookup table
    //    is covered too. The literals in the body that are NOT keys are the label
    //    predicates' arguments and the row wording quoted in comments; both are
    //    stripped first, leaving exactly the returned vocabulary.
    let wdf_src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/fundamentals/extraction/espi_cover_note.rs"
    ));
    let classify_body = {
        let start = wdf_src
            .find("\nfn classify(")
            .expect("espi_cover_note must define `fn classify(` at top level");
        let rest = &wdf_src[start + 1..];
        let end = rest
            .find("\n}\n")
            .expect("`fn classify` body must end at a column-0 brace");
        &rest[..end]
    };
    let comment_re = regex::Regex::new(r"//[^\n]*").expect("comment regex");
    let predicate_re = regex::Regex::new(r#"(?:contains|starts_with|ends_with)\(\s*"[^"]*"\s*\)"#)
        .expect("predicate regex");
    let comparison_re = regex::Regex::new(r#"==\s*"[^"]*""#).expect("comparison regex");
    let body = comment_re.replace_all(classify_body, "");
    let body = predicate_re.replace_all(&body, "");
    let body = comparison_re.replace_all(&body, "");
    let literal_re = regex::Regex::new(r#""([a-z][a-z0-9_]{2,})""#).expect("literal regex");
    let mut wdf_key_count = 0usize;
    for capture in literal_re.captures_iter(&body) {
        keys.insert(capture[1].to_owned());
        wdf_key_count += 1;
    }
    // The mapper's vocabulary is ~40 keys; a scan that suddenly sees a handful
    // means `classify` was split or restructured and the body no longer holds the
    // returns — the scan must be re-pointed, not silently narrowed.
    assert!(
        wdf_key_count >= 30,
        "the cover-note body scan must find the mapper vocabulary; got only {wdf_key_count}"
    );

    // 3. The ESEF concept map: `"Concept" => "metric_key"` match arms.
    let esef_src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/fundamentals/extraction/esef.rs"
    ));
    let arm_re = regex::Regex::new(r#"=>\s*"([a-z][a-z0-9_]{2,})""#).expect("arm regex");
    for capture in arm_re.captures_iter(esef_src) {
        keys.insert(capture[1].to_owned());
    }

    for literal in NON_METRIC_LITERALS {
        keys.remove(*literal);
    }
    assert!(
        keys.len() >= 20,
        "the source scan must find the mapper vocabulary; got only {keys:?}"
    );

    let connection = crate::storage::open_in_memory_database().expect("db");
    let mut unseeded: Vec<String> = Vec::new();
    for key in &keys {
        let found: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM kpi_definitions WHERE metric_key = ?1 AND company_id IS NULL",
                [key],
                |row| row.get(0),
            )
            .expect("catalog query");
        if found == 0 {
            unseeded.push(key.clone());
        }
    }
    assert!(
        unseeded.is_empty(),
        "every emittable metric key needs a canonical seed migration, or its facts \
         silently drop at NoDefinition — unseeded: {unseeded:?}"
    );
}

#[test]
fn migration_0115_creates_fx_rates_and_seeds_nbp_adapter_idempotently() {
    // ADR 0089 dec. 2 / plan v0.61 §A1: migration 0115 adds the append-only
    // `fx_rates` table (NBP Table-A mids, upsert by (currency, date)) and seeds
    // the internal `nbp-fx` source adapter. `CREATE TABLE IF NOT EXISTS` + an
    // `ON CONFLICT DO UPDATE` seed, so re-running the runner is a safe no-op that
    // neither errors nor duplicates — and it never disturbs stored FX rows.
    let mut connection = open_in_memory_database().expect("database should initialize");

    // The table exists and accepts a decimal-exact mid keyed by (currency, date).
    connection
        .execute(
            "INSERT INTO fx_rates (currency, date, mid_rate, source_adapter_id)
             VALUES ('EUR', '2024-01-15', '4.3748', 'nbp-fx')",
            [],
        )
        .expect("an fx_rate row must be storable");
    // The PK rejects a duplicate key on a plain insert (append-only by key).
    assert!(
        connection
            .execute(
                "INSERT INTO fx_rates (currency, date, mid_rate) VALUES ('EUR', '2024-01-15', '9.9')",
                [],
            )
            .is_err(),
        "the (currency, date) primary key must reject a duplicate key"
    );

    // The nbp-fx adapter row is seeded exactly once.
    let nbp_before: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM source_adapters WHERE id = 'nbp-fx'",
            [],
            |row| row.get(0),
        )
        .expect("count nbp-fx");
    assert_eq!(nbp_before, 1, "nbp-fx must be seeded once");

    // Re-running the runner is a safe no-op on the table and the seed.
    apply_migrations(&mut connection).expect("re-running migrations should be safe");

    let nbp_after: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM source_adapters WHERE id = 'nbp-fx'",
            [],
            |row| row.get(0),
        )
        .expect("count nbp-fx after");
    assert_eq!(nbp_after, 1, "nbp-fx re-seed must not duplicate");
    assert_eq!(
        count_rows(&connection, "fx_rates").expect("count"),
        1,
        "re-running migrations must not disturb stored FX rates"
    );
}

#[test]
fn migration_0116_creates_valuation_runs_idempotently() {
    // ADR 0089 dec. 5 / plan §B2: migration 0116 adds the append-only
    // `valuation_runs` table (IF NOT EXISTS + IF NOT EXISTS indexes), so
    // re-running the runner is a safe no-op that neither errors nor disturbs data.
    let mut connection = open_in_memory_database().expect("database should initialize");

    connection
        .execute(
            "INSERT INTO valuation_runs
                (id, company_id, method, inputs_json, fair_low, fair_base, fair_high,
                 data_as_of, confidence_grade)
             VALUES ('v1', 'c1', 'pe_multiple', '{\"a\":1}', '19', '22', '25',
                     '2026-07-27', 'B')",
            [],
        )
        .expect("a valuation run row must be storable");

    // The newest-run index exists (list ordering + signature lookup).
    let index_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master
              WHERE type = 'index' AND name = 'idx_valuation_runs_company_as_of')",
            [],
            |row| row.get(0),
        )
        .expect("index existence check");
    assert!(index_exists, "the company/as_of index must be created");

    // Re-running the runner is a safe no-op on the new table.
    apply_migrations(&mut connection).expect("re-running migrations should be safe");
    assert_eq!(
        count_rows(&connection, "valuation_runs").expect("count"),
        1,
        "re-running migrations must not disturb stored valuation runs"
    );
    assert_eq!(
        count_applied_migrations(&connection).expect("count applied"),
        expected_migration_count(),
        "re-run must reach exactly the expected migration count",
    );
}

/// Issue #243 (epic #40 S5 residue): migration 0119 rebuilds
/// `fundamentals_extraction_outcomes` to admit `facts_superseded`, recounts
/// `fact_count` from the facts actually AT the slot for the pre-S5 zero-effect
/// rows, and renames the still-zero survivors' claimed emission to
/// `facts_superseded`. A rebuild can silently lose rows, so row survival is
/// asserted alongside the repair semantics.
#[test]
fn migration_0119_recounts_zero_effect_rows_and_names_superseded() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 118).expect("apply schema through 0118");

    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed a company");
    seed_period(&connection, "p1", "c1", 2025, "FY");
    // Two facts AT the doc1 slot (distinct definitions satisfy the fact slot
    // unique index) — the recount source for the lying row below.
    for (id, def) in [("f1", "kpidef_net_profit"), ("f2", "kpidef_total_assets")] {
        connection
            .execute(
                "INSERT INTO financial_facts
                    (id, company_id, period_id, definition_id, value_numeric, source_document_ref)
                 VALUES (?1, 'c1', 'p1', ?2, '100', 'doc1')",
                params![id, def],
            )
            .expect("seed a fact at the doc1 slot");
    }

    let outcome = |id: &str, doc: &str, acceptance: &str, reason: &str, fact_count: i64| {
        connection
            .execute(
                "INSERT INTO fundamentals_extraction_outcomes
                    (id, company_id, report_document_id, fiscal_year, period_type, period_end,
                     tier, acceptance, reason_code, fact_count)
                 VALUES (?1, 'c1', ?2, 2025, 'FY', '2025-12-31', 'esef', ?3, ?4, ?5)",
                params![id, doc, acceptance, reason, fact_count],
            )
            .expect("seed a pre-0119 outcome");
    };
    // The pre-S5 lying state: emitting acceptance, zero count, claims emission.
    outcome("fxo_recover", "doc1", "accepted", "emitted", 0); // facts still at the slot
    outcome("fxo_super", "doc2", "accepted_unreviewed", "emitted", 0); // facts gone
    outcome("fxo_legacy", "doc3", "accepted", "witness_fallback", 0); // retired gap-fill
                                                                      // Untouchable bystanders: a healthy count and a non-emitting acceptance
                                                                      // (doc5 — the slot index is unique per document).
    outcome("fxo_healthy", "doc5", "accepted", "emitted", 5);
    outcome("fxo_flagged", "doc4", "flagged", "validation_failed", 0);

    // The pre-0119 CHECK must reject the new code — otherwise this test would
    // pass vacuously and prove nothing about the widening.
    assert!(
        connection
            .execute(
                "INSERT INTO fundamentals_extraction_outcomes
                    (id, company_id, report_document_id, fiscal_year, period_type, period_end,
                     acceptance, reason_code)
                 VALUES ('fxo_pre', 'c1', 'doc9', 2025, 'FY', '2025-12-31',
                     'accepted', 'facts_superseded')",
                [],
            )
            .is_err(),
        "the pre-0119 CHECK should not admit facts_superseded"
    );

    apply_migrations(&mut connection).expect("apply migration 0119");

    fn row(connection: &Connection, id: &str) -> (String, i64) {
        connection
            .query_row(
                "SELECT reason_code, fact_count
                 FROM fundamentals_extraction_outcomes WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or_else(|error| panic!("outcome {id} must survive the rebuild: {error}"))
    }

    // Recovered: the two facts at the doc1 slot become the honest count.
    assert_eq!(row(&connection, "fxo_recover"), ("emitted".to_owned(), 2));
    // Still zero after the recount: the claim is renamed, acceptance history kept.
    assert_eq!(
        row(&connection, "fxo_super"),
        ("facts_superseded".to_owned(), 0)
    );
    assert_eq!(
        row(&connection, "fxo_legacy"),
        ("facts_superseded".to_owned(), 0)
    );
    // Bystanders untouched.
    assert_eq!(row(&connection, "fxo_healthy"), ("emitted".to_owned(), 5));
    assert_eq!(
        row(&connection, "fxo_flagged"),
        ("validation_failed".to_owned(), 0)
    );

    // The widened CHECK now admits the repair code.
    connection
        .execute(
            "INSERT INTO fundamentals_extraction_outcomes
                (id, company_id, report_document_id, fiscal_year, period_type, period_end,
                 acceptance, reason_code)
             VALUES ('fxo_post', 'c1', 'doc9', 2025, 'FY', '2025-12-31',
                 'accepted', 'facts_superseded')",
            [],
        )
        .expect("facts_superseded must be admitted after 0119");

    // Re-running the runner is a safe no-op (the healed rows match no predicate).
    apply_migrations(&mut connection).expect("re-running migrations should be safe");
    assert_eq!(row(&connection, "fxo_recover"), ("emitted".to_owned(), 2));
    assert_eq!(
        row(&connection, "fxo_super"),
        ("facts_superseded".to_owned(), 0)
    );
}

#[test]
fn migration_0122_adds_witness_columns_null_and_keeps_prior_provenance() {
    // Epic #229 T5: 0122 appends the positive-corroboration columns to
    // `financial_fact_provenance`. Append-only + tolerant-read guard — a row
    // stored before the columns must upgrade cleanly, keep every other value,
    // and read back NULL ("never corroborated", never "disagreed").
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 121).expect("apply schema through 0121");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed a company");
    seed_period(&connection, "p1", "c1", 2025, "FY");
    connection
        .execute(
            "INSERT INTO financial_facts (id, company_id, period_id, definition_id, value_numeric)
             VALUES ('f1', 'c1', 'p1', 'kpidef_net_profit', '100')",
            [],
        )
        .expect("seed a fact");
    connection
        .execute(
            "INSERT INTO financial_fact_provenance
                (fact_id, source_tier, validation_status, citation)
             VALUES ('f1', 'esef', 'passed', 'ifrs-full:ProfitLoss')",
            [],
        )
        .expect("seed a pre-0122 provenance row");

    apply_migrations(&mut connection).expect("upgrade to latest");
    assert_eq!(
        count_applied_migrations(&connection).expect("count applied"),
        expected_migration_count(),
        "upgrade should reach the latest migration",
    );

    let (tier, status, citation, witness_value, witness_page_url, corroborated_at): (
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = connection
        .query_row(
            "SELECT source_tier, validation_status, citation,
                    witness_value, witness_page_url, corroborated_at
             FROM financial_fact_provenance WHERE fact_id = 'f1'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .expect("the legacy provenance row survives the upgrade");
    assert_eq!(tier, "esef");
    assert_eq!(
        status, "passed",
        "the verdict is never rewritten by a column add"
    );
    assert_eq!(citation.as_deref(), Some("ifrs-full:ProfitLoss"));
    assert_eq!(witness_value, None, "no corroboration is invented");
    assert_eq!(witness_page_url, None);
    assert_eq!(corroborated_at, None);
}

#[test]
fn migration_0123_admits_value_divergence_and_loses_no_outcome_row() {
    // Epic #229 T5 (#192 residual): 0123 widens the outcome `reason_code` CHECK
    // with `value_divergence` through the 0119 table-rebuild shape. The rebuild
    // must lose nothing, and the pre-0123 CHECK must genuinely reject the code —
    // otherwise this test would pass vacuously.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 122).expect("apply schema through 0122");
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.')",
            [],
        )
        .expect("seed a company");
    connection
        .execute(
            "INSERT INTO fundamentals_extraction_outcomes
                (id, company_id, report_document_id, fiscal_year, period_type, period_end,
                 tier, acceptance, reason_code, detail_json, fact_count, attempt_count)
             VALUES ('fxo_keep', 'c1', 'doc1', 2025, 'FY', '2025-12-31',
                 'esef', 'accepted', 'emitted', '{\"a\":1}', 7, 3)",
            [],
        )
        .expect("seed a pre-0123 outcome");

    assert!(
        connection
            .execute(
                "INSERT INTO fundamentals_extraction_outcomes
                    (id, company_id, report_document_id, fiscal_year, period_type, period_end,
                     acceptance, reason_code)
                 VALUES ('fxo_pre', 'c1', 'doc2', 2025, 'FY', '2025-12-31',
                     'flagged', 'value_divergence')",
                [],
            )
            .is_err(),
        "the pre-0123 CHECK should not admit value_divergence"
    );

    apply_migrations(&mut connection).expect("apply migration 0123");

    let (tier, acceptance, reason, detail, facts, attempts): (
        Option<String>,
        String,
        String,
        Option<String>,
        i64,
        i64,
    ) = connection
        .query_row(
            "SELECT tier, acceptance, reason_code, detail_json, fact_count, attempt_count
             FROM fundamentals_extraction_outcomes WHERE id = 'fxo_keep'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .expect("the pre-0123 outcome survives the rebuild");
    assert_eq!(tier.as_deref(), Some("esef"));
    assert_eq!(acceptance, "accepted");
    assert_eq!(reason, "emitted");
    assert_eq!(detail.as_deref(), Some("{\"a\":1}"));
    assert_eq!((facts, attempts), (7, 3), "counters survive the rebuild");

    connection
        .execute(
            "INSERT INTO fundamentals_extraction_outcomes
                (id, company_id, report_document_id, fiscal_year, period_type, period_end,
                 acceptance, reason_code)
             VALUES ('fxo_post', 'c1', 'doc2', 2025, 'FY', '2025-12-31',
                 'flagged', 'value_divergence')",
            [],
        )
        .expect("value_divergence must be admitted after 0123");

    // The slot unique index survives the rebuild: the same slot upserts, never
    // duplicates.
    assert!(
        connection
            .execute(
                "INSERT INTO fundamentals_extraction_outcomes
                    (id, company_id, report_document_id, fiscal_year, period_type, period_end,
                     acceptance, reason_code)
                 VALUES ('fxo_dupe', 'c1', 'doc2', 2025, 'FY', '2025-12-31',
                     'flagged', 'value_divergence')",
                [],
            )
            .is_err(),
        "the slot unique index must survive the rebuild"
    );

    apply_migrations(&mut connection).expect("re-running migrations should be safe");
}

#[test]
fn migration_0124_heals_companies_missing_core_kpi_relevance() {
    // #203 residual (epic #229 T7): migration 0106 seeded the IFRS core set for
    // the companies that existed WHEN IT APPLIED. Two companies on the owner's
    // database were created afterwards and hold ZERO kpi_relevance rows, so
    // `expected_primary_metric_keys` returns None and the completeness check
    // never fires for them. 0124 re-runs 0106's statement, curated rows intact.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 123).expect("apply schema through 0123");

    let def_id = |conn: &rusqlite::Connection, key: &str| -> String {
        conn.query_row(
            "SELECT id FROM kpi_definitions WHERE metric_key = ?1 AND scope = 'canonical'",
            [key],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| panic!("seeded canonical definition for {key}"))
    };

    connection
        .execute_batch(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                VALUES ('gap', 'GPW', 'ALE', 'GPW:ALE', 'Allegro'),
                       ('curated', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.');",
        )
        .expect("seed the gap company and the curated one");

    // The curated company already holds a hand-ranked `revenue` row: the seed
    // must fill the OTHER four around it and leave this one exactly as it is.
    connection
        .execute(
            "INSERT INTO kpi_relevance (id, company_id, definition_id, status, source, rank)
             VALUES ('kpirel_user_curated_revenue', 'curated', ?1, 'retired', 'user', 'secondary')",
            [def_id(&connection, "revenue")],
        )
        .expect("seed the curated row");

    apply_migrations(&mut connection).expect("apply migration 0124");

    let core_keys = |conn: &rusqlite::Connection, company: &str| -> Vec<String> {
        let mut statement = conn
            .prepare(
                "SELECT d.metric_key
                 FROM kpi_relevance r
                 JOIN kpi_definitions d ON d.id = r.definition_id
                 WHERE r.company_id = ?1 AND r.source = 'core'
                 ORDER BY d.metric_key",
            )
            .expect("prepare");
        let rows = statement
            .query_map([company], |row| row.get::<_, String>(0))
            .expect("query");
        rows.collect::<Result<Vec<_>, _>>().expect("collect")
    };

    assert_eq!(
        core_keys(&connection, "gap"),
        vec![
            "net_profit".to_owned(),
            "operating_profit".to_owned(),
            "revenue".to_owned(),
            "total_assets".to_owned(),
            "total_equity".to_owned(),
        ],
        "the gap company gets the full 0106 core set"
    );

    assert_eq!(
        core_keys(&connection, "curated"),
        vec![
            "net_profit".to_owned(),
            "operating_profit".to_owned(),
            "total_assets".to_owned(),
            "total_equity".to_owned(),
        ],
        "the curated metric is filled around, never re-seeded"
    );

    let curated: (String, String, Option<String>) = connection
        .query_row(
            "SELECT status, source, rank FROM kpi_relevance WHERE id = 'kpirel_user_curated_revenue'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("the curated row must survive");
    assert_eq!(curated.0, "retired");
    assert_eq!(curated.1, "user");
    assert_eq!(curated.2.as_deref(), Some("secondary"));

    // Deterministic 0106-shaped ids, so re-applying converges.
    let seeded_id: String = connection
        .query_row(
            "SELECT id FROM kpi_relevance WHERE company_id = 'gap' AND source = 'core'
             AND definition_id = (SELECT id FROM kpi_definitions
                                  WHERE metric_key = 'revenue' AND scope = 'canonical')",
            [],
            |row| row.get(0),
        )
        .expect("seeded id");
    assert_eq!(seeded_id, "kpirel_core_gap_revenue");

    let before: i64 = connection
        .query_row("SELECT COUNT(*) FROM kpi_relevance", [], |row| row.get(0))
        .expect("count");
    apply_migrations(&mut connection).expect("re-running migrations should be safe");
    let after: i64 = connection
        .query_row("SELECT COUNT(*) FROM kpi_relevance", [], |row| row.get(0))
        .expect("count");
    assert_eq!(before, after, "the seed converges instead of accumulating");
}

#[test]
fn migration_0125_rekeys_scope_blind_definition_ids_and_repoints_every_reference() {
    // #149 (epic #229 T7): `kpi_definition_id` built the PRIMARY KEY from
    // `metric_key` alone, so a company/sector-scoped definition squatted on the
    // bare `kpidef_<key>` id that belongs to the canonical concept. 0125 moves
    // those rows onto scope-discriminated ids and re-points every FK'd child —
    // with `PRAGMA foreign_keys = ON`, which is how the runner runs.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("enforce foreign keys");
    apply_migrations_up_to(&mut connection, 123).expect("apply schema through 0123");

    connection
        .execute_batch(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                VALUES ('cdr', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.');
             INSERT INTO financial_periods (id, company_id, fiscal_year, period_type, period_end_date)
                VALUES ('per_fy', 'cdr', 2024, 'FY', '2024-12-31');
             -- The bug class: a company-scoped definition on the bare id, with a
             -- fact and a relevance row hanging off it.
             INSERT INTO kpi_definitions (id, scope, company_id, metric_key, label, value_kind)
                VALUES ('kpidef_backlog', 'company', 'cdr', 'backlog', 'Backlog', 'monetary');
             INSERT INTO kpi_relevance (id, company_id, definition_id, status, source, rank)
                VALUES ('kpirel_backlog', 'cdr', 'kpidef_backlog', 'active', 'user', 'primary');
             INSERT INTO financial_facts (id, company_id, period_id, definition_id, value_numeric)
                VALUES ('fact_backlog', 'cdr', 'per_fy', 'kpidef_backlog', '1234');",
        )
        .expect("seed the pre-fix state");

    let canonical_revenue: String = connection
        .query_row(
            "SELECT id FROM kpi_definitions WHERE metric_key = 'revenue' AND scope = 'canonical'",
            [],
            |row| row.get(0),
        )
        .expect("canonical revenue");

    apply_migrations(&mut connection).expect("apply migration 0125");

    // Re-keyed, with the scope discriminator; nothing was dropped.
    let rekeyed: (String, String, Option<String>) = connection
        .query_row(
            "SELECT id, scope, company_id FROM kpi_definitions WHERE metric_key = 'backlog'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("the company-scoped definition must survive the re-key");
    assert_eq!(rekeyed.0, "kpidef_backlog__c_cdr");
    assert_eq!(rekeyed.1, "company");
    assert_eq!(rekeyed.2.as_deref(), Some("cdr"));

    // Both children follow the parent — nothing was cascade-deleted.
    let relevance: String = connection
        .query_row(
            "SELECT definition_id FROM kpi_relevance WHERE id = 'kpirel_backlog'",
            [],
            |row| row.get(0),
        )
        .expect("the relevance row must survive and follow the parent");
    assert_eq!(relevance, "kpidef_backlog__c_cdr");
    let fact: String = connection
        .query_row(
            "SELECT definition_id FROM financial_facts WHERE id = 'fact_backlog'",
            [],
            |row| row.get(0),
        )
        .expect("the fact must survive and follow the parent");
    assert_eq!(fact, "kpidef_backlog__c_cdr");

    // The canonical rows and the curated sector packs (own id shape) are not touched.
    assert_eq!(canonical_revenue, "kpidef_revenue");
    let sector_pack: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM kpi_definitions WHERE id = 'kpidef_bank_nim'",
            [],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!(sector_pack, 1, "curated sector packs keep their ids");

    // The freed bare id is now available to the canonical concept.
    connection
        .execute(
            "INSERT OR IGNORE INTO kpi_definitions (id, scope, metric_key, label, value_kind)
             VALUES ('kpidef_backlog', 'canonical', 'backlog', 'Backlog', 'monetary')",
            [],
        )
        .expect("insert canonical backlog");
    let canonical_backlog: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM kpi_definitions WHERE id = 'kpidef_backlog' AND scope = 'canonical'",
            [],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!(
        canonical_backlog, 1,
        "a canonical seed for the key is no longer silently ignored"
    );

    apply_migrations(&mut connection).expect("re-running migrations should be safe");
}

#[test]
fn migration_0126_seeds_statement_packs_without_touching_the_core_floor() {
    // ADR 0092 layer 2 (#273): the scope='sector' packs have existed since 0034
    // and nothing ever read them. 0126 selects the conservative subset for each
    // company's statement_type, additively, on top of the core floor.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 125).expect("apply schema through 0125");

    connection
        .execute_batch(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name, statement_type)
                VALUES ('pko', 'GPW', 'PKO', 'GPW:PKO', 'PKO BP S.A.', 'banking'),
                       ('pzu', 'GPW', 'PZU', 'GPW:PZU', 'PZU S.A.', 'insurance'),
                       ('kru', 'GPW', 'KRU', 'GPW:KRU', 'KRUK S.A.', 'specialty_finance'),
                       ('cdr', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.', 'industrial');",
        )
        .expect("seed companies across every statement type");

    // A curated row on a key the banking pack would otherwise seed: automation
    // must fill around it, never re-source it.
    connection
        .execute(
            "INSERT INTO kpi_relevance (id, company_id, definition_id, status, source, rank)
             VALUES ('kpirel_user_nii', 'pko', 'kpidef_bank_net_interest_income',
                     'retired', 'user', 'secondary')",
            [],
        )
        .expect("seed the curated row");

    apply_migrations(&mut connection).expect("apply migration 0126");

    let pack_keys = |conn: &rusqlite::Connection, company: &str| -> Vec<String> {
        let mut statement = conn
            .prepare(
                "SELECT d.metric_key
                 FROM kpi_relevance r
                 JOIN kpi_definitions d ON d.id = r.definition_id
                 WHERE r.company_id = ?1 AND r.source = 'sector'
                 ORDER BY d.metric_key",
            )
            .expect("prepare");
        let rows = statement
            .query_map([company], |row| row.get::<_, String>(0))
            .expect("query");
        rows.collect::<Result<Vec<_>, _>>().expect("collect")
    };

    assert_eq!(
        pack_keys(&connection, "pko"),
        vec![
            "net_fee_commission_income".to_owned(),
            "total_deposits".to_owned(),
            "total_loans".to_owned(),
        ],
        "the conservative banking subset, minus the curated key it filled around"
    );
    assert_eq!(
        pack_keys(&connection, "pzu"),
        vec!["gross_insurance_revenue".to_owned()],
        "insurance seeds the IFRS 17 top line only"
    );
    assert_eq!(
        pack_keys(&connection, "kru"),
        vec![
            "cash_ebitda".to_owned(),
            "erc".to_owned(),
            "portfolio_purchases".to_owned(),
            "recoveries".to_owned(),
        ],
        "specialty_finance seeded NOTHING when 0126 shipped (0095 had brokers on \
         the same statement_type as debt collectors, so no pack key was \
         universal); migration 0127 split `brokerage` out and this test runs the \
         whole chain, so the freed debt-collection pack lands in full"
    );
    assert!(
        pack_keys(&connection, "cdr").is_empty(),
        "the industrial pack is scope='canonical'; there is no sector pack to add"
    );

    let curated: (String, String, Option<String>) = connection
        .query_row(
            "SELECT status, source, rank FROM kpi_relevance WHERE id = 'kpirel_user_nii'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("the curated row must survive");
    assert_eq!(curated.0, "retired");
    assert_eq!(curated.1, "user");
    assert_eq!(curated.2.as_deref(), Some("secondary"));

    // Deterministic ids, and re-applying converges instead of accumulating.
    let seeded_id: String = connection
        .query_row(
            "SELECT id FROM kpi_relevance
             WHERE company_id = 'pko' AND definition_id = 'kpidef_bank_total_loans'",
            [],
            |row| row.get(0),
        )
        .expect("seeded id");
    assert_eq!(seeded_id, "kpirel_sector_pko_total_loans");

    let before: i64 = connection
        .query_row("SELECT COUNT(*) FROM kpi_relevance", [], |row| row.get(0))
        .expect("count");
    apply_migrations(&mut connection).expect("re-running migrations should be safe");
    let after: i64 = connection
        .query_row("SELECT COUNT(*) FROM kpi_relevance", [], |row| row.get(0))
        .expect("count");
    assert_eq!(before, after, "the seed converges instead of accumulating");
}

#[test]
fn migration_0127_splits_brokers_out_of_specialty_finance_without_touching_anything_else() {
    // Owner decision 2026-07-31: `specialty_finance` conflated debt collectors
    // (KRU) with exchanges and brokerage houses (GPW, XTB) because migration
    // 0095 mapped them onto one type. The KPI pack is debt-collector
    // vocabulary, so the brokers move to their own `brokerage` type — which
    // frees `specialty_finance` to seed its whole pack (0126's allow-list grew
    // with it).
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 126).expect("apply schema through 0126");

    connection
        .execute_batch(
            "INSERT INTO companies
                (id, exchange, ticker, qualified_ticker, display_name, sector, statement_type)
             VALUES
                -- the 0095-mapped brokers: reclassified
                ('xtb', 'GPW', 'XTB', 'GPW:XTB', 'XTB S.A.',
                    'giełdy i biura maklerskie', 'specialty_finance'),
                ('gpw', 'GPW', 'GPW', 'GPW:GPW', 'GPW S.A.',
                    'giełdy i biura maklerskie', 'specialty_finance'),
                -- the 0098-mapped debt collector: keeps specialty_finance
                ('kru', 'GPW', 'KRU', 'GPW:KRU', 'KRUK S.A.',
                    'Wierzytelności', 'specialty_finance'),
                -- a hand-set value on a broker sector: NOT the 0095 mapping, so
                -- the manual choice is authoritative and must survive
                ('manual', 'GPW', 'MAN', 'GPW:MAN', 'Manual Co.',
                    'giełdy i biura maklerskie', 'industrial'),
                -- a specialty_finance row whose sector is NOT in the broker
                -- list (unknown/NULL provenance): never guessed at
                ('unknown', 'GPW', 'UNK', 'GPW:UNK', 'Unknown Co.',
                    NULL, 'specialty_finance'),
                -- controls
                ('pko', 'GPW', 'PKO', 'GPW:PKO', 'PKO BP S.A.',
                    'banki komercyjne', 'banking'),
                ('cdr', 'GPW', 'CDR', 'GPW:CDR', 'CD PROJEKT S.A.', 'Gry', 'industrial');",
        )
        .expect("seed the pre-split state");

    let before: i64 = connection
        .query_row("SELECT COUNT(*) FROM companies", [], |row| row.get(0))
        .expect("count");

    apply_migrations(&mut connection).expect("apply migration 0127");

    let statement_type = |conn: &rusqlite::Connection, id: &str| -> String {
        conn.query_row(
            "SELECT statement_type FROM companies WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .expect("statement type")
    };

    assert_eq!(statement_type(&connection, "xtb"), "brokerage");
    assert_eq!(statement_type(&connection, "gpw"), "brokerage");
    assert_eq!(
        statement_type(&connection, "kru"),
        "specialty_finance",
        "the debt collector keeps the type its KPI pack describes"
    );
    assert_eq!(
        statement_type(&connection, "manual"),
        "industrial",
        "a value that is not the 0095 mapping is the owner's, never rewritten"
    );
    assert_eq!(
        statement_type(&connection, "unknown"),
        "specialty_finance",
        "a specialty_finance row with no broker sector is never guessed at"
    );
    assert_eq!(statement_type(&connection, "pko"), "banking");
    assert_eq!(statement_type(&connection, "cdr"), "industrial");

    assert_eq!(
        before,
        connection
            .query_row("SELECT COUNT(*) FROM companies", [], |row| row
                .get::<_, i64>(0))
            .expect("count"),
        "a reclassification loses no company row"
    );

    // The freed `specialty_finance` type now seeds its whole pack for KRU, and
    // the brokers get nothing (there is no `brokerage` pack).
    let pack_keys = |conn: &rusqlite::Connection, company: &str| -> Vec<String> {
        let mut statement = conn
            .prepare(
                "SELECT d.metric_key
                 FROM kpi_relevance r
                 JOIN kpi_definitions d ON d.id = r.definition_id
                 WHERE r.company_id = ?1 AND r.source = 'sector'
                 ORDER BY d.metric_key",
            )
            .expect("prepare");
        let rows = statement
            .query_map([company], |row| row.get::<_, String>(0))
            .expect("query");
        rows.collect::<Result<Vec<_>, _>>().expect("collect")
    };
    assert_eq!(
        pack_keys(&connection, "kru"),
        vec![
            "cash_ebitda".to_owned(),
            "erc".to_owned(),
            "portfolio_purchases".to_owned(),
            "recoveries".to_owned(),
        ],
        "the whole debt-collection pack, now that the type means only that"
    );
    assert!(
        pack_keys(&connection, "xtb").is_empty() && pack_keys(&connection, "gpw").is_empty(),
        "no `brokerage` sector pack exists, so a broker keeps the core floor only"
    );

    let before_rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM kpi_relevance", [], |row| row.get(0))
        .expect("count");
    apply_migrations(&mut connection).expect("re-running migrations should be safe");
    let after_rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM kpi_relevance", [], |row| row.get(0))
        .expect("count");
    assert_eq!(before_rows, after_rows, "the split converges");
    assert_eq!(statement_type(&connection, "xtb"), "brokerage");
}

#[test]
fn migration_0128_repairs_only_the_peo_wdf_bank_misclassified_facts() {
    // Epic #277 T2, card #279: the pre-fix `espi_cover_note` classifier's bare
    // "zobowiązania" stem greedily mapped Bank Pekao's interbank-liabilities
    // row onto `total_liabilities` (~40x understated — the real total has no
    // "razem" row in this bank note at all), and the "przypisan" equity
    // trigger mapped the non-controlling-interests row onto
    // `wdf_equity_parent` — the opposite of parent equity. This forward
    // repair deletes ONLY those two exact wrong values, at PEO's H1/2026
    // espi_cover_note slot, and nothing else.
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 127).expect("apply schema through 0127");

    let def_id = |conn: &rusqlite::Connection, key: &str| -> String {
        conn.query_row(
            "SELECT id FROM kpi_definitions WHERE metric_key = ?1 AND scope = 'canonical'",
            [key],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| panic!("seeded canonical definition for {key}"))
    };
    let def_liab = def_id(&connection, "total_liabilities");
    let def_eq = def_id(&connection, "wdf_equity_parent");

    connection
        .execute_batch(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                VALUES ('peo', 'GPW', 'PEO', 'GPW:PEO', 'BANK POLSKA KASA OPIEKI S.A.'),
                       ('pko', 'GPW', 'PKO', 'GPW:PKO', 'PKO BP S.A.');
             INSERT INTO financial_periods (id, company_id, fiscal_year, period_type, period_end_date)
                VALUES ('per_peo_2026_h1', 'peo', 2026, 'H1', '2026-06-30'),
                       ('per_peo_2025_fy', 'peo', 2025, 'FY', '2025-12-31'),
                       ('per_pko_2026_h1', 'pko', 2026, 'H1', '2026-06-30');",
        )
        .expect("seed companies + periods");

    connection
        .execute(
            "INSERT INTO financial_facts
                (id, company_id, period_id, definition_id, value_numeric, extraction_method,
                 statement_basis, source_document_ref)
             VALUES
                -- The two live-wrong facts (the exact PEO H1/2026 values).
                ('f_wrong_liab', 'peo', 'per_peo_2026_h1', ?1, '7899000000', 'espi_cover_note',
                 'consolidated', 'feed_bankier_company_komunikatyarticle9175261'),
                ('f_wrong_eq', 'peo', 'per_peo_2026_h1', ?2, '12000000', 'espi_cover_note',
                 'consolidated', 'feed_bankier_company_komunikatyarticle9175261'),
                -- Controls that must survive:
                -- same company/period/metric, DIFFERENT value + basis (a
                -- plausible re-extracted correct total_liabilities).
                ('f_ok_liab', 'peo', 'per_peo_2026_h1', ?1, '316900000000', 'espi_cover_note',
                 'standalone', 'feed_bankier_company_komunikatyarticle9175261'),
                -- same value + metric, DIFFERENT period.
                ('f_other_period', 'peo', 'per_peo_2025_fy', ?1, '7899000000', 'espi_cover_note',
                 'consolidated', 'feed_bankier_company_komunikatyarticle9175261'),
                -- same value + metric + period_type, DIFFERENT company.
                ('f_other_ticker', 'pko', 'per_pko_2026_h1', ?1, '7899000000', 'espi_cover_note',
                 'consolidated', 'feed_bankier_company_komunikatyarticle9175261'),
                -- same company/period/metric/value coincidence, but a MANUAL
                -- fact (a different basis so it does not collide with
                -- f_wrong_eq's slot) — the user's own entry, never touched by
                -- an automatic repair.
                ('f_manual', 'peo', 'per_peo_2026_h1', ?2, '12000000', 'manual',
                 'standalone', NULL)",
            rusqlite::params![def_liab, def_eq],
        )
        .expect("seed facts");

    connection
        .execute_batch(
            "INSERT INTO financial_fact_provenance (fact_id, source_tier, validation_status, citation)
                VALUES
                    ('f_wrong_liab', 'espi_cover_note', 'unreviewed',
                     'Zobowiązania wobec innych banków | feed_item:feed_bankier_company_komunikatyarticle9175261'),
                    ('f_wrong_eq', 'espi_cover_note', 'unreviewed',
                     'Kapitał własny przypisany udziałom niedającym kontroli | feed_item:feed_bankier_company_komunikatyarticle9175261'),
                    ('f_ok_liab', 'espi_cover_note', 'unreviewed', 'Zobowiązania razem'),
                    ('f_other_period', 'espi_cover_note', 'unreviewed', 'Zobowiązania wobec innych banków'),
                    ('f_other_ticker', 'espi_cover_note', 'unreviewed', 'Zobowiązania wobec innych banków');",
        )
        .expect("seed provenance");

    let before: i64 = connection
        .query_row("SELECT COUNT(*) FROM financial_facts", [], |row| row.get(0))
        .expect("count");

    apply_migrations(&mut connection).expect("apply migration 0128");

    let fact_exists = |conn: &rusqlite::Connection, id: &str| -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM financial_facts WHERE id = ?1",
            [id],
            |row| row.get::<_, i64>(0),
        )
        .expect("count")
            > 0
    };
    let provenance_exists = |conn: &rusqlite::Connection, id: &str| -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM financial_fact_provenance WHERE fact_id = ?1",
            [id],
            |row| row.get::<_, i64>(0),
        )
        .expect("count")
            > 0
    };

    assert!(
        !fact_exists(&connection, "f_wrong_liab"),
        "the wrong total_liabilities fact is deleted"
    );
    assert!(
        !fact_exists(&connection, "f_wrong_eq"),
        "the wrong wdf_equity_parent fact is deleted"
    );
    assert!(
        !provenance_exists(&connection, "f_wrong_liab"),
        "the orphaned provenance row is cleaned up too"
    );
    assert!(
        !provenance_exists(&connection, "f_wrong_eq"),
        "the orphaned provenance row is cleaned up too"
    );

    assert!(
        fact_exists(&connection, "f_ok_liab"),
        "a different value at a different basis is never touched"
    );
    assert!(
        fact_exists(&connection, "f_other_period"),
        "a fact in a different period is never matched"
    );
    assert!(
        fact_exists(&connection, "f_other_ticker"),
        "a fact for a different company is never matched"
    );
    assert!(
        fact_exists(&connection, "f_manual"),
        "a manual fact at the same numeric coincidence is never touched"
    );
    assert!(provenance_exists(&connection, "f_ok_liab"));
    assert!(provenance_exists(&connection, "f_other_period"));
    assert!(provenance_exists(&connection, "f_other_ticker"));

    assert_eq!(
        before - 2,
        connection
            .query_row("SELECT COUNT(*) FROM financial_facts", [], |row| row
                .get::<_, i64>(0))
            .expect("count"),
        "exactly the two wrong facts are removed"
    );

    // Idempotent: nothing left matches the wrong-value predicate.
    apply_migrations(&mut connection).expect("re-run must be safe");
    assert!(!fact_exists(&connection, "f_wrong_liab"));
    assert!(!fact_exists(&connection, "f_wrong_eq"));
    assert!(fact_exists(&connection, "f_ok_liab"));
}

/// Migration `0129` (ADR 0093 decision 4, epic #285 T9): `kpi_definitions`
/// gains an `origin` column (`seed | user | agent`), backfilled by id shape —
/// a bare id (`kpidef_<key>`, no `__<marker>_` discriminator) is
/// migration-seeded (canonical packs + hand-written sector packs like
/// `kpidef_bank_nim`); every runtime-created row (`company`-scope custom
/// KPIs, `user`-scope quality-framework metrics) carries the discriminator
/// suffix `kpi_definition_id` always appends for a non-canonical scope, and
/// is left at the column DEFAULT (`user`).
#[test]
fn migration_0129_backfills_seed_origin_by_bare_id_shape_only() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 128).expect("apply schema through 0128");

    // Sanity: the column does not exist yet pre-migration.
    let has_origin_column: bool = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('kpi_definitions') WHERE name = 'origin'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("pragma_table_info")
        > 0;
    assert!(!has_origin_column, "origin must not exist before 0129");

    connection
        .execute_batch(
            "INSERT INTO kpi_definitions (id, scope, metric_key, label, value_kind, computation)
                VALUES
                    -- A bare canonical-style id: the 0034-style seed shape.
                    ('kpidef_test_seed_metric', 'canonical', 'test_seed_metric', 'Seed Metric', 'monetary', 'reported'),
                    -- A bare hand-written sector-pack id (the 0034/0048 shape,
                    -- e.g. kpidef_bank_nim): no scope=canonical requirement.
                    ('kpidef_bank_test_nim', 'sector', 'bank_test_nim', 'Bank Test NIM', 'ratio', 'reported'),
                    -- A runtime company-scoped custom KPI (UI CustomKpiManager
                    -- shape, __c_<company> discriminator).
                    ('kpidef_broker_clients__c_xtb', 'company', 'broker_clients', 'Broker Clients', 'count', 'reported'),
                    -- A runtime user-scope quality-framework custom metric
                    -- (__user_ discriminator).
                    ('kpidef_custom_metric__user_', 'user', 'custom_metric', 'Custom Metric', 'ratio', 'reported');",
        )
        .expect("seed kpi_definitions rows across every id shape");

    apply_migrations(&mut connection).expect("apply migration 0129");

    let origin_of = |conn: &rusqlite::Connection, id: &str| -> String {
        conn.query_row(
            "SELECT origin FROM kpi_definitions WHERE id = ?1",
            [id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| panic!("row {id} should exist"))
    };

    assert_eq!(
        origin_of(&connection, "kpidef_test_seed_metric"),
        "seed",
        "a bare canonical-style id backfills to seed"
    );
    assert_eq!(
        origin_of(&connection, "kpidef_bank_test_nim"),
        "seed",
        "a bare hand-written sector-pack id backfills to seed"
    );
    assert_eq!(
        origin_of(&connection, "kpidef_broker_clients__c_xtb"),
        "user",
        "a runtime company-scoped custom KPI is left at the DEFAULT (never seed)"
    );
    assert_eq!(
        origin_of(&connection, "kpidef_custom_metric__user_"),
        "user",
        "a runtime user-scope custom metric is left at the DEFAULT (never seed)"
    );

    // A pre-existing real seeded canonical definition (0034) also backfills.
    assert_eq!(origin_of(&connection, "kpidef_net_profit"), "seed");

    // Idempotent re-run: no error, backfill is stable.
    apply_migrations(&mut connection).expect("re-run must be safe");
    assert_eq!(origin_of(&connection, "kpidef_test_seed_metric"), "seed");
}

/// Migration `0130` (card #307): `kpi_definitions` gains a `statement_group`
/// column (`income | balance | cash_flow | per_share | other`), backfilled by
/// metric_key for the canonical/sector catalog — guarded to bare ids only
/// (`id NOT GLOB '*__*'`, the 0129 shape) so a company/user/agent row sharing a
/// canonical metric_key (e.g. a company's own "revenue" concept) never
/// inherits the seeded classification.
#[test]
fn migration_0130_backfills_statement_group_by_metric_key_bare_ids_only() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("open in-memory database");
    apply_migrations_up_to(&mut connection, 129).expect("apply schema through 0129");

    let has_column: bool = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('kpi_definitions') WHERE name = 'statement_group'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("pragma_table_info")
        > 0;
    assert!(!has_column, "statement_group must not exist before 0130");

    connection
        .execute_batch(
            "INSERT INTO kpi_definitions (id, scope, company_id, metric_key, label, value_kind, computation)
                VALUES
                    -- A runtime company-scoped custom KPI that reuses the 'revenue' metric key
                    -- (ADR 0077 d.8 no-repaint rule): must NOT inherit 'income'.
                    ('kpidef_revenue__c_xtb', 'company', NULL, 'revenue', 'Revenue (as XTB defines it)', 'monetary', 'reported'),
                    -- A runtime user-scope custom metric: stays 'other' (DEFAULT).
                    ('kpidef_custom_metric__user_', 'user', NULL, 'custom_metric', 'Custom Metric', 'ratio', 'reported');",
        )
        .expect("seed kpi_definitions rows across the non-canonical id shapes");

    apply_migrations(&mut connection).expect("apply migration 0130");

    let group_of = |conn: &rusqlite::Connection, id: &str| -> String {
        conn.query_row(
            "SELECT statement_group FROM kpi_definitions WHERE id = ?1",
            [id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| panic!("row {id} should exist"))
    };

    // Real seeded canonical definitions (0034/0072/0089/0110/0112) backfill by
    // their metric_key, one per group.
    assert_eq!(group_of(&connection, "kpidef_revenue"), "income");
    assert_eq!(group_of(&connection, "kpidef_total_assets"), "balance");
    assert_eq!(
        group_of(&connection, "kpidef_operating_cash_flow"),
        "cash_flow"
    );
    assert_eq!(group_of(&connection, "kpidef_eps_basic"), "per_share");
    assert_eq!(
        group_of(&connection, "kpidef_roe"),
        "other",
        "a canonical ratio with no explicit mapping stays at the DEFAULT"
    );
    // A bare hand-written sector-pack id (banking): the 0034/0048 shape.
    assert_eq!(
        group_of(&connection, "kpidef_bank_net_interest_income"),
        "income"
    );
    assert_eq!(group_of(&connection, "kpidef_bank_total_loans"), "balance");
    // A wdf_ issuer-row key (0112).
    assert_eq!(
        group_of(&connection, "kpidef_wdf_net_cash_change"),
        "cash_flow"
    );

    assert_eq!(
        group_of(&connection, "kpidef_revenue__c_xtb"),
        "other",
        "a company-scoped row must never inherit the canonical classification, even sharing the metric_key"
    );
    assert_eq!(
        group_of(&connection, "kpidef_custom_metric__user_"),
        "other",
        "a runtime user-scope custom metric is left at the DEFAULT"
    );

    // Idempotent re-run: no error, backfill is stable.
    apply_migrations(&mut connection).expect("re-run must be safe");
    assert_eq!(group_of(&connection, "kpidef_revenue"), "income");
}
