use super::*;
use crate::storage::{
    open_in_memory_database, CaptureReportDocumentInput, NewCompany, MODE_AUTOPILOT,
};

// A minimal balanced ESEF/iXBRL instance (45m = 20m + 25m at 2026-03-31).
const ESEF: &str = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:ifrs-full="https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2026-03-31</xbrli:instant></xbrli:period></xbrli:context>
      <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" unitRef="pln" scale="3">45 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Liabilities" contextRef="c" unitRef="pln" scale="3">20 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="c" unitRef="pln" scale="3">25 000</ix:nonFraction>
    </html>"#;

/// A fetcher failing with a **real** `reqwest` network error (the only way
/// to construct the `Request` variant): an instant local connection
/// failure — hermetic, no external network.
struct TransientFailingFetcher;

impl crate::document_fetcher::DocumentFetcher for TransientFailingFetcher {
    fn fetch(
        &self,
        _url: &str,
    ) -> Result<
        crate::document_fetcher::FetchedDocument,
        crate::document_fetcher::DocumentFetcherError,
    > {
        let error = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_millis(250))
            .build()
            .expect("client builds")
            .get("http://127.0.0.1:9/refused")
            .send()
            .expect_err("nothing listens on the discard port");
        Err(crate::document_fetcher::DocumentFetcherError::Request(
            error,
        ))
    }
}

/// A run whose document is still pending fetch (no stored file), so the
/// fetch stage must go through the injected fetcher.
fn pending_fetch_run(state: &AppState, run_id: &str) {
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "TST".to_owned(),
            display_name: "Transient Test S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://example.com/report.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Pending report".to_owned()),
            attribution: None,
        })
        .expect("document");
    state
        .autopilot()
        .create_run_if_absent(
            run_id,
            &company.id,
            &document.id,
            "manual",
            MODE_AUTOPILOT,
            None,
        )
        .expect("create run")
        .expect("run created");
}

fn fetch_payload(run_id: &str) -> String {
    format!(r#"{{"run_id":"{run_id}","stage":"{STAGE_FETCH}"}}"#)
}

/// #189 / ADR 0055 dec. 2: a network-level fetch failure with attempts left
/// returns `Err` (the durable queue retries with backoff) and does NOT
/// finalize the run — and the stage job is armed with retries at all.
#[test]
fn transient_fetch_failure_goes_back_to_the_queue_and_keeps_the_run_alive() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let run_id = "run_transient";
    pending_fetch_run(&state, run_id);
    let _ = enqueue_stage(&state, run_id, STAGE_FETCH);
    let job = state.jobs().claim_next().expect("claim").expect("a job");
    assert_eq!(job.kind, AUTOPILOT_STAGE_KIND);
    let row = state
        .jobs()
        .status(&stage_job_id(run_id, STAGE_FETCH))
        .expect("status")
        .expect("job row");
    assert_eq!(
        row.max_attempts, STAGE_MAX_ATTEMPTS,
        "stage jobs must arm queue retries (reddens on a revert to 1)"
    );

    let result = run_stage_with_fetcher(&state, &fetch_payload(run_id), &TransientFailingFetcher);

    assert!(
        result.is_err(),
        "transient failure must go back to the queue"
    );
    let run = state.autopilot().get_run(run_id).expect("run");
    assert_ne!(run.status, "failed", "run must stay alive for the retry");
}

/// #189: the last allowed attempt of a transient failure finalizes the run
/// as failed (still notified) instead of stranding it over a dead job.
#[test]
fn transient_fetch_failure_on_the_last_attempt_finalizes_the_run() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let run_id = "run_exhausted";
    pending_fetch_run(&state, run_id);
    let _ = enqueue_stage(&state, run_id, STAGE_FETCH);
    // Burn all but the last attempt the way the worker would.
    for _ in 0..(STAGE_MAX_ATTEMPTS - 1) {
        let job = state.jobs().claim_next().expect("claim").expect("a job");
        assert!(
            state
                .jobs()
                .mark_failed(&job.id, "transient blip", 0)
                .expect("mark failed"),
            "non-final attempts stay retryable"
        );
    }
    let _last = state.jobs().claim_next().expect("claim").expect("a job");

    let result = run_stage_with_fetcher(&state, &fetch_payload(run_id), &TransientFailingFetcher);

    assert!(result.is_ok(), "exhausted attempt must not loop the job");
    let run = state.autopilot().get_run(run_id).expect("run");
    assert_eq!(run.status, "failed", "the run is honestly finalized");
}

/// #189: a fatal (non-network) fetch failure still finalizes immediately,
/// even with queue attempts remaining — retrying a domain failure is waste.
#[test]
fn fatal_fetch_failure_finalizes_immediately_despite_remaining_attempts() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let run_id = "run_fatal";
    pending_fetch_run(&state, run_id);
    let _ = enqueue_stage(&state, run_id, STAGE_FETCH);
    let _job = state.jobs().claim_next().expect("claim").expect("a job");

    let fetcher = crate::document_fetcher::FakeDocumentFetcher::new_error(
        crate::document_fetcher::DocumentFetcherError::InvalidContentType("boom".to_owned()),
    );
    let result = run_stage_with_fetcher(&state, &fetch_payload(run_id), &fetcher);

    assert!(result.is_ok(), "fatal failure must not be retried");
    let run = state.autopilot().get_run(run_id).expect("run");
    assert_eq!(run.status, "failed");
}

#[test]
fn last_attempt_exhaustion_matrix() {
    assert!(!last_attempt_exhausted(1, STAGE_MAX_ATTEMPTS));
    assert!(!last_attempt_exhausted(2, STAGE_MAX_ATTEMPTS));
    assert!(last_attempt_exhausted(3, STAGE_MAX_ATTEMPTS));
}

/// Fault injection (issue #458, docs/testing.md § Failure-path tests): the
/// first stage's own job insert is poisoned. Pre-fix, `enqueue_first_stage`
/// swallowed the error as a warn-only log — the run was left `pending` with
/// no job ever able to drive it (permanently stranded; no reconcile rule
/// resurrects a run whose stage job never existed at all under the OLD
/// "no live job -> failed" rule alone, since that rule only runs at the
/// NEXT startup). `enqueue_extraction_run` must report `Failed`, not
/// `Created`, and the run must be finalized `failed` with a typed
/// `last_error` naming the stage.
#[test]
fn a_first_stage_enqueue_failure_creates_the_run_failed_with_a_typed_reason() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "FST".to_owned(),
            display_name: "First Stage Test S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://example.com/first-stage.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("First stage report".to_owned()),
            attribution: None,
        })
        .expect("document");

    let connection = state.checkout_for_tests().expect("checkout");
    connection
        .execute_batch(
            "CREATE TRIGGER poison_first_stage_enqueue BEFORE INSERT ON job_queue
                 WHEN NEW.kind = 'autopilot_stage' AND NEW.id LIKE '%:fetch'
                 BEGIN SELECT RAISE(ABORT, 'fetch stage poisoned for test'); END;",
        )
        .expect("install poison trigger");
    drop(connection); // avoid a pool deadlock across the call below (#360/#376)

    let outcome = enqueue_extraction_run(
        &state,
        &company.id,
        &document.id,
        "manual",
        MODE_AUTOPILOT,
        None,
    );
    assert_eq!(outcome, EnqueueExtractionOutcome::Failed);

    let run_id = format!("autopilot_run:{}:{}", company.id, document.id);
    let run = state.autopilot().get_run(&run_id).expect("run row exists");
    assert_eq!(run.status, "failed");
    assert!(
            run.last_error.as_deref().is_some_and(
                |error| error.contains("fetch") && error.contains("could not be enqueued")
            ),
            "last_error must name the stage: {:?}",
            run.last_error
        );
}

/// Fault injection (issue #458): the FIRST stage's own enqueue succeeds and
/// genuinely runs (an already-fetched document, so `stage_fetch` is a
/// no-op success), but the hand-off to `extract` is poisoned. Pre-fix, the
/// hand-off's `enqueue_stage` call was fire-and-forget (return value
/// discarded) — the run was left `running` with no live job under it,
/// permanently stranded until the NEXT startup's reconcile pass (and only
/// then honestly failed). Now the run must be finalized `failed` in the
/// SAME pass, with a typed `last_error` naming the stage — while the
/// `fetch` stage job itself still reports its own genuine success.
#[test]
fn a_hand_off_enqueue_failure_fails_the_run_with_a_typed_reason() {
    let dir = unique_temp_dir("handoff-poison");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let (company_id, document_id) = seed_pdf_report(
        &state,
        &dir,
        "Raport Q1 2026",
        &["Test line for hand-off poison"],
    );

    let run_id = "run_handoff_poison";
    state
        .autopilot()
        .create_run_if_absent(
            run_id,
            &company_id,
            &document_id,
            "manual",
            MODE_AUTOPILOT,
            None,
        )
        .expect("create run")
        .expect("run created");

    let connection = state.checkout_for_tests().expect("checkout");
    connection
        .execute_batch(
            "CREATE TRIGGER poison_handoff_extract BEFORE INSERT ON job_queue
                 WHEN NEW.kind = 'autopilot_stage' AND NEW.id LIKE '%:extract'
                 BEGIN SELECT RAISE(ABORT, 'extract hand-off poisoned for test'); END;",
        )
        .expect("install poison trigger");
    drop(connection); // avoid a pool deadlock across the queue drive below (#360/#376)

    enqueue_first_stage(&state, run_id);
    crate::jobs::handlers::build_worker(state.clone())
        .run_until_idle()
        .expect("drain the queue");

    let run = state.autopilot().get_run(run_id).expect("get run");
    assert_eq!(
        run.status, "failed",
        "the run must be finalized failed, not left dangling with no live job"
    );
    assert!(
        run.last_error.as_deref().is_some_and(
            |error| error.contains("extract") && error.contains("could not be enqueued")
        ),
        "last_error must name the stage: {:?}",
        run.last_error
    );
    let fetch_job = state
        .jobs()
        .status(&stage_job_id(run_id, STAGE_FETCH))
        .expect("status")
        .expect("fetch job row exists");
    assert_eq!(
        fetch_job.status, "succeeded",
        "the fetch stage's own job genuinely succeeded; only the hand-off enqueue failed"
    );
}

/// ADR 0061: in autopilot mode a tagged ESEF filing is extracted
/// deterministically before AI — facts land `confirmed` (review-free, ADR
/// 0086 dec. 5) with `esef`/`passed` provenance, and the AI path is skipped.
#[test]
fn autopilot_esef_uses_structured_extraction_and_skips_ai() {
    let dir = std::env::temp_dir().join(format!("brawler-autopilot-esef-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CBF".to_owned(),
            display_name: "Cyber_Folks S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://example.com/annual-2026.xhtml".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Annual 2026 ESEF".to_owned()),
            attribution: None,
        })
        .expect("document");
    // A package, not a bare instance: a bare iXBRL instance has no
    // presentation linkbase to read, so its facts get no role rows and
    // never survive Layer 2 projection's primary-statement role filter
    // (ADR 0100 decision 3, epic #398) — every real GPW filing ships as a
    // package anyway.
    let pre_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<link:linkbase xmlns:link="http://www.xbrl.org/2003/linkbase" xmlns:xlink="http://www.w3.org/1999/xlink">
  <link:presentationLink xlink:type="extended" xlink:role="http://x/role/ias_1_role-210000">
    <link:loc xlink:type="locator" xlink:href="ifrs-full-2023.xsd#ifrs-full_Assets" xlink:label="loc_assets"/>
  </link:presentationLink>
  <link:presentationLink xlink:type="extended" xlink:role="http://x/role/ias_1_role-210000">
    <link:loc xlink:type="locator" xlink:href="ifrs-full-2023.xsd#ifrs-full_Liabilities" xlink:label="loc_liabilities"/>
  </link:presentationLink>
  <link:presentationLink xlink:type="extended" xlink:role="http://x/role/ias_1_role-210000">
    <link:loc xlink:type="locator" xlink:href="ifrs-full-2023.xsd#ifrs-full_Equity" xlink:label="loc_equity"/>
  </link:presentationLink>
</link:linkbase>"#;
    let bytes = {
        use std::io::Write;
        let mut buf = Vec::new();
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("reports/annual-2026.xhtml", opts)
            .expect("start instance entry");
        zip.write_all(ESEF.as_bytes()).expect("write instance");
        zip.start_file("www/annual-2026_pre.xml", opts)
            .expect("start pre.xml entry");
        zip.write_all(pre_xml.as_bytes()).expect("write pre.xml");
        zip.finish().expect("finish zip");
        buf
    };
    std::fs::write(dir.join("annual.xhtml"), &bytes).expect("write esef");
    state
        .mark_report_document_fetched(
            &document.id,
            Some("annual.xhtml"),
            // Extension lies on purpose (card eb71488): routing reads the
            // ZIP magic bytes, never the content-type/filename.
            Some("application/octet-stream"),
            None,
            Some(bytes.len() as i64),
        )
        .expect("mark fetched");

    let run_id = "run_esef";
    state
        .autopilot()
        .create_run_if_absent(
            run_id,
            &company.id,
            &document.id,
            "manual",
            MODE_AUTOPILOT,
            None,
        )
        .expect("create run")
        .expect("run created");
    let run = state.autopilot().get_run(run_id).expect("get run");

    stage_extract(&state, &run).expect("extract stage");

    let after = state.autopilot().get_run(run_id).expect("get run");
    assert!(
        !after.produced_fact_ids.is_empty(),
        "structured ESEF facts should be auto-committed"
    );
    let delta = after.kpi_delta_json.clone().expect("kpi delta recorded");
    assert!(delta.contains("\"structured\":true"), "delta: {delta}");
    assert!(
        delta.contains("esef"),
        "delta should name the tier: {delta}"
    );
    assert!(
        delta.contains("\"factsProposed\":3"),
        "delta should carry the normalized proposed count: {delta}"
    );
    assert!(
        delta.contains("\"factsAutoConfirmed\":3"),
        "a validation-clean ESEF set auto-confirms all 3 facts: {delta}"
    );

    let provenance = state
        .fundamentals_provenance()
        .get_many(&after.produced_fact_ids)
        .expect("provenance");
    assert_eq!(provenance.len(), after.produced_fact_ids.len());
    assert!(provenance
        .iter()
        .all(|p| p.source_tier == "esef" && p.validation_status == "passed"));

    // Review-free (ADR 0086 dec. 5): every emitted fact lands `confirmed` —
    // no `auto_unreviewed`/`pending` awaiting-confirmation state survives.
    let facts = state
        .list_financial_facts(storage::ListFinancialFactsInput {
            company_id: Some(company.id.clone()),
            period_id: None,
            definition_id: None,
        })
        .expect("list facts");
    assert!(
        facts
            .iter()
            .filter(|f| after.produced_fact_ids.contains(&f.id))
            .all(|f| f.confirmation_state == "confirmed"),
        "facts: {facts:?}"
    );

    // The composed notification summary must reflect the honest count.
    let summary = compose_summary(&after);
    assert!(
        summary.contains("kpi_confirmed:3:3"),
        "summary must carry the honest typed count token: {summary}"
    );
}

/// Regression for `compose_summary`, isolated from the full
/// structured-extraction pipeline: a structured-tier delta shaped like a
/// live report (40 facts stored, `autopilot` mode) must summarize with the
/// real count.
#[test]
fn compose_summary_reports_honest_counts_for_a_structured_tier_delta() {
    let connection = open_in_memory_database().expect("db");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CBF".to_owned(),
            display_name: "Cyber_Folks S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let run_id = "run_structured_counts";
    state
        .autopilot()
        .create_run_if_absent(run_id, &company.id, "doc1", "manual", MODE_AUTOPILOT, None)
        .expect("create run")
        .expect("run created");
    let delta = serde_json::json!({
        "extractionAvailable": true,
        "structured": true,
        "tier": "esef",
        "produced": 40,
        "factsProposed": 40,
        "factsAutoConfirmed": 40,
        "mode": MODE_AUTOPILOT,
    });
    state
        .autopilot()
        .set_kpi_delta_json(run_id, &delta.to_string())
        .expect("set delta");
    let run = state.autopilot().get_run(run_id).expect("get run");

    let summary = compose_summary(&run);
    assert!(
        summary.contains("kpi_confirmed:40:40"),
        "summary must carry the honest typed count token: {summary}"
    );
    // ADR 0084 dec 6: no user-visible English prose in the stored summary.
    assert!(
        !summary.contains("auto-confirmed"),
        "English prose leaked into the typed summary: {summary}"
    );
}

/// ADR 0084 decision 6 (completion) — `compose_summary` emits a **typed token
/// stream** for every fragment, not just the extraction-unavailable branch.
/// A run with KPI counts, claims-to-verify and open questions must serialize
/// as machine tokens the frontend translates, with NO user-visible English
/// prose reaching the stored summary.
#[test]
fn compose_summary_emits_only_typed_tokens_with_no_english_prose() {
    let connection = open_in_memory_database().expect("db");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "TOK".to_owned(),
            display_name: "Tokens S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let run_id = "run_all_tokens";
    state
        .autopilot()
        .create_run_if_absent(
            run_id,
            &company.id,
            "docTok",
            "manual",
            MODE_AUTOPILOT,
            None,
        )
        .expect("create run")
        .expect("run created");
    state
        .autopilot()
        .set_kpi_delta_json(
            run_id,
            &serde_json::json!({
                "extractionAvailable": true,
                "structured": true,
                "factsProposed": 7,
                "factsAutoConfirmed": 7,
            })
            .to_string(),
        )
        .expect("set delta");
    state
        .autopilot()
        .set_cross_refs_json(
            run_id,
            &serde_json::json!({
                "claimsOverdue": 2,
                "claimsDue": 1,
                "openQuestions": 3,
                "expectationsToReview": 0,
            })
            .to_string(),
        )
        .expect("set cross refs");
    let run = state.autopilot().get_run(run_id).expect("get run");

    let summary = compose_summary(&run);
    // Every fragment is a typed token.
    assert!(summary.contains("kpi_confirmed:7:7"), "summary: {summary}");
    assert!(summary.contains("claims_to_verify:3"), "summary: {summary}");
    assert!(
        summary.contains("research_questions:3"),
        "summary: {summary}"
    );
    // No user-visible English prose fragment may reach the stored summary.
    for prose in [
        "auto-confirmed",
        "to verify",
        "open research question",
        "New report processed",
        "extracted",
    ] {
        assert!(
            !summary.contains(prose),
            "English prose {prose:?} leaked into the typed summary: {summary}"
        );
    }
}

/// ADR 0085 / C1 — a witness-fallback gap (the aggregator sourced the period
/// because no issuer tier could read the filing) must surface its OWN typed
/// code, never collapse into `no_deterministic_tier`. Before this the run's
/// notification lied about its cause.
#[test]
fn compose_summary_maps_witness_fallback_to_its_own_typed_code() {
    let connection = open_in_memory_database().expect("db");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "WFB".to_owned(),
            display_name: "Witness Fallback S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let run_id = "run_witness_fallback";
    state
        .autopilot()
        .create_run_if_absent(
            run_id,
            &company.id,
            "docWfb",
            "manual",
            MODE_AUTOPILOT,
            None,
        )
        .expect("create run")
        .expect("run created");
    state
        .autopilot()
        .set_kpi_delta_json(
            run_id,
            &serde_json::json!({
                "extractionAvailable": false,
                "reason": "witness_fallback",
            })
            .to_string(),
        )
        .expect("set delta");
    let run = state.autopilot().get_run(run_id).expect("get run");

    let summary = compose_summary(&run);
    assert!(
        summary.contains("kpi_extraction_unavailable:witness_fallback"),
        "witness fallback must surface its own typed code: {summary}"
    );
    assert!(
        !summary.contains("no_deterministic_tier"),
        "witness fallback must not collapse into no_deterministic_tier: {summary}"
    );
}

/// ADR 0084 decision 6 — honest failure reporting. The summary carries a
/// **typed reason code** from the fixed vocabulary, and distinct causes
/// stay distinguishable — never collapsed into one. Rendering the code
/// into a sentence is the frontend's job; the backend emits typed data.
#[test]
fn compose_summary_emits_typed_reason_codes_that_stay_distinguishable() {
    let connection = open_in_memory_database().expect("db");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "RSN".to_owned(),
            display_name: "Reason Codes S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");

    // (stored delta reason, expected typed code). The AI-era stored reasons
    // remain readable (ADR 0084 decision 5) and each maps to a typed code;
    // after the retirement the live one is `no_deterministic_tier`.
    let cases: [(&str, &str); 6] = [
        ("no_deterministic_tier", "no_deterministic_tier"),
        ("not_extractable", "no_deterministic_tier"),
        ("no_vision_provider", "provider_not_configured"),
        ("quota_exhausted", "quota_exhausted"),
        ("provider_error", "provider_error"),
        // The by-design PDF gap (ADR 0086 dec. 1) keeps its own code.
        ("pdf_document", "pdf_document"),
    ];

    let mut emitted: Vec<String> = Vec::new();
    for (index, (stored_reason, expected_code)) in cases.into_iter().enumerate() {
        let run_id = format!("run_reason_{index}");
        // Distinct document id per run: `create_run_if_absent` dedups on
        // (company, document), so reusing one id would return None after the
        // first insert.
        let document_id = format!("doc_{index}");
        state
            .autopilot()
            .create_run_if_absent(
                &run_id,
                &company.id,
                &document_id,
                "manual",
                MODE_AUTOPILOT,
                None,
            )
            .expect("create run")
            .expect("run created");
        let delta = serde_json::json!({
            "extractionAvailable": false,
            "reason": stored_reason,
        });
        state
            .autopilot()
            .set_kpi_delta_json(&run_id, &delta.to_string())
            .expect("set delta");
        let run = state.autopilot().get_run(&run_id).expect("get run");

        let summary = compose_summary(&run);
        assert!(
            summary.contains(expected_code),
            "stored reason {stored_reason} must surface the typed code \
                 {expected_code}, got: {summary}"
        );
        assert!(
            !summary.contains("no AI provider configured"),
            "the guessed English diagnosis must be gone, got: {summary}"
        );
        emitted.push(summary);
    }

    // The three distinct causes must not collapse into one string — that
    // collapse is the exact defect this test pins.
    assert_ne!(
        emitted[0], emitted[2],
        "a missing deterministic tier and an exhausted quota must stay distinguishable"
    );
    assert_ne!(
        emitted[2], emitted[3],
        "an exhausted quota and a provider error must stay distinguishable"
    );
    assert_ne!(
        emitted[1], emitted[3],
        "an unconfigured provider and a provider error must stay distinguishable"
    );
}

/// A per-call-unique scratch dir: a fixed pid-only dir would collide across
/// parallel `#[test]` threads and loop iterations sharing this file's data
/// dir (the same flakiness class fixed in `jobs::structured_extraction`'s
/// test module).
fn unique_temp_dir(label: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "brawler-autopilot-{}-{label}-{n}",
        std::process::id()
    ))
}

/// Builds a minimal, valid single-page PDF whose extracted text reproduces
/// `lines` (padded past `pdf-extract`'s 200-chars/page no-text-layer floor
/// with statement boilerplate) — see `jobs::structured_extraction`'s test
/// module for the full rationale; duplicated here rather than shared since
/// each module's tests are self-contained.
fn minimal_text_pdf(lines: &[&str]) -> Vec<u8> {
    let filler = "Nota objasniajaca do sprawozdania finansowego za okres sprawozdawczy.";
    let mut all_lines: Vec<&str> = lines.to_vec();
    while all_lines.iter().map(|l| l.len() + 1).sum::<usize>() < 220 {
        all_lines.push(filler);
    }
    let mut content = String::from("BT /F1 12 Tf 40 750 Td 16 TL\n");
    for (i, line) in all_lines.iter().enumerate() {
        if i > 0 {
            content.push_str("T*\n");
        }
        let escaped = line
            .replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)");
        content.push_str(&format!("({escaped}) Tj\n"));
    }
    content.push_str("ET");

    let objects = [
            "<</Type/Catalog/Pages 2 0 R>>".to_owned(),
            "<</Type/Pages/Kids[3 0 R]/Count 1>>".to_owned(),
            "<</Type/Page/Parent 2 0 R/Resources<</Font<</F1 4 0 R>>>>/MediaBox[0 0 612 792]/Contents 5 0 R>>"
                .to_owned(),
            "<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>".to_owned(),
            format!(
                "<</Length {}>>\nstream\n{}\nendstream",
                content.len(),
                content
            ),
        ];
    let mut buf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(buf.len());
        buf.extend_from_slice(format!("{} 0 obj\n{obj}\nendobj\n", i + 1).as_bytes());
    }
    let xref_offset = buf.len();
    buf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    buf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        buf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    buf.extend_from_slice(
        format!(
            "trailer\n<</Size {}/Root 1 0 R>>\nstartxref\n{}\n%%EOF",
            objects.len() + 1,
            xref_offset
        )
        .as_bytes(),
    );
    buf
}

/// Seeds a company + fetched PDF report document whose `title` carries a
/// parseable period (`report_diff::classify::period_sort_key`) and whose
/// file contains `lines`, so `try_structured_extraction`'s PDF branch has
/// both a period to derive and text to parse.
fn seed_pdf_report(
    state: &AppState,
    dir: &std::path::Path,
    title: &str,
    lines: &[&str],
) -> (String, String) {
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CBF".to_owned(),
            display_name: "Cyber_Folks S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://example.com/report.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some(title.to_owned()),
            attribution: None,
        })
        .expect("document");
    let bytes = minimal_text_pdf(lines);
    std::fs::write(dir.join("report.pdf"), &bytes).expect("write pdf");
    state
        .mark_report_document_fetched(
            &document.id,
            Some("report.pdf"),
            Some("application/pdf"),
            // A real content hash, not a placeholder (issue #455's verified-
            // identity capture check reads this back — `local_file_matches_row`
            // — so a stale/absent hash here would make an already-fetched
            // seeded document look unverified and get silently re-fetched).
            Some(&crate::report_documents_capture::content_hash_hex(&bytes)),
            Some(bytes.len() as i64),
        )
        .expect("mark fetched");
    (company.id, document.id)
}

fn report_doc(id: &str, url: &str, title: &str, created_at: &str) -> storage::ReportDocument {
    storage::ReportDocument {
        id: id.to_owned(),
        company_id: "company_gpw_cbf".to_owned(),
        period_id: None,
        source_type: "user_url".to_owned(),
        origin_ref: None,
        url: url.to_owned(),
        local_path: Some("report_documents/x.pdf".to_owned()),
        content_type: Some("application/pdf".to_owned()),
        content_hash: None,
        byte_size: Some(1),
        title: Some(title.to_owned()),
        attribution: None,
        fetch_status: "fetched".to_owned(),
        fetch_error: None,
        fetched_at: None,
        created_at: created_at.to_owned(),
        updated_at: created_at.to_owned(),
        doc_kind: None,
        detected_container: None,
    }
}

#[test]
fn disclosure_key_reads_the_emitent_month_from_espi_urls() {
    // Both accepted ESPI attachment hosts embed /emitent/YYYY-MM/.
    assert_eq!(
        disclosure_month_from_url(
            "https://bonnier.pl/static/att/emitent/2026-05/20260520_172023_x_ssf.pdf"
        ),
        Some("2026-05".to_owned())
    );
    assert_eq!(
        disclosure_month_from_url(
            "https://www.bankier.pl/static/att/emitent/2023-09/c-F-2023-Q2-SSF.pdf"
        ),
        Some("2023-09".to_owned())
    );
    // No /emitent/ segment (e.g. an IR landing page) → no month.
    assert_eq!(
        disclosure_month_from_url("https://modivo.pl/relacje-inwestorskie"),
        None
    );

    // Key falls back to fetched_at, then created_at, when the URL has no month.
    let mut doc = report_doc(
        "d",
        "https://example.com/ir",
        "Q1 SSF",
        "2026-06-15T10:00:00Z",
    );
    doc.fetched_at = Some("2023-08-01T09:00:00Z".to_owned());
    assert_eq!(report_disclosure_key(&doc), "2023-08-01");
    doc.fetched_at = None;
    assert_eq!(report_disclosure_key(&doc), "2026-06-15");
}

/// Guardrail (`d60305c`): detection must rank by the report's disclosure date,
/// not `created_at`. Real-data-shaped: an on-track backfill gives the OLD 2023
/// report a NEWER `created_at` than the actual-latest 2026 report — ranking on
/// `created_at` (the bug) picks 2023; ranking on disclosure picks 2026.
#[test]
fn newest_per_type_ranks_by_disclosure_not_created_at() {
    let stale_2023 = report_doc(
        "doc_2023_q2_ssf",
        "https://www.bankier.pl/static/att/emitent/2023-09/c-F-2023-Q2-SSF.pdf",
        "Cyber Folks 2023 Q2 SSF",
        "2026-06-15T16:49:36.268Z", // backfilled later → newer created_at
    );
    let latest_2026 = report_doc(
        "doc_2026_q1_ssf",
        "https://bonnier.pl/static/att/emitent/2026-05/20260520_x_ssf.pdf",
        "Cyber Folks 2026 Q1 SSF",
        "2026-06-15T16:49:36.167Z", // ingested earlier → older created_at
    );

    let picked = newest_periodic_reports_per_type(vec![stale_2023, latest_2026]);

    assert_eq!(picked.len(), 1, "both are the same statement type (ssf)");
    assert_eq!(
        picked[0].id, "doc_2026_q1_ssf",
        "the actual-latest report must win, not the recently-backfilled old one"
    );
}

/// ADR 0061 decision 1b: on a disclosure-date TIE for the same statement
/// type, the structured xhtml document wins over a PDF sibling, since the
/// deterministic structured-extraction pipeline prefers the xhtml input.
#[test]
fn newest_per_type_prefers_xhtml_on_disclosure_tie() {
    let pdf = report_doc(
        "doc_ssf_pdf",
        "https://bonnier.pl/static/att/emitent/2026-05/20260520_ssf.pdf",
        "Cyber Folks 2026 Q1 SSF",
        "2026-06-15T16:49:36.000Z",
    );
    let xhtml = report_doc(
        "doc_ssf_xhtml",
        "https://bonnier.pl/static/att/emitent/2026-05/20260520_ssf.xhtml",
        "Cyber Folks 2026 Q1 SSF",
        "2026-06-15T16:49:36.100Z",
    );

    let picked = newest_periodic_reports_per_type(vec![pdf, xhtml]);
    assert_eq!(picked.len(), 1);
    assert_eq!(
        picked[0].id, "doc_ssf_xhtml",
        "xhtml must win a disclosure-date tie over its pdf sibling"
    );
}

/// The tie-break preference must never override a genuinely newer
/// disclosure date: a strictly-newer PDF still beats an older xhtml.
#[test]
fn newest_per_type_strictly_newer_pdf_still_beats_older_xhtml() {
    let older_xhtml = report_doc(
        "doc_ssf_xhtml_old",
        "https://bonnier.pl/static/att/emitent/2026-03/20260320_ssf.xhtml",
        "Cyber Folks 2025 Q4 SSF",
        "2026-04-01T10:00:00.000Z",
    );
    let newer_pdf = report_doc(
        "doc_ssf_pdf_new",
        "https://bonnier.pl/static/att/emitent/2026-05/20260520_ssf.pdf",
        "Cyber Folks 2026 Q1 SSF",
        "2026-06-15T10:00:00.000Z",
    );

    let picked = newest_periodic_reports_per_type(vec![older_xhtml, newer_pdf]);
    assert_eq!(picked.len(), 1);
    assert_eq!(
        picked[0].id, "doc_ssf_pdf_new",
        "a strictly newer disclosure date must win regardless of format"
    );
}

/// Helper: a company × framework with one qualitative criterion already
/// carrying a prior agent assessment (the §T6d re-enqueue precondition).
/// Seed a financial period + one confirmed fact for `(company, 2026, H1)` so
/// the occurrence's facts exist — the moment expectations freeze (mirrors the
/// `storage::tests::report_expectations` helper).
fn seed_h1_2026_facts(state: &AppState, company_id: &str) {
    let raw = state.checkout_for_tests().expect("raw connection");
    raw.execute(
        "INSERT INTO financial_periods (id, company_id, fiscal_year, period_type)
             VALUES ('p_h1', ?1, 2026, 'H1')",
        [company_id],
    )
    .expect("seed period");
    raw.execute(
        "INSERT INTO financial_facts (id, company_id, period_id, definition_id, value_numeric)
             VALUES ('f_np', ?1, 'p_h1', 'kpidef_net_profit', '120')",
        [company_id],
    )
    .expect("seed fact");
}

fn seed_expectation_company(state: &AppState) -> storage::Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company")
}

/// J4 (ADR 0071): a frozen, unresolved expectation for the run's occurrence
/// makes the cross-reference stage record an `expectationsToReview` count and
/// the summary nudge the user to review vs actuals — decision-support, no
/// scoring of the user's judgment (ADR 0042).
#[test]
fn cross_reference_links_expectation_review_when_expectations_exist() {
    let connection = open_in_memory_database().expect("db");
    let state = AppState::new(connection);
    let company = seed_expectation_company(&state);

    state
        .report_expectations()
        .create_report_expectation(storage::NewReportExpectation {
            company_id: company.id.clone(),
            event_key: "evt-h1-2026".to_owned(),
            fiscal_year: 2026,
            period_type: "H1".to_owned(),
            stance_md: "Margin recovery on the launch.".to_owned(),
            metrics: Vec::new(),
        })
        .expect("expectation");
    // The report lands: facts arrive → the expectation freezes.
    seed_h1_2026_facts(&state, &company.id);

    let run_id = "run_xref_expect";
    state
        .autopilot()
        .create_run_if_absent(run_id, &company.id, "doc1", "manual", MODE_AUTOPILOT, None)
        .expect("create run")
        .expect("run created");
    let run = state.autopilot().get_run(run_id).expect("get run");

    stage_cross_reference(&state, &run).expect("cross_reference stage");

    let run = state.autopilot().get_run(run_id).expect("get run");
    let refs: serde_json::Value =
        serde_json::from_str(run.cross_refs_json.as_deref().expect("cross_refs written"))
            .expect("cross_refs json");
    assert_eq!(
        refs.get("expectationsToReview").and_then(|v| v.as_u64()),
        Some(1),
        "a frozen, unresolved expectation for the occurrence is counted"
    );

    let summary = compose_summary(&run);
    assert!(
        summary.contains("expectations_to_review"),
        "summary should carry the typed expectations token: {summary}"
    );
}

/// Inverse: with no expectation for the occurrence, the cross-reference stage
/// records a zero count and the summary carries no expectations line.
#[test]
fn cross_reference_omits_expectation_link_when_none_exist() {
    let connection = open_in_memory_database().expect("db");
    let state = AppState::new(connection);
    let company = seed_expectation_company(&state);

    let run_id = "run_xref_no_expect";
    state
        .autopilot()
        .create_run_if_absent(run_id, &company.id, "doc1", "manual", MODE_AUTOPILOT, None)
        .expect("create run")
        .expect("run created");
    let run = state.autopilot().get_run(run_id).expect("get run");

    stage_cross_reference(&state, &run).expect("cross_reference stage");

    let run = state.autopilot().get_run(run_id).expect("get run");
    let refs: serde_json::Value =
        serde_json::from_str(run.cross_refs_json.as_deref().expect("cross_refs written"))
            .expect("cross_refs json");
    assert_eq!(
        refs.get("expectationsToReview").and_then(|v| v.as_u64()),
        Some(0),
        "no expectation → zero count"
    );

    let summary = compose_summary(&run);
    assert!(
        !summary.contains("expectations_to_review"),
        "no expectations token when none exist: {summary}"
    );
}

// ---- F3b: history-sweep tier-4 budget (ADR 0077 §6) --------------------

/// Seed a fetched, canonical periodic PDF report for `company_id` under `dir`,
/// with prose-only content so determinism emits nothing (the run reaches
/// tier-4). Returns the document id.
fn seed_periodic_pdf(
    state: &AppState,
    dir: &std::path::Path,
    company_id: &str,
    title: &str,
    file: &str,
) -> String {
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company_id.to_owned(),
            source_type: "user_url".to_owned(),
            url: format!("https://example.com/{file}"),
            period_id: None,
            origin_ref: None,
            title: Some(title.to_owned()),
            attribution: None,
        })
        .expect("document");
    let bytes = minimal_text_pdf(&["prose only"]);
    std::fs::write(dir.join(file), &bytes).expect("write pdf");
    state
        .mark_report_document_fetched(
            &document.id,
            Some(file),
            Some("application/pdf"),
            // A real content hash — see `seed_pdf_report`'s note (#455's
            // verified-identity capture check reads this back).
            Some(&crate::report_documents_capture::content_hash_hex(&bytes)),
            Some(bytes.len() as i64),
        )
        .expect("mark fetched");
    document.id
}

/// ADR 0084 decision 4 — flagged, never silent. With the AI layer retired,
/// a report document that **no deterministic tier can parse** must still
/// produce a completed run carrying an unread notification whose summary
/// names the typed `no_deterministic_tier` reason — never a silently absent
/// run, never a guessed value, and with no AI branch anywhere in the path.
/// Driven end-to-end through the real durable queue (no network).
#[test]
fn unparseable_report_is_flagged_with_a_notification_and_no_ai_branch() {
    let dir = unique_temp_dir("flagged-no-tier");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CBF".to_owned(),
            display_name: "Cyber_Folks S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    state
        .autopilot()
        .set_mode(&company.id, "assist")
        .expect("assist mode");
    // A text PDF with no financial table: every deterministic tier declines.
    let document_id = seed_periodic_pdf(
        &state,
        &dir,
        &company.id,
        "Skonsolidowany raport roczny 2025 SSF",
        "unparseable.pdf",
    );

    let outcome = enqueue_extraction_run(
        &state,
        &company.id,
        &document_id,
        "detection",
        "assist",
        None,
    );
    assert_eq!(outcome, EnqueueExtractionOutcome::Created);
    crate::jobs::handlers::build_worker(state.clone())
        .run_until_idle()
        .expect("drain the queue");

    let runs = state
        .autopilot()
        .list_runs(&crate::storage::ListAutopilotRunsInput {
            company_id: Some(company.id.clone()),
            limit: Some(50),
            ..Default::default()
        })
        .expect("list runs");
    assert_eq!(runs.len(), 1, "the run must not be silently dropped");
    let run = &runs[0];

    assert_eq!(
        run.stage, STAGE_NOTIFY,
        "an unparseable document still reaches the notify stage"
    );
    assert_eq!(
        run.notification_state, "unread",
        "the gap must be surfaced as an unread notification, not swallowed"
    );
    assert!(
        run.produced_fact_ids.is_empty(),
        "nothing may be guessed for a document no tier parsed"
    );

    let delta = run
        .kpi_delta_json
        .as_deref()
        .expect("a non-emitting run still records its honest delta");
    assert!(
        delta.contains("\"extractionAvailable\":false"),
        "delta: {delta}"
    );
    assert!(
        // A raw-PDF document reports the BY-DESIGN `pdf_document` gap
        // (ADR 0086 dec. 1).
        delta.contains("pdf_document"),
        "the delta must carry the typed reason code, got: {delta}"
    );
    assert!(
        !delta.contains("tier4") && !delta.contains("vision"),
        "no AI/tier-4 branch may appear in the path, got: {delta}"
    );

    let summary = run
        .summary_text
        .as_deref()
        .expect("the notification must carry a summary");
    assert!(
        summary.contains("pdf_document"),
        "the notification must name the typed reason, got: {summary}"
    );
    assert!(
        !summary.contains("no AI provider configured"),
        "the retired guessed diagnosis must be gone, got: {summary}"
    );
}

/// Helper: seed a succeeded, couldn't-extract terminal run over a real
/// (extractable-by-construction) PDF, with a caller-built `kpi_delta_json`.
fn seed_terminal_unavailable_run(
    state: &AppState,
    dir: &std::path::Path,
    run_id: &str,
    delta: serde_json::Value,
) -> storage::AutopilotRun {
    let (company_id, document_id) = seed_pdf_report(
        state,
        dir,
        "Cyber Folks raport roczny 2025 SSF",
        &["Brak danych"],
    );
    state
        .autopilot()
        .create_run_if_absent(
            run_id,
            &company_id,
            &document_id,
            "detection",
            MODE_AUTOPILOT,
            None,
        )
        .expect("create run")
        .expect("run created");
    state
        .autopilot()
        .set_kpi_delta_json(run_id, &delta.to_string())
        .expect("set delta");
    state
        .autopilot()
        .finalize_run(run_id, "succeeded", STAGE_NOTIFY, Some("s"), None)
        .expect("finalize");
    state.autopilot().get_run(run_id).expect("get run")
}

/// A couldn't-extract run whose delta already carries the CURRENT
/// `pipelineVersion` must NOT re-arm on a subsequent enqueue — nothing
/// about the pipeline's read capability changed, so re-running the full
/// file IO + PDF parse with identical inputs is pure waste.
#[test]
fn a_current_version_couldnt_extract_run_is_not_re_armed() {
    let dir = unique_temp_dir("rearm-current-version");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let delta = serde_json::json!({
        "extractionAvailable": false,
        "reason": crate::jobs::structured_extraction::reason::NO_DETERMINISTIC_TIER,
        "pipelineVersion": crate::jobs::structured_extraction::EXTRACTION_PIPELINE_VERSION,
    });
    let run = seed_terminal_unavailable_run(&state, &dir, "run_current_ver", delta);
    assert!(
        !terminal_run_should_rearm(&state, &run),
        "a run stamped with the current pipeline version must settle (no re-arm) — \
             this is the fix for the extraction storm"
    );
}

/// A legacy run (delta predates versioning, no `pipelineVersion`) re-arms
/// ONCE under the new build; after the re-run records a delta stamped with
/// the current version, the next enqueue dedups. Deterministic, no time-based
/// backoff: the version is the only knob.
#[test]
fn a_legacy_run_re_arms_once_then_settles() {
    let dir = unique_temp_dir("rearm-legacy-once");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let legacy = serde_json::json!({
        "extractionAvailable": false,
        "reason": crate::jobs::structured_extraction::reason::NO_DETERMINISTIC_TIER,
    });
    let run = seed_terminal_unavailable_run(&state, &dir, "run_legacy", legacy);
    assert!(
        terminal_run_should_rearm(&state, &run),
        "a legacy (unstamped) couldn't-extract run must re-arm once under the new build"
    );

    // Simulate the re-run recording its new, stamped delta (what stage_extract
    // writes on the extractionAvailable:false path). The period must then settle.
    let stamped = serde_json::json!({
        "extractionAvailable": false,
        "reason": crate::jobs::structured_extraction::reason::NO_DETERMINISTIC_TIER,
        "pipelineVersion": crate::jobs::structured_extraction::EXTRACTION_PIPELINE_VERSION,
    });
    state
        .autopilot()
        .set_kpi_delta_json("run_legacy", &stamped.to_string())
        .expect("set stamped delta");
    let settled = state.autopilot().get_run("run_legacy").expect("get run");
    assert!(
        !terminal_run_should_rearm(&state, &settled),
        "after recording a stamped delta the period must dedup on the next enqueue"
    );
}

/// Regression: when the version gate is OPEN (a run stamped with a version
/// LOWER than the current build — a genuine parser upgrade), a witness_fallback
/// period still re-arms so the upgrade reaches it with real issuer data. The
/// version gate must not close the door on legitimate capability upgrades.
#[test]
fn a_lower_version_witness_fallback_still_re_arms_on_upgrade() {
    let dir = unique_temp_dir("rearm-lower-version");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let older_version =
        crate::jobs::structured_extraction::EXTRACTION_PIPELINE_VERSION.saturating_sub(1);
    let delta = serde_json::json!({
        "extractionAvailable": false,
        "reason": "witness_fallback",
        "pipelineVersion": older_version,
    });
    let run = seed_terminal_unavailable_run(&state, &dir, "run_lower_ver", delta);
    assert!(
        terminal_run_should_rearm(&state, &run),
        "a parser upgrade (higher current version) must still reach a lower-version \
             witness_fallback period"
    );
}

/// ADR 0086 dec. 1: `pdf_document` is the BY-DESIGN gap — machine
/// fact-reading of PDFs is retired, so no pipeline upgrade ever makes the
/// document readable. Even a lower-version delta must never re-arm.
#[test]
fn a_pdf_document_gap_is_never_rearmed() {
    let dir = unique_temp_dir("rearm-pdf-doc");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let older_version =
        crate::jobs::structured_extraction::EXTRACTION_PIPELINE_VERSION.saturating_sub(1);
    let delta = serde_json::json!({
        "extractionAvailable": false,
        "reason": "pdf_document",
        "pipelineVersion": older_version,
    });
    let run = seed_terminal_unavailable_run(&state, &dir, "run_pdf_gap", delta);
    assert!(
        !terminal_run_should_rearm(&state, &run),
        "the by-design PDF gap must never re-arm — no upgrade makes a PDF readable"
    );
}

/// The gap-reason derivation: a raw-PDF document reports the by-design
/// `pdf_document` reason; anything else keeps the caller's fallback.
#[test]
fn gap_reason_names_a_pdf_document_and_keeps_the_fallback_otherwise() {
    let dir = unique_temp_dir("gap-reason");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "GAP".to_owned(),
            display_name: "Gap Reason S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");

    let run_for = |suffix: &str, url: &str, local: &str, mime: &str| {
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "user_url".to_owned(),
                url: url.to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some(format!("doc {suffix}")),
                attribution: None,
            })
            .expect("document");
        std::fs::write(dir.join(local), b"stub").expect("write file");
        state
            .mark_report_document_fetched(&document.id, Some(local), Some(mime), None, Some(4))
            .expect("mark fetched");
        let run_id = format!("run_gap_{suffix}");
        state
            .autopilot()
            .create_run_if_absent(
                &run_id,
                &company.id,
                &document.id,
                "manual",
                MODE_AUTOPILOT,
                None,
            )
            .expect("create run")
            .expect("run created");
        state.autopilot().get_run(&run_id).expect("get run")
    };

    let pdf_run = run_for(
        "pdf",
        "https://example.com/report.pdf",
        "report.pdf",
        "application/pdf",
    );
    assert_eq!(
        gap_reason(&state, &pdf_run, "no_deterministic_tier"),
        "pdf_document"
    );

    let xhtml_run = run_for(
        "xhtml",
        "https://example.com/report.xhtml",
        "report.xhtml",
        "application/xhtml+xml",
    );
    assert_eq!(
        gap_reason(&state, &xhtml_run, "no_deterministic_tier"),
        "no_deterministic_tier"
    );
}

/// Epic #229 T2: `pdf_document` is the **never-re-armed** verdict (ADR 0086
/// dec. 1). Only bytes that really are a PDF may earn it. 45 of the
/// maintainer's stored `.pdf` files are XML or ZIP inside; stamping those
/// `pdf_document` on the strength of their extension would wrongly retire
/// documents a deterministic tier can still read.
#[test]
fn gap_reason_earns_pdf_document_only_from_the_sniffed_container() {
    let dir = unique_temp_dir("gap-reason-container");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "GPC".to_owned(),
            display_name: "Gap Container S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");

    let run_for = |suffix: &str, container: &str| {
        let local = format!("report_{suffix}.pdf");
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "user_url".to_owned(),
                url: format!("https://example.com/{local}"),
                period_id: None,
                origin_ref: None,
                title: Some(format!("doc {suffix}")),
                attribution: None,
            })
            .expect("document");
        std::fs::write(dir.join(&local), b"stub").expect("write file");
        state
            .mark_report_document_fetched(
                &document.id,
                Some(&local),
                Some("application/pdf"),
                None,
                Some(4),
            )
            .expect("mark fetched");
        state
            .set_report_document_detected_container(&document.id, container)
            .expect("stamp container");
        let run_id = format!("run_gapc_{suffix}");
        state
            .autopilot()
            .create_run_if_absent(
                &run_id,
                &company.id,
                &document.id,
                "manual",
                MODE_AUTOPILOT,
                None,
            )
            .expect("create run")
            .expect("run created");
        state.autopilot().get_run(&run_id).expect("get run")
    };

    // Everything below is named `.pdf` and served `application/pdf`.
    assert_eq!(
        gap_reason(&state, &run_for("real", "pdf"), "no_deterministic_tier"),
        "pdf_document",
        "a genuine PDF still earns the by-design reason"
    );
    assert_eq!(
        gap_reason(&state, &run_for("xml", "xml"), "no_deterministic_tier"),
        "no_deterministic_tier",
        "an XML statement under a .pdf name keeps a re-armable reason"
    );
    assert_eq!(
        gap_reason(&state, &run_for("zip", "zip"), "no_deterministic_tier"),
        "no_deterministic_tier",
        "an ESEF package under a .pdf name keeps a re-armable reason"
    );
}

/// Epic #229 T2: the canonical-report tie-break (`prefers_candidate`, reused by
/// the coverage read model) prefers the **structured** document when two
/// filings share a disclosure key. Resolving that from the URL alone ranked the
/// corpus's 38 XML statements stored under a `.pdf` name below their companion
/// PDF, handing the canonical slot to the document with less extractable data.
#[test]
fn structured_tie_break_prefers_the_markup_stored_under_a_pdf_name() {
    let mut markup = report_doc(
        "doc_markup",
        "https://bonnier.pl/static/att/emitent/2025-05/ssf_2025.pdf",
        "SSF 2025",
        "2025-05-02T00:00:00Z",
    );
    markup.detected_container = Some("xml".to_owned());
    let mut pdf = report_doc(
        "doc_pdf",
        "https://bonnier.pl/static/att/emitent/2025-05/ssf_2025_scan.pdf",
        "SSF 2025 scan",
        "2025-05-01T00:00:00Z",
    );
    pdf.detected_container = Some("pdf".to_owned());

    assert!(is_structured_document(&markup));
    assert!(!is_structured_document(&pdf));
    // Same disclosure month → the tie-break decides, and it must pick the
    // markup even though BOTH URLs end `.pdf`.
    assert!(
        prefers_candidate(&pdf, &markup),
        "the genuinely structured sibling must win the canonical slot"
    );
    assert!(
        !prefers_candidate(&markup, &pdf),
        "and the PDF must not displace it on a re-run"
    );

    // An ESEF report package is structured too — the structured path unpacks it.
    let mut package = report_doc(
        "doc_zip",
        "https://bonnier.pl/static/att/emitent/2025-05/ssf_2025_pkg.pdf",
        "SSF 2025 package",
        "2025-05-03T00:00:00Z",
    );
    package.detected_container = Some("zip".to_owned());
    assert!(is_structured_document(&package));
}
