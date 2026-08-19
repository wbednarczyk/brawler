use super::*;
use crate::storage::migrations::open_in_memory_database;
use rusqlite::Connection;

fn seed_company(connection: &Connection, id: &str) {
    connection
        .execute(
            &format!(
                "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                     VALUES ('{id}', 'gpw', '{id}', 'GPW:{id}', '{id} SA')"
            ),
            [],
        )
        .expect("company");
}

fn seed_document(connection: &Connection, id: &str, company_id: &str) {
    connection
        .execute(
            "INSERT INTO report_documents (id, company_id, source_type, url, fetch_status)
                 VALUES (?1, ?2, 'espi_attachment', ?3, 'fetched')",
            params![id, company_id, format!("https://x/{id}.pdf")],
        )
        .expect("document");
}

/// Seed a run directly at a given status with a complete period
/// descriptor (staging's minimum period requirement) — bypasses
/// `create_run_if_absent` so tests can start from any status. `scope`/
/// `data_quality` are fixed at `consolidated`/`final` — the SAME values
/// [`sealed_manifest_for`]/[`failed_sealed_manifest_for`] hardcode into
/// their manifests' run context, so the #361 finding-1b run-context
/// binding check (`apply_validation_outcome`) sees a consistent world by
/// default.
fn seed_run(connection: &Connection, id: &str, doc: &str, company: &str, status: &str) {
    connection
        .execute(
            "INSERT INTO kpi_ingest_runs
                    (id, report_document_id, company_id, profile_version, status,
                     period_fiscal_year, period_type, scope, data_quality)
                 VALUES (?1, ?2, ?3, 'p1', ?4, 2025, 'FY', 'consolidated', 'final')",
            params![id, doc, company, status],
        )
        .expect("seed run");
}

/// #360 back-fit: `stage_observations` now requires a live lease. Every
/// test that stages against `setup()`'s run authenticates as this holder.
const TEST_HOLDER: &str = "agent-1";

fn one_observation() -> NewStagedObservation {
    NewStagedObservation {
        raw_label: "Przychody ze sprzedaży".to_owned(),
        raw_value: "1 234,5".to_owned(),
        currency: Some("pln".to_owned()),
        normalized_value: Some("1234.5".to_owned()),
        metric_key_candidate: Some("revenue".to_owned()),
        citation_page: Some(3),
        ..Default::default()
    }
}

thread_local! {
    /// Test-only hook: a delay injected between `stage_observations`'
    /// entry checks and its final guarded flip, so a test can cross the
    /// wall-clock lease expiry EXACTLY mid-batch (luna review B1 —
    /// sleeping before the call only exercises the entry check).
    static MID_BATCH_DELAY: std::cell::Cell<Option<std::time::Duration>> =
        const { std::cell::Cell::new(None) };
}

pub(super) fn mid_batch_test_delay() {
    if let Some(delay) = MID_BATCH_DELAY.with(|cell| cell.take()) {
        std::thread::sleep(delay);
    }
}

fn setup() -> (AppState, &'static str) {
    let connection = open_in_memory_database().expect("db");
    seed_company(&connection, "c1");
    seed_document(&connection, "doc1", "c1");
    seed_run(&connection, "run1", "doc1", "c1", "extracting");
    let state = AppState::new(connection);
    // The real claim seam (#360: "claim before staging") — establishes
    // the live lease `stage_observations` now requires.
    state
        .kpi_ingest_runs()
        .claim_next(TEST_HOLDER, 3600)
        .expect("claim")
        .expect("run1 must be claimable");
    (state, "run1")
}

// --- Test 1: stage ---------------------------------------------------

#[test]
fn stage_observations_first_snapshot_and_restage_after_validation_failure() {
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();

    let (revision, observations) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![one_observation(), one_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("first stage");
    assert_eq!(revision, 1);
    assert_eq!(observations.len(), 2);
    assert_eq!(observations[0].ordinal, 0);
    assert_eq!(observations[1].ordinal, 1);
    assert_eq!(observations[0].currency.as_deref(), Some("PLN"));

    let run = state
        .kpi_ingest_runs()
        .get_run(run_id)
        .expect("get")
        .expect("some");
    assert_eq!(run.status, KpiIngestRunState::Staged);
    assert!(run.manifest_hash.is_none());
    assert_eq!(run.manifest_revision, 1);

    // Restage requires validation_failed, not staged directly — #360's
    // real seam, replacing the raw status flip this test used before it
    // existed. The lease claimed in `setup()` is untouched by
    // `mark_validation_failed_on_connection` (no lease requirement on
    // that edge), so it stays live for the restage below.
    mark_validation_failed_on_connection(
        &state.checkout_for_tests().expect("checkout"),
        run_id,
        revision,
    )
    .expect("flip to validation_failed");

    let (revision2, observations2) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![one_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("restage");
    assert_eq!(revision2, 2);
    assert_eq!(observations2.len(), 1);

    // Revision 1 is untouched (audit trail).
    let rev1 = store
        .list_staged_observations(run_id, Some(1))
        .expect("list rev1");
    assert_eq!(rev1.len(), 2);
}

#[test]
fn stage_observations_rejects_non_stageable_statuses() {
    for status in [
        "discovered",
        "source_captured",
        "staged",
        "ready_to_commit",
        "committing",
        "complete",
        "partial",
        "failed",
        "cancelled",
    ] {
        let connection = open_in_memory_database().expect("db");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        seed_run(&connection, "run1", "doc1", "c1", status);
        let state = AppState::new(connection);
        let store = state.kpi_ingest_staging();

        let error = store
            .stage_observations(
                "run1",
                TEST_HOLDER,
                vec![one_observation()],
                &std::collections::BTreeMap::new(),
                None,
            )
            .expect_err(&format!("status '{status}' must be refused"));
        assert!(
            matches!(error, StorageError::InvalidRunStateForStaging { .. }),
            "status '{status}' produced {error:?}"
        );
    }
}

#[test]
fn stage_observations_requires_a_period_identity() {
    let connection = open_in_memory_database().expect("db");
    seed_company(&connection, "c1");
    seed_document(&connection, "doc1", "c1");
    connection
        .execute(
            "INSERT INTO kpi_ingest_runs (id, report_document_id, company_id, profile_version, status)
                 VALUES ('run1', 'doc1', 'c1', 'p1', 'extracting')",
            [],
        )
        .expect("seed run without period");
    let state = AppState::new(connection);
    state
        .kpi_ingest_runs()
        .claim_next(TEST_HOLDER, 3600)
        .expect("claim")
        .expect("run1 must be claimable");
    let store = state.kpi_ingest_staging();

    let error = store
        .stage_observations(
            "run1",
            TEST_HOLDER,
            vec![one_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect_err("no period_id, no descriptor");
    assert!(matches!(
        error,
        StorageError::InvalidKpiIngestRunValue { key: "period", .. }
    ));
}

#[test]
fn stage_observations_rejects_unknown_run_and_bad_vocab() {
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();

    let error = store
        .stage_observations(
            "kpiing_missing",
            TEST_HOLDER,
            vec![one_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect_err("unknown run");
    assert!(matches!(error, StorageError::KpiIngestRunNotFound { .. }));

    let mut bad_currency = one_observation();
    bad_currency.currency = Some("dollars".to_owned());
    let error = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![bad_currency],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect_err("bad currency");
    assert!(matches!(
        error,
        StorageError::InvalidKpiIngestRunValue {
            key: "currency",
            ..
        }
    ));

    // Nothing was inserted for the rejected batch.
    assert!(store
        .list_staged_observations(run_id, None)
        .expect("list")
        .is_empty());
}

#[test]
fn excluded_requires_reason() {
    // ADR 0102 dec. 1: mappingStatus="excluded" is legal ONLY with a
    // non-blank exclusionReason; a reason on any other disposition is a
    // typed refusal too (never silently dropped).
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();

    let mut missing_reason = one_observation();
    missing_reason.mapping_status = Some("excluded".to_owned());
    let error = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![missing_reason],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect_err("excluded without a reason must refuse");
    assert!(matches!(
        error,
        StorageError::InvalidKpiIngestRunValue {
            key: "exclusion_reason",
            ..
        }
    ));

    let mut blank_reason = one_observation();
    blank_reason.mapping_status = Some("excluded".to_owned());
    blank_reason.exclusion_reason = Some("   ".to_owned());
    let error = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![blank_reason],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect_err("blank exclusion reason must refuse");
    assert!(matches!(
        error,
        StorageError::InvalidKpiIngestRunValue {
            key: "exclusion_reason",
            ..
        }
    ));

    let mut reason_on_mapped = one_observation();
    reason_on_mapped.mapping_status = Some("mapped".to_owned());
    reason_on_mapped.exclusion_reason = Some("not applicable".to_owned());
    let error = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![reason_on_mapped],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect_err("a reason on a non-excluded observation must refuse");
    assert!(matches!(
        error,
        StorageError::InvalidKpiIngestRunValue {
            key: "exclusion_reason",
            ..
        }
    ));

    assert!(store
        .list_staged_observations(run_id, None)
        .expect("list")
        .is_empty());

    // The legal shape: excluded with a real reason stages cleanly.
    let mut excluded = one_observation();
    excluded.mapping_status = Some("excluded".to_owned());
    excluded.exclusion_reason = Some("footnote disclosure, not a KPI".to_owned());
    let (_, rows) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![excluded],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("excluded with a reason stages");
    assert_eq!(rows[0].mapping_status, "excluded");
    assert_eq!(
        rows[0].exclusion_reason.as_deref(),
        Some("footnote disclosure, not a KPI")
    );
}

#[test]
fn stage_observations_numbers_ordinals_zero_based_and_contiguous() {
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();
    let (_, observations) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![one_observation(), one_observation(), one_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("stage three");
    let ordinals: Vec<i64> = observations.iter().map(|o| o.ordinal).collect();
    assert_eq!(ordinals, vec![0, 1, 2]);
}

// --- Test 2: list / latest_revision -----------------------------------

#[test]
fn list_and_latest_revision_default_to_the_newest() {
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();
    assert_eq!(
        store.latest_staging_revision(run_id).expect("none yet"),
        None
    );

    let (revision1, _) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![one_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("rev1");
    mark_validation_failed_on_connection(
        &state.checkout_for_tests().expect("checkout"),
        run_id,
        revision1,
    )
    .expect("flip");
    store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![one_observation(), one_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("rev2");

    assert_eq!(
        store.latest_staging_revision(run_id).expect("latest"),
        Some(2)
    );
    assert_eq!(
        store
            .list_staged_observations(run_id, None)
            .expect("list latest")
            .len(),
        2
    );
    assert_eq!(
        store
            .list_staged_observations(run_id, Some(1))
            .expect("list rev1")
            .len(),
        1
    );
}

// --- Test 3: apply_validation_outcome (#361) ----------------------------

/// `one_observation()` leaves `measure_window` unset (`period.window_missing`
/// would always flag it) — these atom tests need a "clean" staged row
/// that seals to a `passed`/`unreviewed` (never `flagged`) observation by
/// default, so a happy-path "ready" scenario is actually reachable.
fn clean_observation() -> NewStagedObservation {
    NewStagedObservation {
        measure_window: Some("flow".to_owned()),
        ..one_observation()
    }
}

/// Builds a [`SealedManifest`] whose observations are EXACTLY the given
/// staged rows (same ids/ordinal/content — the atom's coverage/content
/// guards require this), each carrying a resolved `revenue` definition
/// with no history (an honest `plausibility.abstained` -> `unreviewed`,
/// which never flips `outcome` — the atom tests below care about the
/// COVERAGE/CONTENT/BINDING guards, not the rule engine already covered
/// by `fundamentals::kpi_manifest::tests`).
fn sealed_manifest_for(run_id: &str, revision: i64, rows: &[StagedObservation]) -> SealedManifest {
    sealed_manifest_with_run(run_id, revision, rows, |_run| {})
}

/// [`sealed_manifest_for`], but `customize` may mutate the manifest's own
/// run context (companyId, sourceContentHash, expectedKpis, ...) BEFORE
/// it is sealed — the #361 finding-1b run-context binding tests need a
/// manifest that disagrees with the live run row on exactly one field.
fn sealed_manifest_with_run(
    run_id: &str,
    revision: i64,
    rows: &[StagedObservation],
    customize: impl FnOnce(&mut crate::fundamentals::kpi_manifest::ManifestRunInput),
) -> SealedManifest {
    use crate::fundamentals::kpi_manifest::{
        build_manifest, ManifestObservationInput, ManifestRunInput, MissingReasons,
        ResolvedDefinitionInput, SlotHistoryInput,
    };
    let mut run = ManifestRunInput {
        run_id: run_id.to_owned(),
        revision,
        company_id: "c1".to_owned(),
        report_document_id: "doc1".to_owned(),
        source_content_hash: None,
        scope: "consolidated".to_owned(),
        data_quality: "final".to_owned(),
        period_id: None,
        fiscal_year: 2025,
        period_type: "FY".to_owned(),
        missing_reasons: MissingReasons::None,
        expected: None,
    };
    customize(&mut run);
    let observations: Vec<ManifestObservationInput> = rows
        .iter()
        .map(|row| ManifestObservationInput {
            observation_id: row.id.clone(),
            ordinal: row.ordinal,
            raw_label: row.raw_label.clone(),
            raw_value: row.raw_value.clone(),
            metric_key_candidate: row.metric_key_candidate.clone(),
            mapping_status: "mapped".to_owned(),
            exclusion_reason: None,
            normalized_value: row.normalized_value.clone(),
            currency: row.currency.clone(),
            unit_scale: row.unit_scale.clone(),
            measure_window: row.measure_window.clone(),
            attribution: row.attribution.clone(),
            scope: row.scope.clone(),
            citation_page: row.citation_page,
            citation_table: row.citation_table.clone(),
            citation_row: row.citation_row.clone(),
            citation_quote: row.citation_quote.clone(),
            // `definition_id` is distinct per ordinal -- two staged rows
            // built from the same `clean_observation()` fixture would
            // otherwise share one slot (definition_id, attribution_eff,
            // measure_window_eff) and trip `duplicate.repeat`, which
            // these tests don't want to reason about. `metric_key`
            // mirrors the row's own candidate (the real resolver always
            // echoes back the exact key it matched on) -- a synthetic
            // per-ordinal metric_key here would fake a mapping the
            // resolver could never produce and desync `present`'s
            // completeness accounting (finding 1a) from the candidate
            // this same content projection binds to the live row.
            definition: Some(ResolvedDefinitionInput {
                definition_id: format!("kpidef_test_metric_{}", row.ordinal),
                metric_key: row.metric_key_candidate.clone().unwrap_or_default(),
                value_kind: "monetary".to_owned(),
                period_nature: "duration".to_owned(),
                history: SlotHistoryInput::default(),
            }),
        })
        .collect();
    SealedManifest::seal(build_manifest(&run, &observations)).expect("consistent manifest seals")
}

/// Same as [`sealed_manifest_for`], but the given `row.id`s are flagged
/// (no resolved definition -> `mapping.unresolved`) so the sealed
/// manifest's derived `outcome` is `failed`.
fn failed_sealed_manifest_for(
    run_id: &str,
    revision: i64,
    rows: &[StagedObservation],
) -> SealedManifest {
    use crate::fundamentals::kpi_manifest::{
        build_manifest, ManifestObservationInput, ManifestRunInput, MissingReasons,
    };
    let run = ManifestRunInput {
        run_id: run_id.to_owned(),
        revision,
        company_id: "c1".to_owned(),
        report_document_id: "doc1".to_owned(),
        source_content_hash: None,
        scope: "consolidated".to_owned(),
        data_quality: "final".to_owned(),
        period_id: None,
        fiscal_year: 2025,
        period_type: "FY".to_owned(),
        missing_reasons: MissingReasons::None,
        expected: None,
    };
    let observations: Vec<ManifestObservationInput> = rows
        .iter()
        .map(|row| ManifestObservationInput {
            observation_id: row.id.clone(),
            ordinal: row.ordinal,
            raw_label: row.raw_label.clone(),
            raw_value: row.raw_value.clone(),
            metric_key_candidate: row.metric_key_candidate.clone(),
            mapping_status: "unmapped".to_owned(),
            exclusion_reason: None,
            normalized_value: row.normalized_value.clone(),
            currency: row.currency.clone(),
            unit_scale: row.unit_scale.clone(),
            measure_window: row.measure_window.clone(),
            attribution: row.attribution.clone(),
            scope: row.scope.clone(),
            citation_page: row.citation_page,
            citation_table: row.citation_table.clone(),
            citation_row: row.citation_row.clone(),
            citation_quote: row.citation_quote.clone(),
            definition: None,
        })
        .collect();
    SealedManifest::seal(build_manifest(&run, &observations)).expect("consistent manifest seals")
}

/// [`sealed_manifest_for`], but every row is sealed `excluded` with
/// `reason` — used by the ADR 0102 tamper tests, which build a manifest
/// claiming a disposition/reason the LIVE staged row does not actually
/// carry.
fn excluded_sealed_manifest_for(
    run_id: &str,
    revision: i64,
    rows: &[StagedObservation],
    reason: &str,
) -> SealedManifest {
    use crate::fundamentals::kpi_manifest::{
        build_manifest, ManifestObservationInput, ManifestRunInput, MissingReasons,
    };
    let run = ManifestRunInput {
        run_id: run_id.to_owned(),
        revision,
        company_id: "c1".to_owned(),
        report_document_id: "doc1".to_owned(),
        source_content_hash: None,
        scope: "consolidated".to_owned(),
        data_quality: "final".to_owned(),
        period_id: None,
        fiscal_year: 2025,
        period_type: "FY".to_owned(),
        missing_reasons: MissingReasons::None,
        expected: None,
    };
    let observations: Vec<ManifestObservationInput> = rows
        .iter()
        .map(|row| ManifestObservationInput {
            observation_id: row.id.clone(),
            ordinal: row.ordinal,
            raw_label: row.raw_label.clone(),
            raw_value: row.raw_value.clone(),
            metric_key_candidate: row.metric_key_candidate.clone(),
            mapping_status: "excluded".to_owned(),
            exclusion_reason: Some(reason.to_owned()),
            normalized_value: row.normalized_value.clone(),
            currency: row.currency.clone(),
            unit_scale: row.unit_scale.clone(),
            measure_window: row.measure_window.clone(),
            attribution: row.attribution.clone(),
            scope: row.scope.clone(),
            citation_page: row.citation_page,
            citation_table: row.citation_table.clone(),
            citation_row: row.citation_row.clone(),
            citation_quote: row.citation_quote.clone(),
            definition: None,
        })
        .collect();
    SealedManifest::seal(build_manifest(&run, &observations)).expect("consistent manifest seals")
}

#[test]
fn apply_validation_outcome_happy_path_ready_in_one_transaction() {
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();
    let (revision, rows) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![clean_observation(), clean_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("stage");
    let sealed = sealed_manifest_for(run_id, revision, &rows);
    let manifest_hash = sealed.manifest_hash().to_owned();

    store
        .apply_validation_outcome(run_id, revision, sealed)
        .expect("apply");

    let after = store
        .list_staged_observations(run_id, Some(revision))
        .expect("list");
    assert!(after.iter().all(|o| o.validation_state == "unreviewed"));
    assert!(after.iter().all(|o| o.validation_codes_json.is_some()));

    let run = state
        .kpi_ingest_runs()
        .get_run(run_id)
        .expect("get")
        .expect("some");
    assert_eq!(run.status, KpiIngestRunState::ReadyToCommit);
    assert_eq!(run.manifest_hash.as_deref(), Some(manifest_hash.as_str()));

    let attempts = state
        .kpi_ingest_runs()
        .list_validation_attempts(run_id)
        .expect("attempts");
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].outcome, "ready");
    assert_eq!(attempts[0].attempt, 1);
    assert_eq!(attempts[0].manifest_hash, manifest_hash);
}

#[test]
fn apply_validation_outcome_failed_in_one_transaction_leaves_run_hash_null() {
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();
    let (revision, rows) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![clean_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("stage");
    let sealed = failed_sealed_manifest_for(run_id, revision, &rows);
    let manifest_hash = sealed.manifest_hash().to_owned();

    store
        .apply_validation_outcome(run_id, revision, sealed)
        .expect("apply");

    let after = store
        .list_staged_observations(run_id, Some(revision))
        .expect("list");
    assert_eq!(after[0].validation_state, "flagged");

    let run = state
        .kpi_ingest_runs()
        .get_run(run_id)
        .expect("get")
        .expect("some");
    assert_eq!(run.status, KpiIngestRunState::ValidationFailed);
    assert!(
        run.manifest_hash.is_none(),
        "failed never sets run.manifest_hash"
    );

    let attempts = state
        .kpi_ingest_runs()
        .list_validation_attempts(run_id)
        .expect("attempts");
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].outcome, "failed");
    assert_eq!(attempts[0].manifest_hash, manifest_hash);
    assert!(attempts[0].manifest_json.contains("mapping.unresolved"));
}

#[test]
fn apply_validation_outcome_refuses_a_non_staged_run() {
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();
    let (revision, rows) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![clean_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("stage");
    state
        .kpi_ingest_runs()
        .cancel_run(run_id)
        .expect("cancel from staged is legal");
    let sealed = sealed_manifest_for(run_id, revision, &rows);
    let error = store
        .apply_validation_outcome(run_id, revision, sealed)
        .expect_err("validating a cancelled run must refuse (status != staged)");
    assert!(matches!(error, StorageError::InvalidStagingRevision { .. }));
}

/// The lease is wall-clock state: it can expire DURING a long staging
/// batch even though no other writer can touch the row under the Immediate
/// transaction. The final flip re-guards the live lease (luna review B1).
#[test]
fn stage_observations_refuses_when_the_lease_expires_mid_batch() {
    let connection = open_in_memory_database().expect("db");
    seed_company(&connection, "c1");
    seed_document(&connection, "doc1", "c1");
    seed_run(&connection, "run1", "doc1", "c1", "extracting");
    let state = AppState::new(connection);
    state
        .kpi_ingest_runs()
        .claim_next(TEST_HOLDER, 1)
        .expect("claim")
        .expect("claimed");
    // The lease is LIVE at the entry check; the injected delay crosses
    // the 1-second expiry between that check and the final guarded flip —
    // the genuine mid-batch scenario (luna review B1: the pre-call sleep
    // variant only exercised the entry check, and the originally broken
    // implementation would have passed it).
    MID_BATCH_DELAY.with(|cell| cell.set(Some(std::time::Duration::from_millis(1200))));
    let error = state
        .kpi_ingest_staging()
        .stage_observations(
            "run1",
            TEST_HOLDER,
            vec![one_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect_err("an expired lease must not stage");
    // #386: the mid-batch refusal classifies through the shared three-way
    // vocabulary — the holder's OWN lease expired, so the typed remedy is
    // `run_lease_expired` (re-claim via start), not the residual shape.
    assert!(matches!(error, StorageError::RunLeaseExpired { .. }));
    let run = state
        .kpi_ingest_runs()
        .get_run("run1")
        .expect("get")
        .expect("some");
    assert_eq!(run.status, KpiIngestRunState::Extracting);
    assert_eq!(run.manifest_revision, 0, "no revision bump on refusal");
}

#[test]
fn apply_validation_outcome_refuses_stale_or_frozen_revision() {
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();
    let (revision, rows) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![clean_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("stage");

    let sealed = sealed_manifest_for(run_id, revision + 1, &rows);
    let error = store
        .apply_validation_outcome(run_id, revision + 1, sealed)
        .expect_err("stale revision");
    assert!(matches!(error, StorageError::InvalidStagingRevision { .. }));

    // Freeze the revision (a manifest was issued). Structurally
    // unreachable through any production seam while `status` stays
    // `staged` (the atom's own transition to `ready_to_commit` moves
    // status off `staged` in the SAME update that sets the hash) —
    // raw-seeded defensively to probe this method's OWN freeze guard.
    state
        .checkout_for_tests()
        .expect("raw")
        .execute(
            "UPDATE kpi_ingest_runs SET manifest_hash = 'deadbeef' WHERE id = ?1",
            [run_id],
        )
        .expect("freeze");
    let sealed = sealed_manifest_for(run_id, revision, &rows);
    let error = store
        .apply_validation_outcome(run_id, revision, sealed)
        .expect_err("frozen revision");
    assert!(matches!(error, StorageError::InvalidStagingRevision { .. }));
}

#[test]
fn apply_validation_outcome_refuses_wrong_run_id_and_revision_binding() {
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();
    let (revision, rows) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![clean_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("stage");

    // Sealed for a DIFFERENT run id -- binding mismatch, distinct from
    // the coverage/revision guards.
    let foreign_run_sealed = sealed_manifest_for("kpiing_other", revision, &rows);
    let error = store
        .apply_validation_outcome(run_id, revision, foreign_run_sealed)
        .expect_err("foreign run id must be refused");
    assert!(matches!(error, StorageError::SealedManifestRejected { .. }));

    // Sealed for the RIGHT run but a revision the manifest itself claims
    // is different from what the caller passed.
    let wrong_revision_sealed = sealed_manifest_for(run_id, revision + 41, &rows);
    let error = store
        .apply_validation_outcome(run_id, revision, wrong_revision_sealed)
        .expect_err("mismatched revision binding must be refused");
    assert!(matches!(error, StorageError::SealedManifestRejected { .. }));

    // Zero writes from either refusal.
    let after = store
        .list_staged_observations(run_id, Some(revision))
        .expect("list");
    assert_eq!(after[0].validation_state, "none");
    assert!(state
        .kpi_ingest_runs()
        .list_validation_attempts(run_id)
        .expect("attempts")
        .is_empty());
}

#[test]
fn apply_validation_outcome_refuses_run_context_mismatches_with_zero_writes() {
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();
    let (revision, rows) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![clean_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("stage");

    let assert_refused = |sealed: SealedManifest| {
        let error = store
            .apply_validation_outcome(run_id, revision, sealed)
            .expect_err("run-context mismatch must be refused");
        assert!(matches!(error, StorageError::SealedManifestRejected { .. }));
    };

    // wrong companyId
    assert_refused(sealed_manifest_with_run(run_id, revision, &rows, |run| {
        run.company_id = "someone_else".to_owned();
    }));
    // wrong sourceContentHash (live run's is NULL; manifest claims one)
    assert_refused(sealed_manifest_with_run(run_id, revision, &rows, |run| {
        run.source_content_hash = Some("different-hash".to_owned());
    }));
    // wrong expectedKpis (live run's expected_kpis_json is NULL; manifest
    // claims a stamped snapshot)
    assert_refused(sealed_manifest_with_run(run_id, revision, &rows, |run| {
        run.expected = Some(crate::fundamentals::kpi_manifest::ExpectedSnapshot {
            schema_version: 1,
            source: "kpi_relevance".to_owned(),
            pack_version: None,
            keys: ["revenue".to_owned()].into_iter().collect(),
        });
    }));

    let after = store
        .list_staged_observations(run_id, Some(revision))
        .expect("list");
    assert_eq!(
        after[0].validation_state, "none",
        "every context-mismatch refusal above must write nothing"
    );
    assert!(state
        .kpi_ingest_runs()
        .list_validation_attempts(run_id)
        .expect("attempts")
        .is_empty());
}

#[test]
fn apply_validation_outcome_refuses_missing_or_extra_observation_coverage() {
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();
    let (revision, rows) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![clean_observation(), clean_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("stage");

    // Missing one of the two staged observations.
    let partial = sealed_manifest_for(run_id, revision, &rows[..1]);
    let error = store
        .apply_validation_outcome(run_id, revision, partial)
        .expect_err("a manifest missing a staged observation must be refused");
    assert!(matches!(error, StorageError::SealedManifestRejected { .. }));

    let after = store
        .list_staged_observations(run_id, Some(revision))
        .expect("list");
    assert!(
        after.iter().all(|o| o.validation_state == "none"),
        "a coverage refusal must write nothing"
    );
    assert!(state
        .kpi_ingest_runs()
        .list_validation_attempts(run_id)
        .expect("attempts")
        .is_empty());
}

#[test]
fn apply_validation_outcome_refuses_content_tamper_with_zero_writes() {
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();
    let (revision, rows) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![clean_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("stage");

    // Same observation id, but the manifest's own staged-content
    // projection was built from a TAMPERED value that no longer matches
    // the live row.
    let mut tampered_row = rows[0].clone();
    tampered_row.normalized_value = Some("999999".to_owned());
    let sealed = sealed_manifest_for(run_id, revision, std::slice::from_ref(&tampered_row));

    let error = store
        .apply_validation_outcome(run_id, revision, sealed)
        .expect_err("a manifest whose content projection disagrees with the live row must refuse");
    assert!(matches!(error, StorageError::SealedManifestRejected { .. }));

    let after = store
        .list_staged_observations(run_id, Some(revision))
        .expect("list");
    assert_eq!(
        after[0].validation_state, "none",
        "tamper refusal writes nothing"
    );
    assert_eq!(
        after[0].normalized_value.as_deref(),
        Some("1234.5"),
        "the LIVE row's value must be untouched"
    );
    assert!(state
        .kpi_ingest_runs()
        .list_validation_attempts(run_id)
        .expect("attempts")
        .is_empty());
}

#[test]
fn status_flipped_to_excluded_after_validation_is_refused() {
    // ADR 0102 dec. 1/3: a manifest claiming a row is `excluded` when the
    // LIVE staged row was never actually staged that way must be refused
    // by the content-projection compare — zero writes, exactly like any
    // other content tamper.
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();
    let (revision, rows) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![clean_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("stage");
    // The live row's mapping_status stays whatever `clean_observation()`
    // defaults to (never "excluded") -- the manifest claims otherwise.
    let sealed = excluded_sealed_manifest_for(run_id, revision, &rows, "not applicable");

    let error = store
        .apply_validation_outcome(run_id, revision, sealed)
        .expect_err("a manifest claiming excluded over a non-excluded live row must refuse");
    assert!(matches!(error, StorageError::SealedManifestRejected { .. }));

    let after = store
        .list_staged_observations(run_id, Some(revision))
        .expect("list");
    assert_eq!(after[0].validation_state, "none", "tamper writes nothing");
    assert_ne!(after[0].mapping_status, "excluded");
    assert!(state
        .kpi_ingest_runs()
        .list_validation_attempts(run_id)
        .expect("attempts")
        .is_empty());
}

#[test]
fn reason_changed_after_validation_is_refused() {
    // ADR 0102 dec. 1/3: the live row IS excluded, but with a DIFFERENT
    // reason than the manifest claims -- content-projection still
    // refuses (the reason is part of the sealed disposition, dec. 1).
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();
    let mut excluded_obs = clean_observation();
    excluded_obs.mapping_status = Some("excluded".to_owned());
    excluded_obs.exclusion_reason = Some("original reason".to_owned());
    let (revision, rows) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![excluded_obs],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("stage");
    assert_eq!(rows[0].exclusion_reason.as_deref(), Some("original reason"));
    let sealed = excluded_sealed_manifest_for(run_id, revision, &rows, "a different reason");

    let error = store
        .apply_validation_outcome(run_id, revision, sealed)
        .expect_err("a manifest with a changed exclusion reason must refuse");
    assert!(matches!(error, StorageError::SealedManifestRejected { .. }));

    let after = store
        .list_staged_observations(run_id, Some(revision))
        .expect("list");
    assert_eq!(after[0].validation_state, "none", "tamper writes nothing");
    assert_eq!(
        after[0].exclusion_reason.as_deref(),
        Some("original reason"),
        "the LIVE row's reason must be untouched"
    );
    assert!(state
        .kpi_ingest_runs()
        .list_validation_attempts(run_id)
        .expect("attempts")
        .is_empty());
}

#[test]
fn apply_validation_outcome_codes_on_rows_equal_codes_in_manifest() {
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();
    let (revision, rows) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![clean_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("stage");
    let sealed = sealed_manifest_for(run_id, revision, &rows);
    let expected_codes_json = sealed.observation_verdicts()[0]
        .validation_codes_json
        .clone();

    store
        .apply_validation_outcome(run_id, revision, sealed)
        .expect("apply");

    let after = store
        .list_staged_observations(run_id, Some(revision))
        .expect("list");
    assert_eq!(
        after[0].validation_codes_json.as_deref(),
        Some(expected_codes_json.as_str()),
        "the row's stored codes must be byte-identical to the sealed manifest's"
    );
}

#[test]
fn apply_validation_outcome_attempt_survives_restage_and_invalidate_bumps_attempt() {
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();
    let (revision, rows) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![clean_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("stage");

    // First attempt: fails.
    let failed = failed_sealed_manifest_for(run_id, revision, &rows);
    store
        .apply_validation_outcome(run_id, revision, failed)
        .expect("apply failed");
    let attempts = state
        .kpi_ingest_runs()
        .list_validation_attempts(run_id)
        .expect("attempts");
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].attempt, 1);
    assert_eq!(attempts[0].outcome, "failed");

    // Restage the SAME revision's repair (validation_failed -> staged is
    // legal, #360) and validate again -- attempt 2 for revision 2.
    let (revision2, rows2) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![clean_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("restage");
    let sealed2 = sealed_manifest_for(run_id, revision2, &rows2);
    store
        .apply_validation_outcome(run_id, revision2, sealed2)
        .expect("apply ready");

    // Invalidate the manifest and re-validate the SAME revision -- the
    // UNIQUE(run_id, revision, attempt) constraint means this must land
    // as attempt 2 for revision2, not collide with attempt 1.
    state
        .kpi_ingest_runs()
        .invalidate_manifest(run_id)
        .expect("invalidate");
    let rows2_current = store
        .list_staged_observations(run_id, Some(revision2))
        .expect("list");
    let sealed2_again = sealed_manifest_for(run_id, revision2, &rows2_current);
    store
        .apply_validation_outcome(run_id, revision2, sealed2_again)
        .expect("re-apply after invalidate");

    let attempts = state
        .kpi_ingest_runs()
        .list_validation_attempts(run_id)
        .expect("attempts");
    assert_eq!(
        attempts.len(),
        3,
        "revision1 attempt 1 + revision2 attempts 1 and 2"
    );
    let rev2_attempts: Vec<_> = attempts
        .iter()
        .filter(|a| a.revision == revision2)
        .collect();
    assert_eq!(rev2_attempts.len(), 2);
    assert_eq!(rev2_attempts[0].attempt, 1);
    assert_eq!(rev2_attempts[1].attempt, 2);

    // The revision-1 failed attempt's diagnostics are still readable
    // (audit survives the re-stage/invalidate cycle).
    let rev1_attempt = attempts
        .iter()
        .find(|a| a.revision == revision)
        .expect("rev1 attempt");
    assert_eq!(rev1_attempt.outcome, "failed");
    assert!(rev1_attempt.manifest_json.contains("mapping.unresolved"));
}

// --- Test 4: record_commit_receipt ------------------------------------

#[test]
fn record_commit_receipt_in_an_external_transaction_and_replay() {
    let (state, run_id) = setup();
    let new_receipt = || NewCommitReceipt {
        run_id: run_id.to_owned(),
        manifest_hash: "hash1".to_owned(),
        manifest_revision: 1,
        terminal_status: "complete".to_owned(),
        period_id: None,
        accepted_count: 3,
        outcomes_schema_version: 1,
        outcomes_json: "[{\"observationId\":\"kpiobs_1\",\"outcome\":\"created\"}]".to_owned(),
    };

    let receipt = {
        let mut connection = state.checkout_for_tests().expect("raw connection");
        let tx = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("tx");
        let receipt = record_commit_receipt(&tx, new_receipt()).expect("record");
        tx.commit().expect("commit");
        receipt
    };
    assert_eq!(receipt.run_id, run_id);
    assert_eq!(receipt.accepted_count, 3);
    assert_eq!(receipt.outcomes_schema_version, 1);

    let error = {
        let mut connection = state.checkout_for_tests().expect("raw connection");
        let tx = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("tx");
        let error = record_commit_receipt(&tx, new_receipt()).expect_err("second insert");
        tx.commit().expect("commit");
        error
    };
    assert!(matches!(
        error,
        StorageError::CommitReceiptAlreadyRecorded { .. }
    ));

    let fetched = state
        .kpi_ingest_staging()
        .get_commit_receipt(run_id)
        .expect("get")
        .expect("some");
    assert_eq!(fetched.id, receipt.id);
    assert_eq!(fetched.outcomes_json, receipt.outcomes_json);
}

/// Only the `UNIQUE(run_id)` violation is a replay; every other
/// constraint (bad status vocab, missing run FK) must surface as its own
/// storage error, never as `CommitReceiptAlreadyRecorded` (luna review P1).
#[test]
fn record_commit_receipt_maps_only_run_uniqueness_to_already_recorded() {
    let (state, run_id) = setup();
    let base = |run: &str| NewCommitReceipt {
        run_id: run.to_owned(),
        manifest_hash: "hash1".to_owned(),
        manifest_revision: 1,
        terminal_status: "complete".to_owned(),
        period_id: None,
        accepted_count: 1,
        outcomes_schema_version: 1,
        outcomes_json: "[]".to_owned(),
    };

    // Bad terminal_status trips the CHECK — a constraint violation that is
    // NOT a replay.
    let mut connection = state.checkout_for_tests().expect("raw connection");
    let tx = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .expect("tx");
    let mut bad_status = base(run_id);
    bad_status.terminal_status = "half-done".to_owned();
    let error = record_commit_receipt(&tx, bad_status).expect_err("bad status");
    assert!(
        !matches!(error, StorageError::CommitReceiptAlreadyRecorded { .. }),
        "a CHECK violation must not masquerade as a replay: {error:?}"
    );

    // A receipt for a nonexistent run trips the FK — also not a replay.
    let error = record_commit_receipt(&tx, base("kpiing_missing")).expect_err("missing run");
    assert!(
        !matches!(error, StorageError::CommitReceiptAlreadyRecorded { .. }),
        "an FK violation must not masquerade as a replay: {error:?}"
    );
    drop(tx);
}

#[test]
fn record_commit_receipt_rolls_back_with_its_external_transaction() {
    let (state, run_id) = setup();
    {
        let mut connection = state.checkout_for_tests().expect("raw connection");
        let tx = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("tx");
        record_commit_receipt(
            &tx,
            NewCommitReceipt {
                run_id: run_id.to_owned(),
                manifest_hash: "hash1".to_owned(),
                manifest_revision: 1,
                terminal_status: "complete".to_owned(),
                period_id: None,
                accepted_count: 1,
                outcomes_schema_version: 1,
                outcomes_json: "[]".to_owned(),
            },
        )
        .expect("record");
        // Deliberately dropped without commit -> rollback.
    }

    assert!(state
        .kpi_ingest_staging()
        .get_commit_receipt(run_id)
        .expect("get")
        .is_none());
}

// --- Test 6: CHECK constraints ------------------------------------------

#[test]
fn check_constraints_reject_bad_rows() {
    let (state, run_id) = setup();
    let connection = state.checkout_for_tests().expect("raw connection");

    assert!(
        connection
            .execute(
                "INSERT INTO kpi_staged_observations (id, run_id, revision, ordinal, raw_label, raw_value)
                     VALUES ('bad1', ?1, 0, 0, 'l', 'v')",
                [run_id],
            )
            .is_err(),
        "revision 0 must be rejected"
    );
    assert!(
        connection
            .execute(
                "INSERT INTO kpi_staged_observations (id, run_id, revision, ordinal, raw_label, raw_value)
                     VALUES ('bad2', ?1, 1, -1, 'l', 'v')",
                [run_id],
            )
            .is_err(),
        "negative ordinal must be rejected"
    );
    connection
        .execute(
            "INSERT INTO kpi_staged_observations (id, run_id, revision, ordinal, raw_label, raw_value)
                 VALUES ('ok1', ?1, 1, 0, 'l', 'v')",
            [run_id],
        )
        .expect("first row at (run, 1, 0) must succeed");
    assert!(
        connection
            .execute(
                "INSERT INTO kpi_staged_observations (id, run_id, revision, ordinal, raw_label, raw_value)
                     VALUES ('dup', ?1, 1, 0, 'l', 'v')",
                [run_id],
            )
            .is_err(),
        "UNIQUE(run_id, revision, ordinal) must reject a duplicate"
    );

    // A second commit receipt for the same run is rejected by UNIQUE(run_id).
    connection
        .execute(
            "INSERT INTO kpi_ingest_commit_receipts
                    (id, run_id, manifest_hash, manifest_revision, terminal_status, accepted_count, outcomes_json)
                 VALUES ('r1', ?1, 'h1', 1, 'complete', 0, '[]')",
            [run_id],
        )
        .expect("first receipt must succeed");
    assert!(
        connection
            .execute(
                "INSERT INTO kpi_ingest_commit_receipts
                        (id, run_id, manifest_hash, manifest_revision, terminal_status, accepted_count, outcomes_json)
                     VALUES ('r2', ?1, 'h2', 1, 'complete', 0, '[]')",
                [run_id],
            )
            .is_err(),
        "a second receipt for the same run must be rejected"
    );
}

/// Migration 0139's CHECK constraints, exercised directly against raw
/// SQL (#361 test group 10: "0139 w inwentarzu, tabela utworzona, CHECKi
/// działają" -- the inventory guard is `migrations::tests::
/// every_migration_file_is_registered_and_vice_versa`).
#[test]
fn validation_attempts_check_constraints_reject_bad_rows() {
    let (state, run_id) = setup();
    let connection = state.checkout_for_tests().expect("raw connection");
    let insert = |id: &str, revision: i64, attempt: i64, outcome: &str| {
        connection.execute(
            "INSERT INTO kpi_ingest_validation_attempts
                    (id, run_id, revision, attempt, outcome, manifest_hash, manifest_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'h', '{}')",
            params![id, run_id, revision, attempt, outcome],
        )
    };

    assert!(
        insert("bad1", 0, 1, "ready").is_err(),
        "revision 0 must be rejected"
    );
    assert!(
        insert("bad2", 1, 0, "ready").is_err(),
        "attempt 0 must be rejected"
    );
    assert!(
        insert("bad3", 1, 1, "half-done").is_err(),
        "an outcome outside ('ready', 'failed') must be rejected"
    );
    insert("ok1", 1, 1, "ready").expect("(run, 1, 1) must succeed");
    assert!(
        insert("dup", 1, 1, "failed").is_err(),
        "UNIQUE(run_id, revision, attempt) must reject a duplicate"
    );
    insert("ok2", 1, 2, "failed").expect("a second attempt for the same revision must succeed");
}

// --- Test 7: revision consistency ---------------------------------------

#[test]
fn stage_observations_bumps_revision_and_zeroes_manifest_hash() {
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();
    let (revision, _) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![one_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("stage");

    // Simulate an issued manifest while still `staged` — structurally
    // unreachable through any production seam (same carve-out as
    // `apply_validation_outcome_refuses_stale_or_frozen_revision`), so
    // raw-seeded; the SUBSEQUENT status flip to `validation_failed`,
    // however, has a real #360/#361 seam now and no longer needs raw SQL.
    state
        .checkout_for_tests()
        .expect("raw")
        .execute(
            "UPDATE kpi_ingest_runs SET manifest_hash = 'deadbeef' WHERE id = ?1",
            [run_id],
        )
        .expect("simulate an issued manifest");
    mark_validation_failed_on_connection(
        &state.checkout_for_tests().expect("checkout"),
        run_id,
        revision,
    )
    .expect("flip");

    store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![one_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("restage");
    let run = state
        .kpi_ingest_runs()
        .get_run(run_id)
        .expect("get")
        .expect("some");
    assert!(
        run.manifest_hash.is_none(),
        "a new staging snapshot must zero out the prior manifest_hash"
    );
    assert_eq!(run.manifest_revision, 2);
}

// --- Test 8: two racing stagers -----------------------------------------

/// Two threads racing `stage_observations` on the SAME run: the new
/// status guard means exactly ONE winner (the run leaves `extracting`
/// for `staged`), the other gets a typed `InvalidRunStateForStaging` —
/// never two revisions racing in. Needs a FILE-backed pool (the
/// `claim_next_two_threads_exactly_one_winner` idiom,
/// `kpi_ingest_runs.rs`) — the in-memory single-connection path can never
/// exercise a genuine SQLite-level race.
#[test]
fn stage_observations_two_threads_exactly_one_winner() {
    use r2d2_sqlite::SqliteConnectionManager;
    use std::sync::Arc;

    let db_path = std::env::temp_dir().join(format!(
        "brawler-kpi-staging-race-{}-{}.sqlite3",
        std::process::id(),
        time::OffsetDateTime::now_utc().unix_timestamp_nanos()
    ));
    {
        let mut connection = Connection::open(&db_path).expect("open file db");
        crate::storage::migrations::apply_migrations(&mut connection).expect("migrate");
        seed_company(&connection, "c1");
        seed_document(&connection, "doc1", "c1");
        seed_run(&connection, "run1", "doc1", "c1", "extracting");
    }
    let manager = SqliteConnectionManager::file(&db_path).with_init(|connection| {
        connection.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        connection.pragma_update(None, "busy_timeout", 5000i64)?;
        Ok(())
    });
    let pool = r2d2::Pool::builder()
        .max_size(4)
        .build(manager)
        .expect("build pool");
    let runs_store = KpiIngestRunsStore::new(Database::from_pool(pool.clone()));
    runs_store
        .claim_next(TEST_HOLDER, 3600)
        .expect("claim")
        .expect("run1 must be claimable");
    let store = Arc::new(KpiIngestStagingStore::new(Database::from_pool(pool)));

    // Barrier so both contenders genuinely start together (luna review:
    // without it the threads may serialize before either call begins).
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let store_a = store.clone();
    let store_b = store.clone();
    let barrier_a = barrier.clone();
    let barrier_b = barrier;
    let a = std::thread::spawn(move || {
        barrier_a.wait();
        store_a.stage_observations(
            "run1",
            TEST_HOLDER,
            vec![one_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
    });
    let b = std::thread::spawn(move || {
        barrier_b.wait();
        store_b.stage_observations(
            "run1",
            TEST_HOLDER,
            vec![one_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
    });
    let result_a = a.join().expect("thread a");
    let result_b = b.join().expect("thread b");

    let winners = [&result_a, &result_b]
        .iter()
        .filter(|result| result.is_ok())
        .count();
    assert_eq!(
        winners, 1,
        "exactly one thread must win the single stageable run"
    );
    let losers = [&result_a, &result_b]
        .iter()
        .filter(|result| result.is_err())
        .count();
    assert_eq!(losers, 1);
    for result in [&result_a, &result_b] {
        if let Err(error) = result {
            assert!(matches!(
                error,
                StorageError::InvalidRunStateForStaging { .. }
            ));
        }
    }

    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("sqlite3-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("sqlite3-shm"));
}

// --- Test 9: empty snapshot, raw round-trip -----------------------------

#[test]
fn stage_observations_rejects_an_empty_snapshot() {
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();
    let error = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect_err("empty batch");
    assert!(matches!(
        error,
        StorageError::InvalidKpiIngestRunValue {
            key: "observations",
            ..
        }
    ));
}

#[test]
fn stage_observations_round_trips_raw_currency_and_unit_scale() {
    let (state, run_id) = setup();
    let store = state.kpi_ingest_staging();
    let mut observation = one_observation();
    observation.raw_currency = Some("PLN".to_owned());
    observation.raw_unit_scale = Some("tys. zł".to_owned());
    let (_, observations) = store
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![observation],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("stage");
    assert_eq!(observations[0].raw_currency.as_deref(), Some("PLN"));
    assert_eq!(observations[0].raw_unit_scale.as_deref(), Some("tys. zł"));
}

// --- #386: missingReasons + execution travel in the staging tx --------

#[test]
fn staging_writes_missing_reasons_with_replace_semantics() {
    let (state, run_id) = setup();
    let reasons: std::collections::BTreeMap<String, String> =
        [("net_profit".to_owned(), "not disclosed".to_owned())].into();
    state
        .kpi_ingest_staging()
        .stage_observations(run_id, TEST_HOLDER, vec![one_observation()], &reasons, None)
        .expect("stage");
    let run = state
        .kpi_ingest_runs()
        .get_run(run_id)
        .expect("get")
        .expect("run");
    assert_eq!(
        run.missing_reasons_json.as_deref(),
        Some(r#"{"net_profit":"not disclosed"}"#),
        "the declaration lands in the SAME staging transaction"
    );

    // A repair revision with `{}` is the explicit clear — replace, never
    // a merge and never a destructive default.
    state.kpi_ingest_runs().invalidate_manifest(run_id).ok();
    let connection = state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "UPDATE kpi_ingest_runs SET status = 'validation_failed' WHERE id = ?1",
            [run_id],
        )
        .expect("force repairable state");
    drop(connection);
    state
        .kpi_ingest_staging()
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![one_observation()],
            &std::collections::BTreeMap::new(),
            None,
        )
        .expect("re-stage");
    let run = state
        .kpi_ingest_runs()
        .get_run(run_id)
        .expect("get")
        .expect("run");
    assert_eq!(
        run.missing_reasons_json.as_deref(),
        Some("{}"),
        "an empty map clears the previous declaration"
    );
}

#[test]
fn staging_merges_execution_into_cost_json_and_corrupt_stored_json_is_replaced() {
    let (state, run_id) = setup();
    // Corrupt pre-existing cost_json: diagnostic, so the merge replaces
    // it fresh instead of failing the stage.
    {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET cost_json = 'not json' WHERE id = ?1",
                [run_id],
            )
            .expect("corrupt");
    }
    let execution = serde_json::json!({ "client": "test-agent", "tokensIn": 5 });
    state
        .kpi_ingest_staging()
        .stage_observations(
            run_id,
            TEST_HOLDER,
            vec![one_observation()],
            &std::collections::BTreeMap::new(),
            Some(&execution),
        )
        .expect("stage");
    let run = state
        .kpi_ingest_runs()
        .get_run(run_id)
        .expect("get")
        .expect("run");
    let cost: serde_json::Value =
        serde_json::from_str(run.cost_json.as_deref().expect("cost_json written"))
            .expect("valid json");
    assert_eq!(cost["schemaVersion"], 1);
    assert_eq!(cost["client"], "test-agent");
    assert_eq!(cost["tokensIn"], 5);
}
