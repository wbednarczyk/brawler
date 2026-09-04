use super::*;
use crate::app_state::AppState;
use crate::jobs::handlers::build_worker;
use crate::jobs::kpi_ingest_queue::KPI_INGEST_VALIDATE_KIND;
use crate::storage::{
    open_in_memory_database, CaptureReportDocumentInput, NewCompany, NewKpiIngestRun,
};

fn state() -> AppState {
    AppState::new(open_in_memory_database().expect("db"))
}

fn company(state: &AppState, ticker: &str) -> String {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: ticker.to_owned(),
            display_name: format!("{ticker} S.A."),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company")
        .id
}

fn document(state: &AppState, company_id: &str, title: &str) -> String {
    state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company_id.to_owned(),
            source_type: "official_report".to_owned(),
            url: format!("https://example.test/{title}.pdf"),
            period_id: None,
            origin_ref: None,
            title: Some(title.to_owned()),
            attribution: None,
        })
        .expect("document")
        .id
}

/// One `(kind, job_id, payload)` per registered kind, with whatever seed rows
/// its identity resolution joins against — the real payload shape each
/// handler actually enqueues (ADR 0109 dec. 1 gate).
fn representative_payloads(state: &AppState) -> Vec<(&'static str, String, String)> {
    use crate::jobs::aggregator_fundamentals_pull::AGGREGATOR_FUNDAMENTALS_PULL_KIND;
    use crate::jobs::autopilot::AUTOPILOT_STAGE_KIND;
    use crate::jobs::backfill::COMPANY_BACKFILL_KIND;
    use crate::jobs::fx_daily_pull::FX_DAILY_PULL_KIND;
    use crate::jobs::history_sweep::HISTORY_SWEEP_KIND;
    use crate::jobs::kpi_ingest_queue::{KPI_INGEST_COMMIT_KIND, KPI_INGEST_VALIDATE_KIND};
    use crate::jobs::management_holdings_extraction::MANAGEMENT_EXTRACTION_KIND;
    use crate::jobs::morning_briefing::MORNING_BRIEFING_KIND;
    use crate::jobs::ownership_extraction::OWNERSHIP_EXTRACTION_KIND;
    use crate::jobs::pipeline_reextraction::PIPELINE_REEXTRACTION_KIND;
    use crate::jobs::quote_backfill::QUOTE_BACKFILL_KIND;
    use crate::jobs::scheduler::{REGISTRY_REFRESH_KIND, SOURCE_REFRESH_KIND};
    use crate::jobs::source_refresh::SOURCE_COMPANY_REFRESH_KIND;

    let cdr = company(state, "CDR");
    let doc = document(state, &cdr, "Raport Q2 2026");

    let sweep = state
        .history_sweeps()
        .create_history_sweep(&cdr, "manual")
        .expect("sweep");

    let batch = state
        .pipeline_reextraction()
        .create_batch(&cdr)
        .expect("batch");

    let run = state
        .autopilot()
        .create_run_if_absent(
            &format!("run:{doc}"),
            &cdr,
            &doc,
            "detection",
            "autopilot",
            None,
        )
        .expect("create run")
        .expect("run created");

    let kpi_doc = document(state, &cdr, "Raport roczny 2025");
    let kpi_run = state
        .kpi_ingest_runs()
        .create_run_if_absent(&NewKpiIngestRun {
            report_document_id: kpi_doc.clone(),
            company_id: cdr.clone(),
            period_id: None,
            profile_version: "gpw_ifrs_annual@v1".to_owned(),
            scope: None,
            data_quality: None,
            period_fiscal_year: None,
            period_type: None,
        })
        .expect("kpi run");

    vec![
        (
            SOURCE_REFRESH_KIND,
            "src:1".to_owned(),
            r#"{"adapterId":"gpw-espi-ebi"}"#.to_owned(),
        ),
        (
            SOURCE_COMPANY_REFRESH_KIND,
            "src-co:1".to_owned(),
            format!(r#"{{"adapterId":"bankier-company","companyId":"{cdr}"}}"#),
        ),
        (
            REGISTRY_REFRESH_KIND,
            REGISTRY_REFRESH_KIND.to_owned(),
            r#"{"staleAfterSeconds":86400}"#.to_owned(),
        ),
        (
            FX_DAILY_PULL_KIND,
            FX_DAILY_PULL_KIND.to_owned(),
            "{}".to_owned(),
        ),
        (
            AGGREGATOR_FUNDAMENTALS_PULL_KIND,
            AGGREGATOR_FUNDAMENTALS_PULL_KIND.to_owned(),
            "{}".to_owned(),
        ),
        (
            MORNING_BRIEFING_KIND,
            MORNING_BRIEFING_KIND.to_owned(),
            r#"{"force":false}"#.to_owned(),
        ),
        (
            COMPANY_BACKFILL_KIND,
            format!("{COMPANY_BACKFILL_KIND}:{cdr}"),
            format!(r#"{{"companyId":"{cdr}"}}"#),
        ),
        (
            QUOTE_BACKFILL_KIND,
            format!("{QUOTE_BACKFILL_KIND}:{cdr}"),
            format!(r#"{{"companyId":"{cdr}"}}"#),
        ),
        (
            HISTORY_SWEEP_KIND,
            "sweep-job:1".to_owned(),
            format!(r#"{{"sweep_id":"{}"}}"#, sweep.id),
        ),
        (
            PIPELINE_REEXTRACTION_KIND,
            "batch-job:1".to_owned(),
            format!(r#"{{"batch_id":"{}"}}"#, batch.id),
        ),
        (
            OWNERSHIP_EXTRACTION_KIND,
            format!("{OWNERSHIP_EXTRACTION_KIND}:{doc}"),
            format!(r#"{{"companyId":"{cdr}","reportDocumentId":"{doc}"}}"#),
        ),
        (
            MANAGEMENT_EXTRACTION_KIND,
            format!("{MANAGEMENT_EXTRACTION_KIND}:{doc}"),
            format!(r#"{{"companyId":"{cdr}","reportDocumentId":"{doc}"}}"#),
        ),
        (
            AUTOPILOT_STAGE_KIND,
            "autopilot-job:1".to_owned(),
            format!(r#"{{"run_id":"{}","stage":"fetch"}}"#, run.id),
        ),
        (
            KPI_INGEST_VALIDATE_KIND,
            // sol diff R1 #9/#17: the REAL production job id shape
            // (`kpi_ingest_queue::validate_job_id`), never an arbitrary
            // fixture id — `run_id` is now parsed FROM this id, so a fixture
            // id that doesn't conform resolves `Corrupted`, not `KpiIngest`.
            format!("{KPI_INGEST_VALIDATE_KIND}:{}:rev1", kpi_run.id),
            format!(r#"{{"jobId":"x","runId":"{}","revision":1}}"#, kpi_run.id),
        ),
        (
            KPI_INGEST_COMMIT_KIND,
            format!("{KPI_INGEST_COMMIT_KIND}:{}:rev1:h", kpi_run.id),
            format!(
                r#"{{"jobId":"x","runId":"{}","revision":1,"manifestHash":"h"}}"#,
                kpi_run.id
            ),
        ),
    ]
}

#[test]
fn every_registered_kind_has_an_activity_identity() {
    // ADR 0109 dec. 1 gate: every kind `build_worker` registers must resolve to
    // an identity with a real family (never Corrupted) given a representative
    // real payload — the same enumeration pattern as `jobs::failure_surface`.
    let state = state();
    let worker = build_worker(state.clone());
    let registered: std::collections::BTreeSet<&str> =
        worker.registered_kinds().into_iter().collect();

    let payloads = representative_payloads(&state);
    let covered: std::collections::BTreeSet<&str> =
        payloads.iter().map(|(kind, _, _)| *kind).collect();
    assert_eq!(
        registered, covered,
        "every registered kind must have a representative payload in this gate"
    );

    let connection = state.checkout_for_tests().expect("checkout");
    for (kind, job_id, payload) in &payloads {
        let identity = identity_for_job(kind, job_id, payload, &connection);
        assert!(
            identity.is_some(),
            "kind {kind} must resolve to an identity"
        );
        assert_ne!(
            identity.unwrap().family,
            ActivityFamily::Corrupted,
            "kind {kind} must not resolve to Corrupted given a real payload"
        );
    }
}

#[test]
fn identity_resolves_every_real_payload_shape() {
    // Spot-check the resolved activity_key / company scoping per kind — the
    // real payload JSON each handler actually enqueues (D1's key scheme).
    let state = state();
    let payloads = representative_payloads(&state);
    let by_kind: std::collections::HashMap<&str, (String, String)> = payloads
        .into_iter()
        .map(|(kind, job_id, payload)| (kind, (job_id, payload)))
        .collect();

    let connection = state.checkout_for_tests().expect("checkout");

    let (job_id, payload) = &by_kind["scheduled_source_refresh"];
    let identity =
        identity_for_job("scheduled_source_refresh", job_id, payload, &connection).unwrap();
    assert_eq!(identity.activity_key, "source-refresh:gpw-espi-ebi");
    assert_eq!(identity.family, ActivityFamily::SourceRefresh);
    assert_eq!(identity.target, ActivityTarget::Sources);

    let (job_id, payload) = &by_kind["history_sweep"];
    let identity = identity_for_job("history_sweep", job_id, payload, &connection).unwrap();
    assert!(identity.activity_key.starts_with("report-sweep:"));
    assert_eq!(identity.family, ActivityFamily::ReportSweep);
    assert!(identity.company_id.is_some());

    let (job_id, payload) = &by_kind["autopilot_stage"];
    let identity = identity_for_job("autopilot_stage", job_id, payload, &connection).unwrap();
    assert!(identity.activity_key.starts_with("report-reading:"));
    assert_eq!(identity.family, ActivityFamily::ReportReading);

    let (job_id, payload) = &by_kind["kpi_ingest_validate"];
    let identity = identity_for_job("kpi_ingest_validate", job_id, payload, &connection).unwrap();
    assert!(identity.activity_key.starts_with("kpi-ingest:"));
    assert_eq!(identity.family, ActivityFamily::KpiIngest);
}

#[test]
fn malformed_payload_yields_corrupted_item_not_silence() {
    // A REGISTERED kind with an unparseable/incomplete payload is an explicit
    // Corrupted item, never a silent drop (ADR 0109 dec. 1).
    let state = state();
    let connection = state.checkout_for_tests().expect("checkout");
    let identity =
        identity_for_job("scheduled_source_refresh", "job-x", "{}", &connection).expect("some");
    assert_eq!(identity.family, ActivityFamily::Corrupted);
    assert_eq!(identity.subject, "job-x");
}

#[test]
fn job_kind_list_matches_registry() {
    // ADR 0109 dec. item 9 (Today label parity): `src/shared/formatting/jobKinds.ts`
    // is hand-maintained; this reads it from disk and asserts set equality
    // with the REAL registry (`registered_kinds()`), so a new/retired kind
    // reddens here until the TS list (and its `formatJobKindDisplayName`
    // label) is updated.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("src/shared/formatting/jobKinds.ts");
    let contents = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let ts_kinds: std::collections::BTreeSet<String> = contents
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let inner = line.strip_prefix('"')?;
            let (value, _) = inner.split_once('"')?;
            Some(value.to_owned())
        })
        .collect();

    let state = state();
    let registered: std::collections::BTreeSet<String> = build_worker(state)
        .registered_kinds()
        .into_iter()
        .map(str::to_owned)
        .collect();

    assert_eq!(
        ts_kinds, registered,
        "jobKinds.ts must list exactly the registered queue kinds"
    );
}

#[test]
fn kpi_identity_is_authoritative_from_the_job_id_never_a_tampered_payload() {
    // sol diff R1 #9: `run_id` must come from the CLAIMED job id
    // (`kpi_ingest_queue::parse_job_id`), never the payload's duplicated
    // `runId` — a payload whose `runId` disagrees with the id must still
    // resolve to the ID's run, never the payload's (a tampered/stale
    // payload could otherwise misattribute the occurrence to the wrong run
    // before preflight validation ever runs).
    let state = state();
    let cdr = company(&state, "CDR");
    let real_doc = document(&state, &cdr, "Prawdziwy raport");
    let real_run = state
        .kpi_ingest_runs()
        .create_run_if_absent(&NewKpiIngestRun {
            report_document_id: real_doc,
            company_id: cdr.clone(),
            period_id: None,
            profile_version: "gpw_ifrs_annual@v1".to_owned(),
            scope: None,
            data_quality: None,
            period_fiscal_year: None,
            period_type: None,
        })
        .expect("real kpi run");
    let decoy_doc = document(&state, &cdr, "Zwodniczy raport");
    let decoy_run = state
        .kpi_ingest_runs()
        .create_run_if_absent(&NewKpiIngestRun {
            report_document_id: decoy_doc,
            company_id: cdr,
            period_id: None,
            profile_version: "gpw_ifrs_annual@v1".to_owned(),
            scope: None,
            data_quality: None,
            period_fiscal_year: None,
            period_type: None,
        })
        .expect("decoy kpi run");

    let connection = state.checkout_for_tests().expect("checkout");
    let job_id = format!("{KPI_INGEST_VALIDATE_KIND}:{}:rev1", real_run.id);
    // Tampered payload: `runId` names the DECOY run, not the claimed job's.
    let payload = format!(r#"{{"jobId":"x","runId":"{}","revision":1}}"#, decoy_run.id);
    let identity = identity_for_job(KPI_INGEST_VALIDATE_KIND, &job_id, &payload, &connection)
        .expect("identity");

    assert_eq!(
        identity.activity_key,
        format!("kpi-ingest:{}", real_run.id),
        "the identity must come from the CLAIMED job id, never the payload's runId"
    );
    assert_ne!(
        identity.activity_key,
        format!("kpi-ingest:{}", decoy_run.id)
    );
}

#[test]
fn kpi_identity_from_a_malformed_job_id_is_corrupted_not_the_payloads_run() {
    // The flip side: a job id that does not match the production
    // `kpi_ingest_validate:{run}:rev{N}` shape must resolve `Corrupted` —
    // never silently fall back to trusting the payload's `runId`.
    let state = state();
    let connection = state.checkout_for_tests().expect("checkout");
    let identity = identity_for_job(
        KPI_INGEST_VALIDATE_KIND,
        "not-a-real-kpi-job-id",
        r#"{"jobId":"x","runId":"kpiing_deadbeef","revision":1}"#,
        &connection,
    )
    .expect("identity");
    assert_eq!(identity.family, ActivityFamily::Corrupted);
}

#[test]
fn briefing_subject_is_never_composed_prose() {
    // sol diff R1 #17: the briefing family had a fixed Polish string baked
    // into the backend (`"Poranny przegląd"`) — composed prose, untranslated
    // for an English user, and contrary to contracts.md's raw-subject rule.
    // The family has no raw subject of its own; the frontend renders the
    // family label instead.
    let state = state();
    let connection = state.checkout_for_tests().expect("checkout");
    let identity = identity_for_job(
        crate::jobs::morning_briefing::MORNING_BRIEFING_KIND,
        "briefing",
        "{}",
        &connection,
    )
    .expect("identity");
    assert_eq!(identity.family, ActivityFamily::Briefing);
    assert_eq!(identity.subject, "");
}

#[test]
fn unregistered_kind_yields_none() {
    // A retired kind's residue rows are excluded, never surfaced as an item.
    let state = state();
    let connection = state.checkout_for_tests().expect("checkout");
    assert!(identity_for_job("qualitative_assessment", "job-x", "{}", &connection).is_none());
}

#[test]
fn registry_refresh_subject_is_never_composed_prose() {
    // sol diff R2 #6 (backend): the composed Polish subject "Rejestr spółek
    // GPW/NewConnect" appeared untranslated in the English UI — empty now,
    // like the briefing/system subjects; the family label carries the prose.
    let state = state();
    let connection = state.checkout_for_tests().expect("checkout");
    let identity = identity_for_job(
        crate::jobs::scheduler::REGISTRY_REFRESH_KIND,
        "registry-refresh",
        "{}",
        &connection,
    )
    .expect("identity");
    assert_eq!(identity.subject, "");
    assert_eq!(identity, registry_refresh_identity());
}

#[test]
fn direct_wrapper_fallback_identities_need_no_connection() {
    // sol diff R2 #4: `refresh_source_direct`/`sweep_adapters`,
    // `refresh_company_directories_direct`, and
    // `run_aggregator_fundamentals_pull_direct` now build their identity via
    // a function that takes NO `&Connection` parameter at all — the type
    // signature itself guarantees registration never again silently depends
    // on a checkout succeeding. Cross-check each fallback against the
    // connection-backed `identity_for_job` result for the same input, which
    // must agree exactly (these functions ARE what `identity_for_job`
    // delegates to for these kinds).
    let state = state();
    let connection = state.checkout_for_tests().expect("checkout");

    assert_eq!(
        source_refresh_identity("gpw-espi-ebi"),
        identity_for_job(
            crate::jobs::scheduler::SOURCE_REFRESH_KIND,
            "direct:gpw-espi-ebi",
            r#"{"adapterId":"gpw-espi-ebi"}"#,
            &connection,
        )
        .expect("identity")
    );
    assert_eq!(
        registry_refresh_identity(),
        identity_for_job(
            crate::jobs::scheduler::REGISTRY_REFRESH_KIND,
            "direct:registry-refresh",
            "{}",
            &connection,
        )
        .expect("identity")
    );
    assert_eq!(
        aggregator_fundamentals_pull_identity(),
        identity_for_job(
            crate::jobs::aggregator_fundamentals_pull::AGGREGATOR_FUNDAMENTALS_PULL_KIND,
            "direct:fundamentals-pull",
            "{}",
            &connection,
        )
        .expect("identity")
    );
}

#[test]
fn company_backfill_fallback_uses_the_raw_company_id_as_subject() {
    // sol diff R2 #4: when no checkout is available to resolve a nicer
    // ticker subject, `backfill_company_history_direct` falls back to the
    // raw company id rather than skipping registration entirely.
    let identity = company_backfill_identity_fallback("company_gpw_cdr");
    assert_eq!(identity.activity_key, "history-fetch:company_gpw_cdr");
    assert_eq!(identity.family, ActivityFamily::HistoryFetch);
    assert_eq!(identity.subject, "company_gpw_cdr");
    assert_eq!(identity.company_id.as_deref(), Some("company_gpw_cdr"));
}
