//! E2E tests for `validate_kpi_ingest_run` against a real (in-memory) DB
//! (#361 test group 7). Unit coverage of the rule engine itself lives in
//! `fundamentals::kpi_manifest::tests`; the atom's own guards in
//! `storage::kpi_ingest_staging::tests`. These tests exercise the seam
//! between them: real staged rows, real resolver, real slot history.

use super::*;
use crate::storage::{
    open_in_memory_database, AppState, KpiIngestRunState, NewKpiIngestRun, NewStagedObservation,
};

fn seed_company_and_document(connection: &rusqlite::Connection) {
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'gpw', 'ABC', 'GPW:ABC', 'ABC SA')",
            [],
        )
        .expect("company");
    connection
        .execute(
            "INSERT INTO report_documents (id, company_id, source_type, url, fetch_status)
             VALUES ('doc1', 'c1', 'espi_attachment', 'https://x/doc1.pdf', 'fetched')",
            [],
        )
        .expect("document");
}

/// A staged observation that resolves cleanly against the seeded canonical
/// `revenue` definition (migration 0034) and carries no history -- an
/// honest `plausibility.abstained` (`unreviewed`), never `flagged`.
fn clean_observation() -> NewStagedObservation {
    NewStagedObservation {
        raw_label: "Przychody ze sprzedaży".to_owned(),
        raw_value: "1 234,5".to_owned(),
        currency: Some("PLN".to_owned()),
        normalized_value: Some("1234.5".to_owned()),
        unit_scale: Some("ones".to_owned()),
        measure_window: Some("flow".to_owned()),
        metric_key_candidate: Some("revenue".to_owned()),
        mapping_status: Some("mapped".to_owned()),
        citation_page: Some(3),
        ..Default::default()
    }
}

/// Drives a fresh run to `staged` with one [`clean_observation`], returning
/// `(state, run_id, revision)`.
fn staged_run() -> (AppState, String, i64) {
    staged_run_with_profile("company_characteristic@v1")
}

/// [`staged_run`], with the caller's registry profile — the mechanics
/// fixtures use `company_characteristic@v1` (empty union on a raw company,
/// completeness gate skipped); the pack-path tests pick a financial profile.
fn staged_run_with_profile(profile_version: &str) -> (AppState, String, i64) {
    let connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection);
    let state = AppState::new(connection);
    let store = state.kpi_ingest_runs();
    let run = store
        .create_run_if_absent(&NewKpiIngestRun {
            report_document_id: "doc1".to_owned(),
            company_id: "c1".to_owned(),
            period_id: None,
            profile_version: profile_version.to_owned(),
            scope: Some("standalone".to_owned()),
            data_quality: Some("final".to_owned()),
            period_fiscal_year: Some(2025),
            period_type: Some("FY".to_owned()),
        })
        .expect("create run");
    store.claim_next("agent-1", 3600).expect("claim");
    store
        .mark_source_captured(&run.id, "agent-1", "hash1")
        .expect("capture");
    store
        .begin_extracting(&run.id, "agent-1", "instr-1")
        .expect("begin extracting");
    let (revision, _) = state
        .kpi_ingest_staging()
        .stage_observations(
            &run.id,
            "agent-1",
            vec![clean_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("stage");
    (state, run.id, revision)
}

#[test]
fn ready_path_creates_an_attempt_and_freezes_the_run_hash() {
    let (state, run_id, revision) = staged_run();

    let result = validate_kpi_ingest_run(&state, &run_id).expect("validate");
    assert_eq!(result.outcome, "ready");
    assert_eq!(result.manifest.observations.len(), 1);
    assert_eq!(
        result.manifest.observations[0].validation_state,
        crate::fundamentals::kpi_manifest::ValidationState::Unreviewed,
        "no history -> an honest plausibility.abstained, never a silent pass"
    );

    let run = state
        .kpi_ingest_runs()
        .get_run(&run_id)
        .expect("get")
        .expect("some");
    assert_eq!(run.status, KpiIngestRunState::ReadyToCommit);
    assert_eq!(
        run.manifest_hash.as_deref(),
        Some(result.manifest_hash.as_str())
    );

    let attempt = state
        .kpi_ingest_runs()
        .get_validation_attempt(&run_id, revision, &result.manifest_hash)
        .expect("get attempt")
        .expect("attempt row must exist");
    assert_eq!(attempt.outcome, "ready");
    assert_eq!(attempt.attempt, 1);

    // The creation-time completeness snapshot is readable back on the run
    // (ADR 0099 dec. 6: stamped by create_run_if_absent, not by validation).
    let stamped = run.expected_kpis_json.expect("snapshot stamped");
    assert!(stamped.contains("\"source\":\"kpi_relevance+profile_pack\""));
    assert!(stamped.contains("\"packVersion\":\"company_characteristic@v1\""));
}

#[test]
fn failed_path_creates_an_attempt_and_leaves_the_run_hash_null() {
    let (state, run_id, revision) = staged_run();

    // Flip the staged observation to a state the resolver can't complete:
    // drop its currency (monetary + no currency -> unit.currency_missing,
    // flagged).
    let connection = state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "UPDATE kpi_staged_observations SET currency = NULL WHERE run_id = ?1",
            [&run_id],
        )
        .expect("tamper for a flagged fixture");
    drop(connection);

    let result = validate_kpi_ingest_run(&state, &run_id).expect("validate");
    assert_eq!(result.outcome, "failed");
    assert_eq!(
        result.manifest.observations[0].validation_state,
        crate::fundamentals::kpi_manifest::ValidationState::Flagged
    );

    let run = state
        .kpi_ingest_runs()
        .get_run(&run_id)
        .expect("get")
        .expect("some");
    assert_eq!(run.status, KpiIngestRunState::ValidationFailed);
    assert!(
        run.manifest_hash.is_none(),
        "failed never sets run.manifest_hash"
    );

    let attempt =
        state
            .kpi_ingest_runs()
            .get_validation_attempt(&run_id, revision, &result.manifest_hash);
    assert!(
        attempt.expect("query ok").is_none(),
        "get_validation_attempt only ever returns 'ready' attempts"
    );
    let attempts = state
        .kpi_ingest_runs()
        .list_validation_attempts(&run_id)
        .expect("attempts");
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].outcome, "failed");
    assert!(attempts[0].manifest_json.contains("unit.currency_missing"));
}

#[test]
fn refuses_a_run_that_is_not_staged() {
    let connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection);
    let state = AppState::new(connection);
    let store = state.kpi_ingest_runs();
    let run = store
        .create_run_if_absent(&NewKpiIngestRun {
            report_document_id: "doc1".to_owned(),
            company_id: "c1".to_owned(),
            period_id: None,
            profile_version: "company_characteristic@v1".to_owned(),
            scope: None,
            data_quality: None,
            period_fiscal_year: None,
            period_type: None,
        })
        .expect("create run");

    let error = validate_kpi_ingest_run(&state, &run.id).expect_err("still discovered");
    assert_eq!(
        error.code,
        crate::commands::error::CommandErrorCode::Conflict
    );
}

#[test]
fn unknown_run_is_not_found() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let error = validate_kpi_ingest_run(&state, "kpiing_missing").expect_err("unknown run");
    assert_eq!(
        error.code,
        crate::commands::error::CommandErrorCode::NotFound
    );
}

/// data-model.md § Kompatybilność: a `ready_to_commit` run whose (run,
/// revision, hash) has no attempt row predates migration 0139 -- possible
/// only via a raw-seeded row in tests (no production writer creates one
/// anymore). `get_validation_attempt` returns `None` for it (the typed-
/// refusal seam #362/#363 build on); the repair path is
/// `invalidate_manifest` -> re-validate, which creates a real attempt.
#[test]
fn legacy_ready_row_without_an_attempt_is_repaired_by_invalidate_and_revalidate() {
    let (state, run_id, revision) = staged_run();

    // Simulate the pre-0139 hash-only shape: freeze the run directly,
    // bypassing the atom (and therefore never inserting an attempt row).
    let connection = state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "UPDATE kpi_ingest_runs SET status = 'ready_to_commit', manifest_hash = 'legacy-hash', \
             lease_holder = NULL, lease_expires_at = NULL, last_heartbeat_at = NULL WHERE id = ?1",
            [&run_id],
        )
        .expect("simulate a pre-0139 ready run");
    drop(connection);

    assert!(
        state
            .kpi_ingest_runs()
            .get_validation_attempt(&run_id, revision, "legacy-hash")
            .expect("query ok")
            .is_none(),
        "no attempt row exists for the legacy hash"
    );

    state
        .kpi_ingest_runs()
        .invalidate_manifest(&run_id)
        .expect("invalidate back to staged");
    let result = validate_kpi_ingest_run(&state, &run_id).expect("re-validate");
    assert_eq!(result.outcome, "ready");
    let attempt = state
        .kpi_ingest_runs()
        .get_validation_attempt(&run_id, revision, &result.manifest_hash)
        .expect("query ok")
        .expect("a real attempt now exists");
    assert_eq!(attempt.attempt, 1, "the SAME revision's first REAL attempt");
}

/// ADR 0099 dec. 6 (#383): a financial profile's creation-time stamp makes
/// the completeness gate real — staging only `revenue` against the 5-key
/// industrial floor fails with the other four keys named, and the manifest
/// carries the non-null `packVersion`.
#[test]
fn a_financial_profile_demands_its_floor_via_the_creation_stamp() {
    let (state, run_id, _revision) = staged_run_with_profile("gpw_ifrs_annual@v1");

    let result = validate_kpi_ingest_run(&state, &run_id).expect("validate");
    assert_eq!(result.outcome, "failed");
    let expected = result
        .manifest
        .expected_kpis
        .as_ref()
        .expect("snapshot in manifest");
    assert_eq!(
        expected.pack_version.as_deref(),
        Some("gpw_ifrs_annual@v1"),
        "the manifest carries the creation-time packVersion"
    );
    let completeness = result
        .manifest
        .completeness
        .as_ref()
        .expect("completeness computed for a non-empty snapshot");
    let missing: Vec<&str> = completeness
        .missing
        .iter()
        .filter(|m| m.reason.is_none())
        .map(|m| m.metric_key.as_str())
        .collect();
    assert_eq!(
        missing,
        [
            "net_profit",
            "operating_profit",
            "total_assets",
            "total_equity"
        ],
        "every unstaged floor key is missing without reason"
    );
}

/// The legacy-NULL fallback: a raw row whose `expected_kpis_json` is NULL is
/// live-stamped by validation exactly as before #383 — source
/// `kpi_relevance`, `packVersion` null — and still reaches `ready`.
#[test]
fn a_legacy_null_row_falls_back_to_the_live_stamp() {
    let (state, run_id, _revision) = staged_run();
    let connection = state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "UPDATE kpi_ingest_runs SET expected_kpis_json = NULL WHERE id = ?1",
            [&run_id],
        )
        .expect("null out the creation stamp");
    drop(connection);

    let result = validate_kpi_ingest_run(&state, &run_id).expect("validate");
    assert_eq!(result.outcome, "ready");
    let run = state
        .kpi_ingest_runs()
        .get_run(&run_id)
        .expect("get")
        .expect("some");
    let stamped = run
        .expected_kpis_json
        .expect("live-stamped by the fallback");
    assert!(stamped.contains("\"source\":\"kpi_relevance\""));
    assert!(!stamped.contains("profile_pack"));
    assert!(stamped.contains("\"packVersion\":null"));
}
