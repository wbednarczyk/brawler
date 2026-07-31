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

/// The provenance row behind one stored slot — the corroboration stamp lives
/// here, so every witness assertion reads it through the real store.
fn provenance_of(
    state: &AppState,
    company_id: &str,
    fiscal_year: i64,
    metric: &str,
) -> Option<crate::storage::FactProvenance> {
    let periods = state
        .financials()
        .list_financial_periods(crate::storage::ListFinancialPeriodsInput {
            company_id: company_id.to_owned(),
            fiscal_year: None,
        })
        .expect("periods");
    let period = periods
        .into_iter()
        .find(|p| p.fiscal_year == fiscal_year && p.period_type == "FY")?;
    let fact = state
        .financials()
        .list_financial_facts(crate::storage::ListFinancialFactsInput {
            company_id: Some(company_id.to_owned()),
            period_id: Some(period.id),
            definition_id: None,
        })
        .expect("facts")
        .into_iter()
        .find(|f| f.definition_id == format!("kpidef_{metric}"))?;
    state
        .fundamentals_provenance()
        .get_fact_provenance(&fact.id)
        .expect("provenance")
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
fn pull_corroborates_an_agreeing_issuer_slot_and_upgrades_the_verdict() {
    // Epic #229 T5 deliverable 1 (ADR 0086 dec. 4, positive half): agreement was
    // the silent case — BR reading the same figure as the issuer's own filing
    // left NOTHING behind. It now stamps the witness columns and upgrades an
    // `unreviewed`/`passed` verdict to `witness_confirmed`.
    let (state, company_id) = state_with_company();
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

    assert!(
        summary.witness_corroborations >= 1,
        "an agreeing issuer slot is corroborated: {summary:?}"
    );
    assert_eq!(summary.witness_disagreements, 0);
    let provenance = provenance_of(&state, &company_id, 2008, "inventories")
        .expect("the issuer fact keeps its provenance row");
    assert_eq!(
        provenance.validation_status, "witness_confirmed",
        "an independently corroborated issuer value is witness_confirmed: {provenance:?}"
    );
    assert_eq!(
        provenance
            .witness_value
            .as_deref()
            .map(|v| v.parse::<Decimal>().expect("witness value parses")),
        Some(Decimal::from(1_373_000)),
        "the aggregator's own figure is stamped: {provenance:?}"
    );
    assert!(
        provenance
            .witness_page_url
            .as_deref()
            .is_some_and(|url| url.contains("biznesradar.pl")),
        "the evidence page is stamped: {provenance:?}"
    );
    assert!(
        provenance.corroborated_at.is_some(),
        "the corroboration is timestamped: {provenance:?}"
    );
    assert_eq!(
        provenance.source_tier, "esef",
        "corroboration never re-labels the tier that produced the value"
    );
}

#[test]
fn pull_stamps_a_manual_slot_without_touching_its_verdict() {
    // ADR 0086 dec. 3 posture: a hand-entered value is the user's, so the witness
    // is RECORDED beside it but the automaton never re-labels it
    // `witness_confirmed`. Within tolerance (0.5%) counts as agreement, so this
    // also exercises the SkippedHigherTier-but-agreeing branch.
    let (state, company_id) = state_with_company();
    seed_fact(
        &state,
        &company_id,
        "inventories",
        2008,
        1_373_100, // 0.007% off the aggregator's 1 373 000 — inside tolerance
        "manual",
        "manual",
    );
    let (fetcher, _) = MapFetcher::new(&[(AggregatorPageKind::Balance, BILANS)]);
    let state = state.with_fundamentals_witness_fetcher(fetcher);

    let summary = run_aggregator_fundamentals_pull(&state).expect("pull runs");

    assert_eq!(
        stored(&state, &company_id, 2008, "inventories"),
        Some(Decimal::from(1_373_100)),
        "the manual value is never overwritten, corroborated or not"
    );
    assert_eq!(
        summary.witness_disagreements, 0,
        "inside tolerance is agreement, not a disagreement: {summary:?}"
    );
    assert!(summary.witness_corroborations >= 1, "{summary:?}");
    let provenance = provenance_of(&state, &company_id, 2008, "inventories")
        .expect("a manual slot gains a provenance row carrying only the witness stamp");
    assert_ne!(
        provenance.validation_status, "witness_confirmed",
        "a manual verdict is NEVER upgraded by the automaton: {provenance:?}"
    );
    assert!(
        provenance.witness_value.is_some() && provenance.corroborated_at.is_some(),
        "the witness columns are stamped even on a manual slot: {provenance:?}"
    );
    // The read-model invariant holds: a hand-entered value still counts in the
    // manual bucket, never as a pipeline tier (the rebuild verdict's promise).
    let breakdown = state
        .fundamentals_provenance()
        .count_facts_by_tier()
        .expect("tier breakdown");
    assert!(
        breakdown.manual_or_unprovenanced >= 1,
        "the manual fact stays in the manual bucket: {breakdown:?}"
    );
    assert!(
        breakdown
            .by_tier
            .iter()
            .all(|entry| entry.source_tier != "manual"),
        "the manual stamp is never reported as a pipeline tier: {breakdown:?}"
    );
}

#[test]
fn pull_never_self_witnesses_its_own_slot() {
    // No self-witnessing: the aggregator re-reading its OWN stored value is not
    // corroboration — a second look at the same source proves nothing.
    let (state, company_id) = state_with_company();
    let (fetcher, _) = MapFetcher::new(&[(AggregatorPageKind::Balance, BILANS)]);
    let state = state.with_fundamentals_witness_fetcher(fetcher);

    run_aggregator_fundamentals_pull(&state).expect("first pull");
    let summary = run_aggregator_fundamentals_pull(&state).expect("second pull re-observes");

    assert!(
        summary.facts_reobserved >= 1,
        "the second pull re-observes its own facts: {summary:?}"
    );
    assert_eq!(
        summary.witness_corroborations, 0,
        "the aggregator never corroborates itself: {summary:?}"
    );
    let provenance = provenance_of(&state, &company_id, 2008, "inventories")
        .expect("the aggregator fact has provenance");
    assert_eq!(provenance.source_tier, "html_aggregator");
    assert_eq!(
        provenance.witness_value, None,
        "no self-witness stamp: {provenance:?}"
    );
    assert_eq!(provenance.corroborated_at, None);
    assert_eq!(
        provenance.validation_status, "unreviewed",
        "a self-read never upgrades the verdict: {provenance:?}"
    );
}

#[test]
fn a_disagreeing_issuer_slot_is_flagged_and_never_corroborated() {
    // The negative half is unchanged AND exclusive: a divergence records the
    // disagreement and stamps no corroboration.
    let (state, company_id) = state_with_company();
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

    assert!(summary.witness_disagreements >= 1, "{summary:?}");
    let provenance = provenance_of(&state, &company_id, 2008, "current_assets")
        .expect("the issuer fact keeps its provenance row");
    assert_eq!(
        provenance.witness_value, None,
        "a disagreement stamps no corroboration: {provenance:?}"
    );
    assert_eq!(provenance.corroborated_at, None);
    assert_eq!(
        provenance.validation_status, "unreviewed",
        "a disagreeing witness never upgrades the verdict: {provenance:?}"
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

// ---------------------------------------------------------------------------
// Zero-effects invariant (epic #40 S5, ADR 0091)
// ---------------------------------------------------------------------------

/// A pull that iterated companies and wrote nothing must NAME why. Each state
/// below is one the pull's own counters can reach; a future state that cannot
/// explain itself falls through to `Unexplained` and reddens here.
#[test]
fn every_zero_effect_aggregator_state_names_a_reason() {
    use crate::effects_honesty::{EffectVerdict, ExplainsEffect};
    use aggregator_effect_reason as reason;

    let with = |mutate: fn(&mut AggregatorPullSummary)| {
        let mut summary = AggregatorPullSummary {
            companies: 12,
            ..AggregatorPullSummary::default()
        };
        mutate(&mut summary);
        summary.effect_verdict()
    };

    assert_eq!(
        with(|s| s.pages_unavailable = 24),
        EffectVerdict::NothingProduced {
            reason: reason::PAGES_UNAVAILABLE
        }
    );
    assert_eq!(
        with(|s| {
            s.pages_resolved = 24;
            s.slots_skipped_higher_tier = 40;
        }),
        EffectVerdict::NothingProduced {
            reason: reason::HIGHER_TIER_HOLDS_SLOT
        }
    );
    assert_eq!(
        with(|s| {
            s.pages_resolved = 24;
            s.facts_reobserved = 40;
        }),
        EffectVerdict::NothingProduced {
            reason: reason::ALREADY_RECORDED
        }
    );
    assert_eq!(
        with(|s| {
            s.pages_resolved = 24;
            s.zero_cells_skipped = 40;
        }),
        EffectVerdict::NothingProduced {
            reason: reason::ZERO_CELLS
        }
    );
    assert_eq!(
        with(|s| {
            s.pages_resolved = 24;
            s.no_definition = 40;
        }),
        EffectVerdict::NothingProduced {
            reason: reason::NO_KPI_DEFINITION
        }
    );
    // A page that resolved but parsed to zero cells (issue #244 — formerly the
    // pinned unexplained state): layout change or empty table, now counted.
    assert_eq!(
        with(|s| {
            s.pages_resolved = 24;
            s.pages_empty = 24;
        }),
        EffectVerdict::NothingProduced {
            reason: reason::PAGES_EMPTY
        }
    );
    // Effects, not gaps: a written/updated fact, and a recorded reversed-
    // witnessing disagreement (no fact written, but a finding persisted).
    assert_eq!(with(|s| s.facts_written = 1), EffectVerdict::Produced);
    assert_eq!(with(|s| s.facts_updated = 1), EffectVerdict::Produced);
    assert_eq!(
        with(|s| s.witness_disagreements = 1),
        EffectVerdict::Produced
    );
    // A recorded corroboration is equally an effect (epic #229 T5): no fact was
    // written, but "an independent source read the same figure" was persisted.
    assert_eq!(
        with(|s| s.witness_corroborations = 1),
        EffectVerdict::Produced
    );
    // No tracked companies: nothing was asked of the pull.
    assert_eq!(
        AggregatorPullSummary::default().effect_verdict(),
        EffectVerdict::NoInputs
    );
}

/// Behavioral pin for the counter itself (issue #244): a resolved page whose
/// parse yields no cells increments `pages_empty` — without it the run reads
/// "resolved, did nothing, says nothing" (the S5 pinned gap, now closed).
#[test]
fn a_resolved_page_with_no_cells_counts_pages_empty() {
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
    let mut summary = AggregatorPullSummary::default();
    let mut ledger = std::collections::BTreeMap::new();

    pull_one_page(
        &state,
        &company.id,
        "<html><body><p>strona bez tabeli finansowej</p></body></html>",
        "https://www.biznesradar.pl/raporty-finansowe-bilans/CDR",
        &Tolerance::default(),
        &mut summary,
        &mut ledger,
    )
    .expect("pull over a cell-less page");

    assert_eq!(summary.pages_empty, 1, "the empty parse must be counted");
    assert_eq!(summary.facts_written, 0);
}

/// The pull is the convergence cadence for ADR 0092 layers 2 and 3 (#273/#274)
/// — the one existing job that walks every tracked company exactly once a day.
/// Without this the layers would only ever land at company creation, so a
/// reclassified company (there is no `statement_type` setter to hang layer 2
/// off) and a company that just filed its third period would both stay stale.
#[test]
fn the_pull_converges_the_automatic_kpi_relevance_layers() {
    let (state, company_id) = state_with_company();

    // Reclassification after creation — exactly what has no other write seam.
    state
        .checkout_for_tests()
        .expect("connection")
        .execute(
            "UPDATE companies SET statement_type = 'banking' WHERE id = ?1",
            [&company_id],
        )
        .expect("reclassify");

    // Three years of an issuer-tier key the core floor does not cover.
    for fiscal_year in 2023..=2025 {
        seed_fact(
            &state,
            &company_id,
            "ebitda",
            fiscal_year,
            100,
            "esef",
            "esef",
        );
    }

    let sources = |state: &AppState| -> Vec<(String, String)> {
        let definitions = state
            .financials()
            .list_kpi_definitions(crate::storage::ListKpiDefinitionsInput {
                scope: None,
                sector: None,
                company_id: None,
            })
            .expect("definitions");
        let mut rows: Vec<(String, String)> = state
            .financials()
            .list_kpi_relevance(&company_id)
            .expect("relevance")
            .into_iter()
            .map(|r| {
                let key = definitions
                    .iter()
                    .find(|d| d.id == r.definition_id)
                    .map(|d| d.metric_key.clone())
                    .expect("definition");
                (key, r.source)
            })
            .collect();
        rows.sort();
        rows
    };

    assert!(
        !sources(&state)
            .iter()
            .any(|(_, source)| source == "sector" || source == "derived"),
        "precondition: neither layer has run yet"
    );

    // No fetcher installed: every page is unavailable. The layers must still
    // converge — they are per-company bookkeeping, not page-derived.
    run_aggregator_fundamentals_pull(&state).expect("pull should run");

    let rows = sources(&state);
    assert_eq!(
        rows.iter()
            .filter(|(_, source)| source == "sector")
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec![
            "net_fee_commission_income",
            "net_interest_income",
            "total_deposits",
            "total_loans",
        ],
        "layer 2 converges after the reclassification"
    );
    assert_eq!(
        rows.iter()
            .filter(|(_, source)| source == "derived")
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["ebitda"],
        "layer 3 picks up the consistently reported key"
    );

    // And the enrichment layer still never reaches the gate.
    let expected = state
        .financials()
        .expected_primary_metric_keys(&company_id)
        .expect("expected keys")
        .expect("denominator");
    assert!(!expected.contains("ebitda"));
    assert!(expected.contains("net_interest_income"));
}
