//! Atomic manifest commit tests (#362) against a real (in-memory) DB. Every
//! `ready_to_commit` fixture is driven through the REAL lifecycle —
//! `create_run_if_absent` → `claim_next` → `mark_source_captured` →
//! `begin_extracting` → `stage_observations` → `validate_kpi_ingest_run`
//! (#361's real validator, never a seeded verdict) — so the manifest bytes
//! `commit_manifest` consumes are exactly what production would produce.
//! Negative fixtures then tamper the DB directly (the only way to reach a
//! defect a real writer can never produce — mirrors
//! `jobs::kpi_ingest_validation::tests`' own tamper idiom).

use super::*;
use crate::jobs::kpi_ingest_validation::validate_kpi_ingest_run;
use crate::storage::{
    open_in_memory_database, AppState, KpiIngestRunState, NewKpiIngestRun, NewStagedObservation,
    StorageError, StructuredFactCommit,
};
use rusqlite::{params, Connection};

fn seed_company_and_document(connection: &Connection, company: &str, doc: &str) {
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES (?1, 'gpw', ?1, ?2, ?3)",
            params![company, format!("GPW:{company}"), format!("{company} SA")],
        )
        .expect("company");
    connection
        .execute(
            "INSERT INTO report_documents (id, company_id, source_type, url, fetch_status)
             VALUES (?1, ?2, 'espi_attachment', ?3, 'fetched')",
            params![doc, company, format!("https://x/{doc}.pdf")],
        )
        .expect("document");
}

fn observation(metric_key: &str, value: &str) -> NewStagedObservation {
    // Balance-sheet ("stock") metrics need `point_in_time`, everything else
    // ("flow") — `period.window_kind_mismatch` flags the mismatch otherwise.
    let measure_window = if matches!(
        metric_key,
        "total_assets" | "total_equity" | "cash" | "net_debt" | "shares_outstanding"
    ) {
        "point_in_time"
    } else {
        "flow"
    };
    NewStagedObservation {
        raw_label: metric_key.to_owned(),
        raw_value: value.to_owned(),
        currency: Some("PLN".to_owned()),
        normalized_value: Some(value.to_owned()),
        unit_scale: Some("ones".to_owned()),
        measure_window: Some(measure_window.to_owned()),
        metric_key_candidate: Some(metric_key.to_owned()),
        mapping_status: Some("mapped".to_owned()),
        citation_page: Some(3),
        citation_quote: Some(format!("{metric_key} quote")),
        ..Default::default()
    }
}

fn create_run(
    state: &AppState,
    doc: &str,
    company: &str,
    scope: &str,
    period_id: Option<String>,
) -> crate::storage::KpiIngestRun {
    state
        .kpi_ingest_runs()
        .create_run_if_absent(&NewKpiIngestRun {
            report_document_id: doc.to_owned(),
            company_id: company.to_owned(),
            period_id,
            profile_version: "p1".to_owned(),
            scope: Some(scope.to_owned()),
            data_quality: Some("final".to_owned()),
            period_fiscal_year: Some(2025),
            period_type: Some("FY".to_owned()),
        })
        .expect("create run")
}

/// Drives a freshly-created run to `ready_to_commit` via the REAL #361
/// validator (never a seeded verdict) and returns `(revision, manifest_hash)`.
fn drive_to_ready(
    state: &AppState,
    run_id: &str,
    observations: Vec<NewStagedObservation>,
) -> (i64, String) {
    let store = state.kpi_ingest_runs();
    store.claim_next("agent-1", 3600).expect("claim");
    store
        .mark_source_captured(run_id, "agent-1", "hash1")
        .expect("capture");
    store
        .begin_extracting(run_id, "agent-1", "instr-1")
        .expect("begin extracting");
    let (revision, _) = state
        .kpi_ingest_staging()
        .stage_observations(run_id, "agent-1", observations)
        .expect("stage");
    let result = validate_kpi_ingest_run(state, run_id).expect("validate");
    assert_eq!(
        result.outcome, "ready",
        "fixture requires a ready manifest: run_diagnostics={:?} observations={:?}",
        result.manifest.run_diagnostics, result.manifest.observations
    );
    (revision, result.manifest_hash)
}

/// One company/doc/run driven all the way to `ready_to_commit`, `(None,
/// None)` period branch (no pinned period at validation time) — the common
/// case every positive test builds on.
struct ReadyFixture {
    state: AppState,
    run_id: String,
    revision: i64,
    manifest_hash: String,
}

fn ready_fixture(scope: &str, observations: Vec<NewStagedObservation>) -> ReadyFixture {
    let connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");
    let state = AppState::new(connection);
    let run = create_run(&state, "doc1", "c1", scope, None);
    let (revision, manifest_hash) = drive_to_ready(&state, &run.id, observations);
    ReadyFixture {
        state,
        run_id: run.id,
        revision,
        manifest_hash,
    }
}

// ---------------------------------------------------------------------
// Group 1: E2E happy path (real validation).
// ---------------------------------------------------------------------

#[test]
fn commit_writes_period_facts_provenance_and_finalizes_the_run() {
    let fixture = ready_fixture("standalone", vec![observation("revenue", "1000")]);
    let receipt = fixture
        .state
        .kpi_ingest_commit()
        .commit_manifest(&fixture.run_id, &fixture.manifest_hash, fixture.revision)
        .expect("commit");

    assert_eq!(receipt.run_id, fixture.run_id);
    assert_eq!(receipt.accepted_count, 1);
    assert_eq!(receipt.terminal_status, "complete");
    assert!(receipt.period_id.is_some());

    let run = fixture
        .state
        .kpi_ingest_runs()
        .get_run(&fixture.run_id)
        .expect("get")
        .expect("some");
    assert_eq!(run.status, KpiIngestRunState::Complete);
    assert_eq!(run.period_id, receipt.period_id);

    let connection = fixture.state.checkout_for_tests().expect("raw");
    let (value, statement_basis, data_quality, extraction_method, confirmation_state, period_id): (
        String,
        String,
        String,
        String,
        String,
        String,
    ) = connection
        .query_row(
            "SELECT value_numeric, statement_basis, data_quality, extraction_method, \
             confirmation_state, period_id FROM financial_facts",
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
        .expect("fact");
    assert_eq!(value, "1000");
    assert_eq!(
        statement_basis, "standalone",
        "regression: a standalone run's fact must land in the standalone slot, never the hardcoded-None default"
    );
    assert_eq!(data_quality, "final");
    assert_eq!(extraction_method, "mcp_agent");
    assert_eq!(confirmation_state, "confirmed");
    assert_eq!(Some(period_id), run.period_id);

    let (source_tier, validation_status): (String, String) = connection
        .query_row(
            "SELECT source_tier, validation_status FROM financial_fact_provenance",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("provenance");
    assert_eq!(source_tier, "agent");
    assert!(matches!(
        validation_status.as_str(),
        "passed" | "unreviewed"
    ));

    let outcomes: serde_json::Value = serde_json::from_str(&receipt.outcomes_json).expect("json");
    let entries = outcomes.as_array().expect("array");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["outcome"], "created");
    assert_eq!(entries[0]["metricKey"], "revenue");
    assert!(entries[0]["factId"].is_string());
}

// ---------------------------------------------------------------------
// Group 2: empty-period-hole rollback (seeded receipt -> step-7 UNIQUE
// failure -> zero facts/provenance/period, run untouched).
// ---------------------------------------------------------------------

#[test]
fn a_mid_transaction_failure_rolls_back_the_period_too() {
    let fixture = ready_fixture("standalone", vec![observation("revenue", "1000")]);

    let connection = fixture.state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "INSERT INTO kpi_ingest_commit_receipts
                (id, run_id, manifest_hash, manifest_revision, terminal_status, accepted_count, outcomes_json)
             VALUES ('kpircpt_seed', ?1, 'other-hash', 1, 'complete', 0, '[]')",
            [&fixture.run_id],
        )
        .expect("seed conflicting receipt");
    drop(connection);

    let before = fixture
        .state
        .kpi_ingest_runs()
        .get_run(&fixture.run_id)
        .expect("get")
        .expect("some");

    let error = fixture
        .state
        .kpi_ingest_commit()
        .commit_manifest(&fixture.run_id, &fixture.manifest_hash, fixture.revision)
        .expect_err("receipt UNIQUE violation must roll back everything");
    assert!(matches!(
        error,
        StorageError::CommitReceiptAlreadyRecorded { .. }
    ));

    let connection = fixture.state.checkout_for_tests().expect("raw");
    let facts: i64 = connection
        .query_row("SELECT COUNT(*) FROM financial_facts", [], |row| row.get(0))
        .expect("count");
    let provenance: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM financial_fact_provenance",
            [],
            |row| row.get(0),
        )
        .expect("count");
    let periods: i64 = connection
        .query_row("SELECT COUNT(*) FROM financial_periods", [], |row| {
            row.get(0)
        })
        .expect("count");
    assert_eq!(facts, 0, "the empty-period-hole regression: zero facts");
    assert_eq!(provenance, 0);
    assert_eq!(periods, 0, "the period row itself must roll back too");
    drop(connection);

    let after = fixture
        .state
        .kpi_ingest_runs()
        .get_run(&fixture.run_id)
        .expect("get")
        .expect("some");
    assert_eq!(after.status, KpiIngestRunState::ReadyToCommit);
    assert_eq!(after.manifest_hash, before.manifest_hash);
    assert_eq!(after.manifest_revision, before.manifest_revision);
    assert_eq!(after.period_id, None);
}

// ---------------------------------------------------------------------
// Group 3: freshness.
// ---------------------------------------------------------------------

#[test]
fn commit_refuses_a_stale_hash_or_revision() {
    let fixture = ready_fixture("standalone", vec![observation("revenue", "1000")]);

    let wrong_hash = fixture
        .state
        .kpi_ingest_commit()
        .commit_manifest(&fixture.run_id, "not-the-real-hash", fixture.revision)
        .expect_err("wrong hash");
    assert!(matches!(
        wrong_hash,
        StorageError::StaleManifestForCommit { .. }
    ));

    let wrong_revision = fixture
        .state
        .kpi_ingest_commit()
        .commit_manifest(
            &fixture.run_id,
            &fixture.manifest_hash,
            fixture.revision + 7,
        )
        .expect_err("wrong revision");
    assert!(matches!(
        wrong_revision,
        StorageError::StaleManifestForCommit { .. }
    ));

    let connection = fixture.state.checkout_for_tests().expect("raw");
    let facts: i64 = connection
        .query_row("SELECT COUNT(*) FROM financial_facts", [], |row| row.get(0))
        .expect("count");
    assert_eq!(facts, 0, "a refused commit writes nothing");
}

#[test]
fn commit_refuses_when_the_run_is_not_ready_to_commit() {
    let fixture = ready_fixture("standalone", vec![observation("revenue", "1000")]);
    let connection = fixture.state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "UPDATE kpi_ingest_runs SET status = 'complete' WHERE id = ?1",
            [&fixture.run_id],
        )
        .expect("tamper status");
    drop(connection);

    let error = fixture
        .state
        .kpi_ingest_commit()
        .commit_manifest(&fixture.run_id, &fixture.manifest_hash, fixture.revision)
        .expect_err("wrong status");
    assert!(matches!(error, StorageError::InvalidRunTransition { .. }));
}

#[test]
fn commit_refuses_a_lingering_lease() {
    let fixture = ready_fixture("standalone", vec![observation("revenue", "1000")]);
    let connection = fixture.state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "UPDATE kpi_ingest_runs SET lease_holder = 'zombie', \
             lease_expires_at = '2999-01-01T00:00:00.000Z', \
             last_heartbeat_at = '2026-01-01T00:00:00.000Z' WHERE id = ?1",
            [&fixture.run_id],
        )
        .expect("tamper lease");
    drop(connection);

    let error = fixture
        .state
        .kpi_ingest_commit()
        .commit_manifest(&fixture.run_id, &fixture.manifest_hash, fixture.revision)
        .expect_err("lease invariant violated");
    assert!(matches!(
        error,
        StorageError::RunLeaseInvariantViolation { .. }
    ));
}

// ---------------------------------------------------------------------
// Group 4: no validation attempt row for the run's current tuple.
// ---------------------------------------------------------------------

#[test]
fn commit_refuses_a_missing_validation_attempt() {
    let fixture = ready_fixture("standalone", vec![observation("revenue", "1000")]);
    let connection = fixture.state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "DELETE FROM kpi_ingest_validation_attempts WHERE run_id = ?1",
            [&fixture.run_id],
        )
        .expect("delete attempt");
    drop(connection);

    let error = fixture
        .state
        .kpi_ingest_commit()
        .commit_manifest(&fixture.run_id, &fixture.manifest_hash, fixture.revision)
        .expect_err("no attempt row for the run's current tuple");
    assert!(matches!(
        error,
        StorageError::MissingValidationAttempt { .. }
    ));
}

// ---------------------------------------------------------------------
// Group 5: corruption.
// ---------------------------------------------------------------------

#[test]
fn commit_refuses_unparseable_manifest_bytes() {
    let fixture = ready_fixture("standalone", vec![observation("revenue", "1000")]);
    let connection = fixture.state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "UPDATE kpi_ingest_validation_attempts SET manifest_json = 'not json' WHERE run_id = ?1",
            [&fixture.run_id],
        )
        .expect("tamper bytes");
    drop(connection);

    let error = fixture
        .state
        .kpi_ingest_commit()
        .commit_manifest(&fixture.run_id, &fixture.manifest_hash, fixture.revision)
        .expect_err("corrupt bytes");
    assert!(matches!(error, StorageError::CorruptStoredManifest { .. }));
}

#[test]
fn commit_refuses_an_unsupported_manifest_schema_version() {
    let fixture = ready_fixture("standalone", vec![observation("revenue", "1000")]);
    let connection = fixture.state.checkout_for_tests().expect("raw");
    let raw_json: String = connection
        .query_row(
            "SELECT manifest_json FROM kpi_ingest_validation_attempts WHERE run_id = ?1",
            [&fixture.run_id],
            |row| row.get(0),
        )
        .expect("read attempt");
    let mut value: serde_json::Value = serde_json::from_str(&raw_json).expect("parse");
    value["manifestSchemaVersion"] = serde_json::json!(999);
    connection
        .execute(
            "UPDATE kpi_ingest_validation_attempts SET manifest_json = ?1 WHERE run_id = ?2",
            params![serde_json::to_string(&value).unwrap(), fixture.run_id],
        )
        .expect("tamper version");
    drop(connection);

    let error = fixture
        .state
        .kpi_ingest_commit()
        .commit_manifest(&fixture.run_id, &fixture.manifest_hash, fixture.revision)
        .expect_err("unsupported version");
    assert!(matches!(
        error,
        StorageError::UnsupportedManifestVersion { .. }
    ));
}

#[test]
fn commit_refuses_manifest_bytes_whose_recomputed_hash_disagrees() {
    let fixture = ready_fixture("standalone", vec![observation("revenue", "1000")]);
    let connection = fixture.state.checkout_for_tests().expect("raw");
    let raw_json: String = connection
        .query_row(
            "SELECT manifest_json FROM kpi_ingest_validation_attempts WHERE run_id = ?1",
            [&fixture.run_id],
            |row| row.get(0),
        )
        .expect("read attempt");
    let mut value: serde_json::Value = serde_json::from_str(&raw_json).expect("parse");
    // Retarget the SAME (structurally valid) observation's normalizedValue —
    // the stored `manifest_hash` column keeps pointing at the OLD bytes, so
    // the re-derived hash of these edited bytes must disagree.
    value["observations"][0]["normalizedValue"] = serde_json::json!("999999");
    connection
        .execute(
            "UPDATE kpi_ingest_validation_attempts SET manifest_json = ?1 WHERE run_id = ?2",
            params![serde_json::to_string(&value).unwrap(), fixture.run_id],
        )
        .expect("tamper content");
    drop(connection);

    let error = fixture
        .state
        .kpi_ingest_commit()
        .commit_manifest(&fixture.run_id, &fixture.manifest_hash, fixture.revision)
        .expect_err("hash mismatch");
    assert!(matches!(error, StorageError::CorruptStoredManifest { .. }));
}

// ---------------------------------------------------------------------
// Group 6: partial vs. complete terminal status.
// ---------------------------------------------------------------------

#[test]
fn commit_derives_partial_from_an_honest_missing_reason() {
    let connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");
    connection
        .execute(
            "INSERT INTO kpi_relevance (id, company_id, definition_id, status, source, rank)
             VALUES ('kpirel1', 'c1', 'kpidef_net_profit', 'active', 'manual', 'primary')",
            [],
        )
        .expect("relevance");
    let state = AppState::new(connection);
    let run = create_run(&state, "doc1", "c1", "standalone", None);
    let raw = state.checkout_for_tests().expect("raw");
    raw.execute(
        "UPDATE kpi_ingest_runs SET missing_reasons_json = '{\"net_profit\":\"not disclosed this quarter\"}' WHERE id = ?1",
        [&run.id],
    )
    .expect("stamp missing reason");
    drop(raw);

    let (revision, manifest_hash) =
        drive_to_ready(&state, &run.id, vec![observation("revenue", "1000")]);
    let receipt = state
        .kpi_ingest_commit()
        .commit_manifest(&run.id, &manifest_hash, revision)
        .expect("commit");
    assert_eq!(receipt.terminal_status, "partial");
}

// ---------------------------------------------------------------------
// Group 7: precedence ladder applied inside the commit.
// ---------------------------------------------------------------------

#[test]
fn commit_applies_the_precedence_ladder_per_observation() {
    let connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");
    let state = AppState::new(connection);

    let period_id = state
        .kpi_extraction()
        .ensure_financial_period("c1", 2025, "FY", None, "doc1")
        .expect("period");

    // Both pre-existing slots below need `statement_basis='standalone'` to
    // land in the SAME slot the incoming (standalone-run) pinned write
    // targets — the public `record_structured_fact`/`record_aggregator_fact`
    // paths hardcode `statement_basis=None` (-> 'consolidated'), exactly the
    // bug #362 fixes ONLY for the pinned path, so raw SQL is the only way to
    // seed a pre-existing 'standalone' slot here.
    let manual_fact_id = "fact_manual_np".to_owned();
    let raw = state.checkout_for_tests().expect("raw");
    raw.execute(
        "INSERT INTO financial_facts
            (id, company_id, period_id, definition_id, value_numeric, currency,
             statement_basis, attribution, measure_window, data_quality,
             extraction_method, confirmation_state)
         VALUES (?1, 'c1', ?2, 'kpidef_net_profit', '100', 'PLN', \
                 'standalone', 'total', 'flow', 'final', 'manual', 'confirmed')",
        params![manual_fact_id, period_id],
    )
    .expect("seed manual net_profit (no provenance row -> untouchable)");

    raw.execute(
        "INSERT INTO financial_facts
            (id, company_id, period_id, definition_id, value_numeric, currency,
             statement_basis, attribution, measure_window, data_quality,
             extraction_method, confirmation_state)
         VALUES ('fact_agg_ta', 'c1', ?1, 'kpidef_total_assets', '200', 'PLN', \
                 'standalone', 'total', 'point_in_time', 'final', 'api', 'confirmed')",
        [&period_id],
    )
    .expect("seed aggregator total_assets fact");
    raw.execute(
        "INSERT INTO financial_fact_provenance (fact_id, source_tier, validation_status)
         VALUES ('fact_agg_ta', 'html_aggregator', 'unreviewed')",
        [],
    )
    .expect("seed aggregator total_assets provenance");
    drop(raw);

    // pre-existing agent-tier eps_basic fact at the value the commit will
    // re-observe -> reobserved target.
    let eps_def = state
        .kpi_extraction()
        .resolve_kpi_definition("c1", "eps_basic")
        .expect("resolve")
        .expect("eps_basic canonical");
    let raw = state.checkout_for_tests().expect("raw");
    kpi_extraction::record_pinned_fact(
        &raw,
        PinnedFactInput {
            run_id: "seed",
            company_id: "c1",
            period_id: &period_id,
            definition_id: &eps_def.definition_id,
            metric_key: "eps_basic",
            value_numeric: "3.5",
            currency: Some("PLN"),
            statement_basis: "standalone",
            attribution: "total",
            measure_window: Some("flow"),
            data_quality: "final",
            report_document_id: "doc1",
            validation_status: "unreviewed",
            citation: None,
        },
    )
    .expect("seed agent eps");
    drop(raw);

    let run = create_run(&state, "doc1", "c1", "standalone", None);
    let (revision, manifest_hash) = drive_to_ready(
        &state,
        &run.id,
        vec![
            observation("net_profit", "150"),
            observation("total_assets", "250"),
            observation("eps_basic", "3.5"),
            observation("revenue", "900"),
        ],
    );

    let receipt = state
        .kpi_ingest_commit()
        .commit_manifest(&run.id, &manifest_hash, revision)
        .expect("commit");

    let outcomes: serde_json::Value = serde_json::from_str(&receipt.outcomes_json).expect("json");
    let by_metric = |key: &str| -> serde_json::Value {
        outcomes
            .as_array()
            .expect("array")
            .iter()
            .find(|o| o["metricKey"] == key)
            .unwrap_or_else(|| panic!("missing outcome for {key}"))
            .clone()
    };
    assert_eq!(by_metric("net_profit")["outcome"], "divergent");
    assert_eq!(by_metric("net_profit")["factId"], serde_json::Value::Null);
    assert_eq!(
        by_metric("net_profit")["detail"]["existingFactId"],
        manual_fact_id
    );
    assert_eq!(by_metric("total_assets")["outcome"], "upgraded");
    assert_eq!(by_metric("eps_basic")["outcome"], "reobserved");
    assert_eq!(by_metric("revenue")["outcome"], "created");
    assert_eq!(
        receipt.accepted_count, 3,
        "divergent never counts as accepted"
    );

    let manual_value: String = state
        .checkout_for_tests()
        .expect("raw")
        .query_row(
            "SELECT value_numeric FROM financial_facts WHERE id = ?1",
            [&manual_fact_id],
            |row| row.get(0),
        )
        .expect("manual fact");
    assert_eq!(
        manual_value, "100",
        "the manual value must survive the divergence"
    );
}

// ---------------------------------------------------------------------
// Group 8: supersession stamp.
// ---------------------------------------------------------------------

#[test]
fn commit_stamps_supersedes_id_next_to_a_preliminary_sibling() {
    let connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");
    let state = AppState::new(connection);
    let period_id = state
        .kpi_extraction()
        .ensure_financial_period("c1", 2025, "FY", None, "doc1")
        .expect("period");
    let revenue_def = state
        .kpi_extraction()
        .resolve_kpi_definition("c1", "revenue")
        .expect("resolve")
        .expect("revenue canonical");
    let raw = state.checkout_for_tests().expect("raw");
    let preliminary = kpi_extraction::record_pinned_fact(
        &raw,
        PinnedFactInput {
            run_id: "seed",
            company_id: "c1",
            period_id: &period_id,
            definition_id: &revenue_def.definition_id,
            metric_key: "revenue",
            value_numeric: "800",
            currency: Some("PLN"),
            statement_basis: "standalone",
            attribution: "total",
            measure_window: Some("flow"),
            data_quality: "preliminary",
            report_document_id: "doc1",
            validation_status: "unreviewed",
            citation: None,
        },
    )
    .expect("seed preliminary");
    let preliminary_id = match preliminary {
        StructuredFactCommit::Created(id) => id,
        other => panic!("expected Created: {other:?}"),
    };
    drop(raw);

    let run = create_run(&state, "doc1", "c1", "standalone", None);
    let (revision, manifest_hash) =
        drive_to_ready(&state, &run.id, vec![observation("revenue", "820")]);
    let receipt = state
        .kpi_ingest_commit()
        .commit_manifest(&run.id, &manifest_hash, revision)
        .expect("commit");
    assert_eq!(receipt.accepted_count, 1);

    let connection = state.checkout_for_tests().expect("raw");
    let final_id: String = connection
        .query_row(
            "SELECT id FROM financial_facts WHERE data_quality = 'final'",
            [],
            |row| row.get(0),
        )
        .expect("final fact");
    let supersedes_id: Option<String> = connection
        .query_row(
            "SELECT supersedes_id FROM financial_facts WHERE id = ?1",
            [&final_id],
            |row| row.get(0),
        )
        .expect("supersedes");
    assert_eq!(supersedes_id, Some(preliminary_id));
}

// ---------------------------------------------------------------------
// Group 9: pinned definition.
// ---------------------------------------------------------------------

#[test]
fn commit_refuses_a_deleted_pinned_definition() {
    let fixture = ready_fixture("standalone", vec![observation("revenue", "1000")]);
    let connection = fixture.state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "DELETE FROM kpi_definitions WHERE metric_key = 'revenue'",
            [],
        )
        .expect("delete definition");
    drop(connection);

    let error = fixture
        .state
        .kpi_ingest_commit()
        .commit_manifest(&fixture.run_id, &fixture.manifest_hash, fixture.revision)
        .expect_err("pinned definition gone");
    assert!(matches!(
        error,
        StorageError::PinnedDefinitionMissing { .. }
    ));
    let connection = fixture.state.checkout_for_tests().expect("raw");
    let facts: i64 = connection
        .query_row("SELECT COUNT(*) FROM financial_facts", [], |row| row.get(0))
        .expect("count");
    assert_eq!(facts, 0);
}

#[test]
fn commit_refuses_a_metric_key_mismatched_pinned_definition() {
    let fixture = ready_fixture("standalone", vec![observation("revenue", "1000")]);
    let connection = fixture.state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "UPDATE kpi_definitions SET metric_key = 'not_revenue_anymore' WHERE metric_key = 'revenue'",
            [],
        )
        .expect("tamper metric_key");
    drop(connection);

    let error = fixture
        .state
        .kpi_ingest_commit()
        .commit_manifest(&fixture.run_id, &fixture.manifest_hash, fixture.revision)
        .expect_err("metric key mismatch");
    assert!(matches!(
        error,
        StorageError::PinnedDefinitionMissing { .. }
    ));
}

#[test]
fn commit_refuses_a_pinned_definition_rescoped_to_another_company() {
    let fixture = ready_fixture("standalone", vec![observation("revenue", "1000")]);
    let connection = fixture.state.checkout_for_tests().expect("raw");
    seed_company_and_document(&connection, "other-co", "doc-other");
    connection
        .execute(
            "UPDATE kpi_definitions SET scope = 'company', company_id = 'other-co' WHERE metric_key = 'revenue'",
            [],
        )
        .expect("rescope definition");
    drop(connection);

    let error = fixture
        .state
        .kpi_ingest_commit()
        .commit_manifest(&fixture.run_id, &fixture.manifest_hash, fixture.revision)
        .expect_err("ineligible for this company");
    assert!(matches!(
        error,
        StorageError::PinnedDefinitionMissing { .. }
    ));
}

#[test]
fn commit_rolls_back_earlier_facts_when_a_later_definition_vanishes() {
    let fixture = ready_fixture(
        "standalone",
        vec![
            observation("revenue", "1000"),
            observation("net_profit", "200"),
        ],
    );
    let connection = fixture.state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "DELETE FROM kpi_definitions WHERE metric_key = 'net_profit'",
            [],
        )
        .expect("delete second definition");
    drop(connection);

    let error = fixture
        .state
        .kpi_ingest_commit()
        .commit_manifest(&fixture.run_id, &fixture.manifest_hash, fixture.revision)
        .expect_err("second observation's definition is gone");
    assert!(matches!(
        error,
        StorageError::PinnedDefinitionMissing { .. }
    ));

    let connection = fixture.state.checkout_for_tests().expect("raw");
    let facts: i64 = connection
        .query_row("SELECT COUNT(*) FROM financial_facts", [], |row| row.get(0))
        .expect("count");
    let periods: i64 = connection
        .query_row("SELECT COUNT(*) FROM financial_periods", [], |row| {
            row.get(0)
        })
        .expect("count");
    assert_eq!(facts, 0, "the FIRST observation's fact must roll back too");
    assert_eq!(periods, 0);
    drop(connection);

    let run = fixture
        .state
        .kpi_ingest_runs()
        .get_run(&fixture.run_id)
        .expect("get")
        .expect("some");
    assert_eq!(run.status, KpiIngestRunState::ReadyToCommit);
}

// ---------------------------------------------------------------------
// Group 10: period — four match branches (sol F3 r2).
// ---------------------------------------------------------------------

#[test]
fn period_branch_some_some_matching_commits_with_one_shared_period_id() {
    let connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");
    let state = AppState::new(connection);
    let period_id = state
        .kpi_extraction()
        .ensure_financial_period("c1", 2025, "FY", None, "doc1")
        .expect("period");
    let run = create_run(&state, "doc1", "c1", "standalone", Some(period_id.clone()));
    let (revision, manifest_hash) =
        drive_to_ready(&state, &run.id, vec![observation("revenue", "1000")]);

    let receipt = state
        .kpi_ingest_commit()
        .commit_manifest(&run.id, &manifest_hash, revision)
        .expect("commit");
    assert_eq!(receipt.period_id, Some(period_id.clone()));

    let after = state
        .kpi_ingest_runs()
        .get_run(&run.id)
        .expect("get")
        .expect("some");
    assert_eq!(after.period_id, Some(period_id.clone()));

    let connection = state.checkout_for_tests().expect("raw");
    let fact_period_id: String = connection
        .query_row("SELECT period_id FROM financial_facts", [], |row| {
            row.get(0)
        })
        .expect("fact period");
    assert_eq!(fact_period_id, period_id);
}

#[test]
fn period_branch_some_none_the_pinned_period_was_deleted_conflicts() {
    let connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");
    let state = AppState::new(connection);
    let period_id = state
        .kpi_extraction()
        .ensure_financial_period("c1", 2025, "FY", None, "doc1")
        .expect("period");
    let run = create_run(&state, "doc1", "c1", "standalone", Some(period_id.clone()));
    let (revision, manifest_hash) =
        drive_to_ready(&state, &run.id, vec![observation("revenue", "1000")]);

    let connection = state.checkout_for_tests().expect("raw");
    connection
        .execute("DELETE FROM financial_periods WHERE id = ?1", [&period_id])
        .expect("delete period (FK ON DELETE SET NULL nulls the run's period_id)");
    let live_period_id: Option<String> = connection
        .query_row(
            "SELECT period_id FROM kpi_ingest_runs WHERE id = ?1",
            [&run.id],
            |row| row.get(0),
        )
        .expect("read");
    assert_eq!(live_period_id, None, "FK cascade precondition");
    drop(connection);

    let error = state
        .kpi_ingest_commit()
        .commit_manifest(&run.id, &manifest_hash, revision)
        .expect_err("manifest still pins the deleted period");
    assert!(matches!(error, StorageError::CommitPeriodConflict { .. }));
}

#[test]
fn period_branch_none_some_the_run_gained_a_period_conflicts() {
    let fixture = ready_fixture("standalone", vec![observation("revenue", "1000")]);
    let new_period_id = fixture
        .state
        .kpi_extraction()
        .ensure_financial_period("c1", 2025, "FY", None, "doc1")
        .expect("period");
    let connection = fixture.state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "UPDATE kpi_ingest_runs SET period_id = ?1 WHERE id = ?2",
            params![new_period_id, fixture.run_id],
        )
        .expect("simulate an out-of-band period attach");
    drop(connection);

    let error = fixture
        .state
        .kpi_ingest_commit()
        .commit_manifest(&fixture.run_id, &fixture.manifest_hash, fixture.revision)
        .expect_err("manifest was frozen with no period");
    assert!(matches!(error, StorageError::CommitPeriodConflict { .. }));
}

#[test]
fn period_branch_none_none_creates_and_attaches_one_shared_period_id() {
    // Exactly the E2E happy fixture's shape; re-asserted here as the
    // explicit fourth branch alongside its three siblings above.
    let fixture = ready_fixture("standalone", vec![observation("revenue", "1000")]);
    let receipt = fixture
        .state
        .kpi_ingest_commit()
        .commit_manifest(&fixture.run_id, &fixture.manifest_hash, fixture.revision)
        .expect("commit");
    let after = fixture
        .state
        .kpi_ingest_runs()
        .get_run(&fixture.run_id)
        .expect("get")
        .expect("some");
    assert_eq!(after.period_id, receipt.period_id);
    let connection = fixture.state.checkout_for_tests().expect("raw");
    let fact_period_id: String = connection
        .query_row("SELECT period_id FROM financial_facts", [], |row| {
            row.get(0)
        })
        .expect("fact period");
    assert_eq!(Some(fact_period_id), receipt.period_id);
}

#[test]
fn period_branch_some_some_mismatched_natural_key_conflicts() {
    let connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");
    let state = AppState::new(connection);
    let period_id = state
        .kpi_extraction()
        .ensure_financial_period("c1", 2025, "FY", None, "doc1")
        .expect("period");
    let run = create_run(&state, "doc1", "c1", "standalone", Some(period_id.clone()));
    let (revision, manifest_hash) =
        drive_to_ready(&state, &run.id, vec![observation("revenue", "1000")]);

    let connection = state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "UPDATE financial_periods SET fiscal_year = 2020 WHERE id = ?1",
            [&period_id],
        )
        .expect("drift the period row's natural key");
    drop(connection);

    let error = state
        .kpi_ingest_commit()
        .commit_manifest(&run.id, &manifest_hash, revision)
        .expect_err("period row no longer matches the manifest's fiscalYear");
    assert!(matches!(error, StorageError::CommitPeriodConflict { .. }));
}

// ---------------------------------------------------------------------
// Group 11: citation canonical JSON goldens.
// ---------------------------------------------------------------------

#[test]
fn citation_json_full_locator_is_canonical() {
    let citation = Citation {
        page: Some(3),
        table: Some("T1".to_owned()),
        row: Some("R2".to_owned()),
        quote: Some("hello world".to_owned()),
    };
    insta::assert_snapshot!("citation_full_locator", canonical_citation_json(&citation));
}

#[test]
fn citation_json_all_nulls_is_canonical() {
    let citation = Citation {
        page: None,
        table: None,
        row: None,
        quote: None,
    };
    insta::assert_snapshot!("citation_all_nulls", canonical_citation_json(&citation));
}

#[test]
fn citation_json_escapes_delimiters_and_newlines() {
    let citation = Citation {
        page: Some(1),
        table: Some("A | B".to_owned()),
        row: Some("R\"1\"".to_owned()),
        quote: Some("line one\nline two \"quoted\", with, commas".to_owned()),
    };
    insta::assert_snapshot!(
        "citation_delimiters_and_newlines",
        canonical_citation_json(&citation)
    );
}
