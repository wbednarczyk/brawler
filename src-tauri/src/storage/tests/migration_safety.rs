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
