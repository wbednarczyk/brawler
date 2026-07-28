//! Tests for the BiznesRadar-primary fundamentals pull (ADR 0086 C2).
//!
//! No test touches the network: the only fetcher installed is a per-kind stub
//! replaying stored sample pages, counting its calls so a "did NOT fetch" (cadence)
//! assertion is real, not "nothing changed".

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rust_decimal::Decimal;

use super::*;
use crate::source_adapters::biznesradar_fundamentals::{
    resolve_aggregator_page, FundamentalsWitnessFetcher, PageResolution,
};
use crate::storage::{open_in_memory_database, AppState, NewCompany, StructuredFactInput};

const BILANS: &str = include_str!("../../../samples/biznesradar_bilans_cdr.html");
const LANDING: &str = include_str!("../../../samples/biznesradar_fundamenty_landing.html");

/// A fetcher replaying a canned body per page kind (defaulting to the landing —
/// "no coverage"), counting calls.
struct MapFetcher {
    by_kind: HashMap<&'static str, String>,
    calls: Arc<AtomicUsize>,
}

impl MapFetcher {
    fn new(pairs: &[(AggregatorPageKind, &str)]) -> (Arc<Self>, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let by_kind = pairs
            .iter()
            .map(|(kind, body)| (kind.as_str(), (*body).to_owned()))
            .collect();
        (
            Arc::new(Self {
                by_kind,
                calls: Arc::clone(&calls),
            }),
            calls,
        )
    }

    fn body(&self, kind: AggregatorPageKind) -> String {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.by_kind
            .get(kind.as_str())
            .cloned()
            .unwrap_or_else(|| LANDING.to_owned())
    }
}

impl FundamentalsWitnessFetcher for MapFetcher {
    fn fetch_fundamentals(&self, _ticker: &str) -> Result<String, String> {
        Ok(self.body(AggregatorPageKind::Income))
    }
    fn fetch_page(&self, kind: AggregatorPageKind, _ticker: &str) -> Result<String, String> {
        Ok(self.body(kind))
    }
}

fn state_with_company() -> (AppState, String) {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    (state, company.id)
}

/// Seed one prior fact into its slot with a chosen tier/method, so the pull's
/// precedence can be exercised against it.
fn seed_fact(
    state: &AppState,
    company_id: &str,
    metric_key: &str,
    fiscal_year: i64,
    value: i64,
    source_tier: &str,
    extraction_method: &str,
) {
    let value = value.to_string();
    state
        .kpi_extraction()
        .record_structured_fact(StructuredFactInput {
            company_id,
            fiscal_year,
            period_type: "FY",
            period_end: Some(&format!("{fiscal_year}-12-31")),
            report_document_id: "seed-doc",
            metric_key,
            value_numeric: &value,
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier,
            extraction_method,
            validation_status: "unreviewed",
            drift_json: None,
            citation: Some("seed"),
        })
        .expect("seed fact");
}

fn stored(state: &AppState, company_id: &str, fiscal_year: i64, metric: &str) -> Option<Decimal> {
    state
        .financials()
        .stored_fact_set(company_id, fiscal_year, "FY")
        .expect("stored set")
        .and_then(|set| set.get(metric).copied())
}

#[test]
fn pull_writes_aggregator_facts_for_every_sample_period() {
    let (state, company_id) = state_with_company();
    let (fetcher, _) = MapFetcher::new(&[(AggregatorPageKind::Balance, BILANS)]);
    let state = state.with_fundamentals_witness_fetcher(fetcher);

    let summary = run_aggregator_fundamentals_pull(&state).expect("pull runs");

    // The balance page carries FY 2007..2011; inventories + current_assets land in
    // each, scaled from tys. zł to base units.
    assert!(summary.facts_written > 0, "summary: {summary:?}");
    assert_eq!(
        stored(&state, &company_id, 2008, "inventories"),
        Some(Decimal::from(1_373_000)),
        "the 2008 inventories column is written from the aggregator page"
    );
    assert_eq!(
        stored(&state, &company_id, 2007, "current_assets"),
        Some(Decimal::from(15_301_000)),
        "a different year's column is also written — every period, not just one"
    );
    // Income & cash-flow pages resolved to the landing (no table) → unavailable.
    assert!(summary.pages_resolved >= 1);
    assert!(summary.pages_unavailable >= 1);
}

#[test]
fn pull_never_overwrites_a_manual_slot() {
    let (state, company_id) = state_with_company();
    // A hand-entered inventories 2008 value the automaton must never touch.
    seed_fact(
        &state,
        &company_id,
        "inventories",
        2008,
        111,
        "manual",
        "manual",
    );
    let (fetcher, _) = MapFetcher::new(&[(AggregatorPageKind::Balance, BILANS)]);
    let state = state.with_fundamentals_witness_fetcher(fetcher);

    let summary = run_aggregator_fundamentals_pull(&state).expect("pull runs");

    assert_eq!(
        stored(&state, &company_id, 2008, "inventories"),
        Some(Decimal::from(111)),
        "a manual fact is untouchable by the aggregator (ADR 0086 dec. 3)"
    );
    assert!(
        summary.slots_skipped_higher_tier >= 1,
        "the manual slot must be reported as skipped: {summary:?}"
    );
    // A manual divergence is NEVER applied (the 111 stays) but IS now recorded as
    // an INFORMATIONAL witness_disagreement (ADR 0086 dec. 3 "logged, never
    // applied" made concrete, amended 2026-07-22): the user must learn the
    // aggregator disagrees with their own entry.
    assert!(
        summary.witness_disagreements >= 1,
        "a manual divergence records an informational disagreement: {summary:?}"
    );
    let flagged = state
        .fundamentals_provenance()
        .list_flagged_extraction_outcomes(&company_id)
        .expect("flagged outcomes");
    assert!(
        flagged
            .iter()
            .any(|outcome| outcome.reason_code == "witness_disagreement"
                && outcome.tier.as_deref() == Some("manual")),
        "a manual-slot divergence records a witness_disagreement tagged manual: {flagged:?}"
    );
}

#[test]
fn pull_records_witness_disagreement_against_an_issuer_slot_without_overwriting() {
    let (state, company_id) = state_with_company();
    // An ESEF current_assets 2008 wildly different from the aggregator's 16 815 000.
    seed_fact(
        &state,
        &company_id,
        "current_assets",
        2008,
        999,
        "esef",
        "api",
    );
    let (fetcher, _) = MapFetcher::new(&[(AggregatorPageKind::Balance, BILANS)]);
    let state = state.with_fundamentals_witness_fetcher(fetcher);

    let summary = run_aggregator_fundamentals_pull(&state).expect("pull runs");

    // The issuer value is untouched.
    assert_eq!(
        stored(&state, &company_id, 2008, "current_assets"),
        Some(Decimal::from(999)),
        "an ESEF slot outranks the aggregator and is never overwritten"
    );
    assert!(
        summary.witness_disagreements >= 1,
        "a divergent issuer slot records a witness_disagreement: {summary:?}"
    );
    let flagged = state
        .fundamentals_provenance()
        .list_flagged_extraction_outcomes(&company_id)
        .expect("flagged outcomes");
    let disagreement = flagged
        .iter()
        .find(|outcome| {
            outcome.reason_code == "witness_disagreement" && outcome.tier.as_deref() == Some("esef")
        })
        .unwrap_or_else(|| {
            panic!("an informational witness_disagreement outcome is recorded: {flagged:?}")
        });
    // FINDING 3: the detail is the canonical gate shape the WDF witness seam
    // writes (failedIdentities / failedCrossChecks / witnessDisagreements arrays),
    // NOT a flat {metricKey, aggregatorValue, ...} object — so the Coverage panel
    // renders it as investor language, never raw JSON keys.
    let detail: serde_json::Value =
        serde_json::from_str(disagreement.detail_json.as_deref().expect("detail_json"))
            .expect("detail parses");
    assert!(
        detail
            .get("witnessDisagreements")
            .and_then(|v| v.as_array())
            .is_some(),
        "detail carries the canonical witnessDisagreements array: {detail}"
    );
    assert!(
        detail.get("aggregatorValue").is_none() && detail.get("issuerValue").is_none(),
        "the flat raw-key shape is gone: {detail}"
    );
    let entry = &detail["witnessDisagreements"][0];
    assert_eq!(entry["metricKey"], "current_assets");
    // Convention (ADR 0085 dec. 2): expected = aggregator, actual = issuer.
    assert_eq!(
        entry["detail"]["actual"], "999",
        "actual is the issuer value: {detail}"
    );
    assert!(
        entry["detail"].get("expected").is_some(),
        "expected is the aggregator value: {detail}"
    );
}

#[test]
fn pull_records_witness_disagreement_against_a_positional_pdf_slot() {
    // FINDING 1: the positional `pdf` tier is the issuer's OWN filing read
    // deterministically (ADR 0086 dec. 4, amended 2026-07-22) — it counts as an
    // issuer tier, so a diverging aggregator value must be flagged, not silently
    // dropped as it was when `is_issuer_tier` string-matched only esef/xhtml/wdf.
    let (state, company_id) = state_with_company();
    seed_fact(
        &state,
        &company_id,
        "current_assets",
        2008,
        999,
        "pdf",
        "html_positional",
    );
    let (fetcher, _) = MapFetcher::new(&[(AggregatorPageKind::Balance, BILANS)]);
    let state = state.with_fundamentals_witness_fetcher(fetcher);

    let summary = run_aggregator_fundamentals_pull(&state).expect("pull runs");

    assert_eq!(
        stored(&state, &company_id, 2008, "current_assets"),
        Some(Decimal::from(999)),
        "a positional pdf slot is the issuer's filing — never overwritten"
    );
    assert!(
        summary.witness_disagreements >= 1,
        "a divergent positional slot records a disagreement: {summary:?}"
    );
    let flagged = state
        .fundamentals_provenance()
        .list_flagged_extraction_outcomes(&company_id)
        .expect("flagged outcomes");
    assert!(
        flagged
            .iter()
            .any(|outcome| outcome.reason_code == "witness_disagreement"
                && outcome.tier.as_deref() == Some("pdf")),
        "an informational witness_disagreement for the positional slot: {flagged:?}"
    );
}

#[test]
fn pull_reobserves_an_agreeing_issuer_slot_without_a_disagreement() {
    // FINDING 4 (the disagreement path's untested agreeing half): when the issuer
    // slot already holds the aggregator's value (within tolerance), the pull
    // RE-OBSERVES it and records NO witness_disagreement.
    let (state, company_id) = state_with_company();
    // The balance page writes inventories 2008 = 1 373 000 (base units); seed the
    // issuer (ESEF) holding exactly that, so the aggregator agrees.
    seed_fact(
        &state,
        &company_id,
        "inventories",
        2008,
        1_373_000,
        "esef",
        "api",
    );
    let (fetcher, _) = MapFetcher::new(&[(AggregatorPageKind::Balance, BILANS)]);
    let state = state.with_fundamentals_witness_fetcher(fetcher);

    let summary = run_aggregator_fundamentals_pull(&state).expect("pull runs");

    assert_eq!(
        summary.witness_disagreements, 0,
        "an agreeing issuer slot is never flagged: {summary:?}"
    );
    assert!(
        summary.facts_reobserved >= 1,
        "the agreeing issuer slot is re-observed, not rewritten: {summary:?}"
    );
    assert_eq!(
        stored(&state, &company_id, 2008, "inventories"),
        Some(Decimal::from(1_373_000)),
        "the issuer value is unchanged by the agreeing pull"
    );
    let flagged = state
        .fundamentals_provenance()
        .list_flagged_extraction_outcomes(&company_id)
        .expect("flagged outcomes");
    assert!(
        flagged
            .iter()
            .all(|outcome| outcome.reason_code != "witness_disagreement"),
        "no witness_disagreement is recorded on agreement: {flagged:?}"
    );
}

#[test]
fn pull_skips_an_empty_zero_aggregator_cell() {
    // The zero rule (ADR 0085 amendment): a tracked metric rendered `0` is a scrape
    // artifact — never written, never evidence.
    let zero_page = r#"<html><body><p>Dane w tys. zł</p>
      <table class="report-table">
        <tr><th>Pozycja</th><th>2020 (gru 20)</th></tr>
        <tr><td>Zapasy</td><td>0</td></tr>
        <tr><td>Aktywa obrotowe</td><td>5 000</td></tr>
      </table></body></html>"#;
    let (state, company_id) = state_with_company();
    let (fetcher, _) = MapFetcher::new(&[(AggregatorPageKind::Balance, zero_page)]);
    let state = state.with_fundamentals_witness_fetcher(fetcher);

    let summary = run_aggregator_fundamentals_pull(&state).expect("pull runs");

    assert!(summary.zero_cells_skipped >= 1, "summary: {summary:?}");
    assert_eq!(
        stored(&state, &company_id, 2020, "inventories"),
        None,
        "a zero cell is never written"
    );
    // The non-zero sibling in the same period IS written — the skip is per-cell.
    assert_eq!(
        stored(&state, &company_id, 2020, "current_assets"),
        Some(Decimal::from(5_000_000))
    );
}

#[test]
fn second_resolve_inside_the_cadence_window_does_not_refetch() {
    // ADR 0085 dec. 3, per page (ADR 0086 dec. 2): at most one fetch per (company,
    // page kind) per day. Asserted on the fetch COUNT.
    let (state, company_id) = state_with_company();
    let (fetcher, calls) = MapFetcher::new(&[(AggregatorPageKind::Balance, BILANS)]);
    let state = state.with_fundamentals_witness_fetcher(fetcher);

    let first = resolve_aggregator_page(&state, &company_id, AggregatorPageKind::Balance);
    let second = resolve_aggregator_page(&state, &company_id, AggregatorPageKind::Balance);

    assert!(matches!(first, PageResolution::Page { .. }));
    assert_eq!(first, second, "the cached page resolves identically");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "the second resolve inside the window must read the cache, not refetch"
    );
}

#[test]
fn a_rerun_reobserves_unchanged_slots_rather_than_duplicating() {
    let (state, _company_id) = state_with_company();
    let (fetcher, _) = MapFetcher::new(&[(AggregatorPageKind::Balance, BILANS)]);
    let state = state.with_fundamentals_witness_fetcher(fetcher);

    let first = run_aggregator_fundamentals_pull(&state).expect("first pull");
    let second = run_aggregator_fundamentals_pull(&state).expect("second pull");

    assert!(first.facts_written > 0);
    assert_eq!(
        second.facts_written, 0,
        "a re-run writes no new facts — the slots already hold the values"
    );
    assert!(
        second.facts_reobserved > 0,
        "the re-run re-observes the aggregator's own slots: {second:?}"
    );
}

#[test]
fn enqueue_daily_pull_arms_one_queue_job_idempotently() {
    let (state, _) = state_with_company();

    enqueue_daily_pull(&state);
    enqueue_daily_pull(&state);

    let claimed = state
        .jobs()
        .claim_next_for_kinds(&[AGGREGATOR_FUNDAMENTALS_PULL_KIND])
        .expect("claim")
        .expect("the daily trigger must arm a queued pull job");
    assert_eq!(claimed.kind, AGGREGATOR_FUNDAMENTALS_PULL_KIND);
    assert!(
        state
            .jobs()
            .claim_next_for_kinds(&[AGGREGATOR_FUNDAMENTALS_PULL_KIND])
            .expect("claim")
            .is_none(),
        "re-arming reuses the same stable job id — one queued pull, not a pile"
    );
}

/// Guardrail G3 (review 2026-07-22): the same metric disagreeing with
/// issuer-held slots at MAPPING_SUSPECT_MIN_COMPANIES+ distinct companies in one
/// run is the finding-1 signature (a dictionary row filed under the wrong
/// metric) — it must surface as a `mapping_suspect`, not drown as N scattered
/// informational entries.
#[test]
fn systematic_cross_company_disagreement_flags_a_mapping_suspect() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let (fetcher, _) = MapFetcher::new(&[(AggregatorPageKind::Balance, BILANS)]);
    let state = state.with_fundamentals_witness_fetcher(fetcher);

    for i in 0..5 {
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: format!("SU{i}"),
                display_name: format!("Suspect {i} S.A."),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");
        // Every company's ESEF-held total_assets wildly disagrees with the
        // sample page — the systematic pattern of a mismapped dictionary row.
        for fiscal_year in [2008, 2009, 2010] {
            seed_fact(
                &state,
                &company.id,
                "total_assets",
                fiscal_year,
                1,
                "esef",
                "api",
            );
        }
    }

    let summary = run_aggregator_fundamentals_pull(&state).expect("pull");

    assert!(
        summary
            .mapping_suspects
            .contains(&"total_assets".to_owned()),
        "5 companies × systematic disagreement must flag the metric: {summary:?}"
    );
}

/// The control: a single company's disagreement stays informational — no suspect.
#[test]
fn a_scattered_single_company_disagreement_is_not_a_mapping_suspect() {
    let (state, company_id) = state_with_company();
    let (fetcher, _) = MapFetcher::new(&[(AggregatorPageKind::Balance, BILANS)]);
    let state = state.with_fundamentals_witness_fetcher(fetcher);
    seed_fact(&state, &company_id, "total_assets", 2008, 1, "esef", "api");

    let summary = run_aggregator_fundamentals_pull(&state).expect("pull");

    assert!(
        summary.witness_disagreements >= 1,
        "the informational entry still records: {summary:?}"
    );
    assert!(
        summary.mapping_suspects.is_empty(),
        "one company is noise, not a mapping suspect: {summary:?}"
    );
}

/// Issue #132: the on-demand entry points (the `run_aggregator_fundamentals_pull`
/// command, the rebuild's pass 1) must share the queue's per-adapter
/// serialization — the politeness posture is "at most one BiznesRadar pull at a
/// time" regardless of who triggered it. A pull racing an in-flight one is
/// rejected with a typed busy error, never run concurrently.
#[test]
fn on_demand_pull_defers_to_an_in_flight_biznesradar_run() {
    let (state, _company_id) = state_with_company();
    let (fetcher, _) = MapFetcher::new(&[(AggregatorPageKind::Balance, BILANS)]);
    let state = state.with_fundamentals_witness_fetcher(fetcher);

    let guard = state
        .try_acquire_source(ADAPTER_ID)
        .expect("lock initially free");
    let error = run_aggregator_fundamentals_pull_serialized(&state)
        .expect_err("a concurrent on-demand pull must be rejected, not run in parallel");
    assert!(
        error.contains("aggregator_pull_already_running"),
        "busy error carries the typed code: {error}"
    );

    drop(guard);
    run_aggregator_fundamentals_pull_serialized(&state).expect("runs once the lock is free");
}
