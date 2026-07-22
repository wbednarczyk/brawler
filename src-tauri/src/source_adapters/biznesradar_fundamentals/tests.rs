//! Tests for the BiznesRadar fundamentals witness (ADR 0085).
//!
//! No test here touches the network: the only fetcher any of them installs is a
//! counting stub. The production `AppState` constructors used by tests leave the
//! fetcher slot empty by construction, so "forgot to stub it" fails as
//! `witness_not_configured`, never as a live request.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::*;
use crate::app_state::AppState;
use crate::storage::{open_in_memory_database, NewCompany};

const SAMPLE: &str = include_str!("../../../samples/biznesradar_fundamenty_cdr.html");
const LANDING: &str = include_str!("../../../samples/biznesradar_fundamenty_landing.html");

/// A fetcher that counts calls and replays a canned response. The counter is the
/// point: several assertions here are about a fetch NOT happening, which "the
/// values did not change" cannot distinguish from "it fetched and agreed".
struct SpyFetcher {
    calls: Arc<AtomicUsize>,
    response: Result<String, String>,
}

impl SpyFetcher {
    fn ok(body: &str) -> (Arc<Self>, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        (
            Arc::new(Self {
                calls: Arc::clone(&calls),
                response: Ok(body.to_owned()),
            }),
            calls,
        )
    }

    fn failing() -> (Arc<Self>, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        (
            Arc::new(Self {
                calls: Arc::clone(&calls),
                response: Err("connection reset".to_owned()),
            }),
            calls,
        )
    }
}

impl FundamentalsWitnessFetcher for SpyFetcher {
    fn fetch_fundamentals(&self, _ticker: &str) -> Result<String, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.response.clone()
    }
}

fn state_with_company(ticker: &str) -> (AppState, String) {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: ticker.to_owned(),
            display_name: format!("{ticker} S.A."),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    (state, company.id)
}

fn value(facts: &[crate::fundamentals::extraction::ExtractedFact], key: &str) -> Option<String> {
    facts
        .iter()
        .find(|fact| fact.metric_key == key)
        .map(|fact| fact.value.to_string())
}

// ---------------------------------------------------------------------------
// Drift guard
// ---------------------------------------------------------------------------

#[test]
fn stored_sample_page_pins_the_witness_parse() {
    // ADR 0085 §Consequences: markup drift is the witness's failure mode, and it
    // must fail LOUD. This pins the exact figures the stored sample yields, so a
    // parser or markup change reddens here instead of quietly degrading every
    // company to "no witness" — a state that otherwise looks identical to
    // "BiznesRadar does not cover this company" and would never be noticed.
    let facts = parse_witness_page(SAMPLE, "2026-03-31", 2026, Some("Q1"));

    assert_eq!(value(&facts, "revenue").as_deref(), Some("200000000"));
    assert_eq!(
        value(&facts, "operating_profit").as_deref(),
        Some("60000000")
    );
    assert_eq!(value(&facts, "net_profit").as_deref(), Some("50000000"));
    assert_eq!(value(&facts, "total_assets").as_deref(), Some("45000000"));
    assert_eq!(
        value(&facts, "total_liabilities").as_deref(),
        Some("20000000")
    );
    assert_eq!(value(&facts, "total_equity").as_deref(), Some("25000000"));
    assert!(
        facts
            .iter()
            .all(|fact| fact.tier == crate::fundamentals::extraction::SourceTier::HtmlAggregator),
        "witness facts must carry the aggregator tier, never a primary one"
    );
}

#[test]
fn other_period_column_is_read_from_the_same_sample() {
    // The column selector is the other half of the drift surface: a header shift
    // would silently read the wrong period's numbers into a "disagreement".
    let facts = parse_witness_page(SAMPLE, "2025-03-31", 2025, Some("Q1"));
    assert_eq!(value(&facts, "revenue").as_deref(), Some("180000000"));
    assert_eq!(value(&facts, "net_profit").as_deref(), Some("44000000"));
}

#[test]
fn landing_page_is_no_coverage_not_drift() {
    // ADR 0085 decision 5: an unresolvable slug is a normal, non-error state, and
    // it must be structurally distinguishable from a page that HAS a report table
    // we failed to parse (that one is drift, above).
    assert!(!has_report_table(LANDING));
    assert!(has_report_table(SAMPLE));
}

// ---------------------------------------------------------------------------
// Politeness / degradation / source health of the page resolve
// (the fetching `resolve_witness` is retired with ADR 0086 — the same
// fetch/cache/health properties now live on `resolve_aggregator_page`, the
// BR-primary pull's page resolver; per-page cadence itself is asserted in
// `jobs::aggregator_fundamentals_pull::tests`)
// ---------------------------------------------------------------------------

#[test]
fn a_failed_fetch_is_cached_too_so_a_dead_host_is_not_hammered() {
    // Politeness has to survive the unhappy path: caching only successes would
    // let a broken slug or a down host cost one request per pull pass.
    let (state, company_id) = state_with_company("CDR");
    let (fetcher, calls) = SpyFetcher::failing();
    let state = state.with_fundamentals_witness_fetcher(fetcher);

    for _ in 0..3 {
        assert!(matches!(
            resolve_aggregator_page(&state, &company_id, AggregatorPageKind::Income),
            PageResolution::Unavailable(WitnessUnavailable::FetchFailed)
        ));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn every_degraded_path_reports_unavailable_and_never_a_page() {
    // ADR 0085 decision 5 (posture unchanged under ADR 0086): each of these is a
    // NORMAL state; none may present as a readable page.
    let (no_fetcher_state, company_id) = state_with_company("CDR");
    assert!(matches!(
        resolve_aggregator_page(&no_fetcher_state, &company_id, AggregatorPageKind::Income),
        PageResolution::Unavailable(WitnessUnavailable::NotConfigured)
    ));

    let (landing_state, landing_company) = state_with_company("XYZ");
    let (fetcher, _) = SpyFetcher::ok(LANDING);
    let landing_state = landing_state.with_fundamentals_witness_fetcher(fetcher);
    assert!(matches!(
        resolve_aggregator_page(&landing_state, &landing_company, AggregatorPageKind::Income),
        PageResolution::Unavailable(WitnessUnavailable::NoCoverage)
    ));

    // Every unavailable resolution yields no facts to the ingest-time comparison
    // — emission is never blocked by the aggregator being unreachable.
    for reason in [
        WitnessUnavailable::NotConfigured,
        WitnessUnavailable::NoSlug,
        WitnessUnavailable::FetchFailed,
        WitnessUnavailable::NoCoverage,
        WitnessUnavailable::NoFactsForPeriod,
    ] {
        assert!(WitnessResolution::Unavailable(reason).facts().is_none());
    }
}

#[test]
fn an_unresolvable_slug_is_no_page_not_an_error() {
    // ADR 0085 decision 4: a company with no ticker cannot address a page. That
    // is a normal non-error state, and it must not consume the fetch budget.
    let state = AppState::new(open_in_memory_database().expect("db"));
    let (fetcher, calls) = SpyFetcher::ok(SAMPLE);
    let state = state.with_fundamentals_witness_fetcher(fetcher);

    assert!(matches!(
        resolve_aggregator_page(&state, "no-such-company", AggregatorPageKind::Income),
        PageResolution::Unavailable(WitnessUnavailable::NoSlug)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

// ---------------------------------------------------------------------------
// Source health (Definition of Done §C)
// ---------------------------------------------------------------------------

#[test]
fn a_successful_refresh_sets_last_success_at_on_the_catalog_row() {
    // DoD §C / ADR 0085 decision 6: a source path that forgets
    // `record_source_outcome` shows as "never refreshed" forever on the Sources
    // screen, which is worse than an error — it looks like nothing is wired.
    let (state, company_id) = state_with_company("CDR");
    let (fetcher, _) = SpyFetcher::ok(SAMPLE);
    let state = state.with_fundamentals_witness_fetcher(fetcher);

    let before = state
        .list_source_adapters()
        .expect("adapters")
        .into_iter()
        .find(|adapter| adapter.id == ADAPTER_ID)
        .expect("the aggregator must be in the user-visible catalog");
    assert!(before.last_success_at.is_none());

    resolve_aggregator_page(&state, &company_id, AggregatorPageKind::Income);

    let after = state
        .list_source_adapters()
        .expect("adapters")
        .into_iter()
        .find(|adapter| adapter.id == ADAPTER_ID)
        .expect("aggregator adapter");
    assert!(
        after.last_success_at.is_some(),
        "a successful page resolve must mark the adapter healthy"
    );
}

#[test]
fn a_no_coverage_page_still_counts_as_a_healthy_refresh() {
    // The fetch succeeded; the company simply is not covered. Reporting that as a
    // source ERROR would make an entirely normal state look like an outage.
    let (state, company_id) = state_with_company("XYZ");
    let (fetcher, _) = SpyFetcher::ok(LANDING);
    let state = state.with_fundamentals_witness_fetcher(fetcher);

    resolve_aggregator_page(&state, &company_id, AggregatorPageKind::Income);

    let adapter = state
        .list_source_adapters()
        .expect("adapters")
        .into_iter()
        .find(|adapter| adapter.id == ADAPTER_ID)
        .expect("aggregator adapter");
    assert!(adapter.last_success_at.is_some());
    assert!(adapter.last_error.is_none());
}

#[test]
fn a_background_pull_does_not_present_as_a_refresh_in_flight() {
    // Live regression (2026-07-21, v0.59 closure): the report pages are pulled by
    // a background job, NOT by a user/scheduler source refresh. If the pull
    // stamps the same `last_attempt_at`/`last_trigger` "a refresh was initiated"
    // markers a real adapter records, the Sources row's "Last attempt" advances
    // ahead of "Last success" on every pull — over a warm-up of N tracked
    // companies the row reads as a source that is continuously refreshing. The
    // background pull must record its OUTCOME (freshness/health) but must not
    // impersonate an in-flight refresh.
    let (state, company_id) = state_with_company("CDR");
    let (fetcher, _) = SpyFetcher::ok(SAMPLE);
    let state = state.with_fundamentals_witness_fetcher(fetcher);

    resolve_aggregator_page(&state, &company_id, AggregatorPageKind::Income);

    let adapter = state
        .list_source_adapters()
        .expect("adapters")
        .into_iter()
        .find(|adapter| adapter.id == ADAPTER_ID)
        .expect("aggregator adapter");
    // Honest freshness is preserved (ADR 0085 decision 6, DoD §C).
    assert!(
        adapter.last_success_at.is_some(),
        "the background pull must still record its outcome for honest freshness"
    );
    // But it must NOT have recorded a source-refresh attempt: those markers are
    // what make the row present as an actively-refreshing source.
    assert!(
        adapter.last_attempt_at.is_none(),
        "a background pull must not stamp last_attempt_at (refresh-in-flight marker)"
    );
    assert!(
        adapter.last_trigger.is_none(),
        "a background pull must not stamp a refresh trigger"
    );
}

#[test]
fn a_real_source_refresh_attempt_still_records_its_in_flight_markers() {
    // The other half of the regression fix: de-impersonating the witness must not
    // silence the genuine attempt markers a real user/scheduler refresh records —
    // those are how the Sources screen shows an actual refresh in flight.
    let (state, _company_id) = state_with_company("CDR");
    state
        .record_source_adapter_attempt("biznesradar-akcjonariat", "manual")
        .expect("record attempt");

    let adapter = state
        .list_source_adapters()
        .expect("adapters")
        .into_iter()
        .find(|adapter| adapter.id == "biznesradar-akcjonariat")
        .expect("a real refreshable adapter");
    assert!(adapter.last_attempt_at.is_some());
    assert_eq!(adapter.last_trigger.as_deref(), Some("manual"));
}

// ---------------------------------------------------------------------------
// Catalog identity
// ---------------------------------------------------------------------------

#[test]
fn the_aggregator_is_registered_as_primary() {
    // ADR 0086 decision 2: BiznesRadar is promoted from witness to PRIMARY for
    // core KPIs (code-side role, ADR 0072 pattern). The role is what the Sources
    // UI renders and what tells a future reader this source may source facts.
    let descriptor = crate::source_adapters::registry::descriptor(ADAPTER_ID)
        .expect("the aggregator needs a registry descriptor");
    assert_eq!(descriptor.role.as_str(), "primary");
    assert_eq!(descriptor.visibility.as_str(), "optional");
    assert_eq!(descriptor.source_type, "fundamentals");
}

#[test]
fn each_report_kind_has_its_robots_allowed_page_url() {
    // ADR 0086 decision 2 / ADR 0085 robots quote: the three /raporty-finansowe-*
    // pages the primary pull fetches, ticker appended for the canonical-slug 301.
    use crate::storage::AggregatorPageKind;
    assert_eq!(
        page_url_for(AggregatorPageKind::Income, "CDR"),
        "https://www.biznesradar.pl/raporty-finansowe-rachunek-zyskow-i-strat/CDR"
    );
    assert_eq!(
        page_url_for(AggregatorPageKind::Balance, "CDR"),
        "https://www.biznesradar.pl/raporty-finansowe-bilans/CDR"
    );
    assert_eq!(
        page_url_for(AggregatorPageKind::Cashflow, "CDR"),
        "https://www.biznesradar.pl/raporty-finansowe-przeplywy-pieniezne/CDR"
    );
}

// ---------------------------------------------------------------------------
// Aggregator-zero guard (ADR 0085 amendment 2026-07-21; BFT H1 2025 incident)
// ---------------------------------------------------------------------------

/// Build a `FactSet` from `(metric_key, base-unit value)` pairs.
fn fact_set(pairs: &[(&str, i64)]) -> crate::fundamentals::validation::FactSet {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), rust_decimal::Decimal::from(*value)))
        .collect()
}

#[test]
fn witness_zero_against_a_non_zero_parse_is_not_a_disagreement() {
    // BFT H1 2025: the aggregator column rendered `net_profit` as a placeholder
    // `0` while the filed report (and our primary parse) said 242 454 000. That
    // `0` is a scrape/cache artifact, never a filed value, so it must NOT block
    // emission. An agreeing metric rides along to prove the run still corroborates
    // on the real overlap rather than collapsing to an empty (→ Flagged) set.
    let primary = fact_set(&[("net_profit", 242_454_000), ("revenue", 500_000_000)]);
    let witness = fact_set(&[("net_profit", 0), ("revenue", 500_000_000)]);
    let tol = crate::fundamentals::validation::Tolerance::default();

    let checks = witness_cross_check(&primary, &witness, &tol);

    assert!(
        !checks.iter().any(|c| c.metric_key == "net_profit"),
        "a zero aggregator cell against a non-zero parse must be dropped, not recorded as a Fail"
    );
    assert!(
        checks.iter().all(|c| c.outcome.is_pass()),
        "the remaining overlap must still corroborate"
    );
    assert!(
        checks.iter().any(|c| c.metric_key == "revenue"),
        "genuinely overlapping agreeing metrics are preserved so the run still corroborates"
    );
}

#[test]
fn a_genuine_witness_disagreement_still_fails() {
    // Both sides non-zero and beyond tolerance: a real conflict the witness must
    // still flag. The zero-guard must not swallow this.
    let primary = fact_set(&[("net_profit", 100_000)]);
    let witness = fact_set(&[("net_profit", 100)]);
    let tol = crate::fundamentals::validation::Tolerance::default();

    let checks = witness_cross_check(&primary, &witness, &tol);

    assert_eq!(checks.len(), 1);
    assert!(
        checks[0].outcome.is_fail(),
        "a non-zero-vs-non-zero mismatch beyond tolerance must remain a disagreement"
    );
}

#[test]
fn zero_witness_against_zero_parse_stays_a_pass() {
    // 0 vs 0 is exact agreement (residual 0 → Pass), never a Fail, so the guard's
    // Fail-only filter leaves it in place as a genuine corroboration.
    let primary = fact_set(&[("net_profit", 0)]);
    let witness = fact_set(&[("net_profit", 0)]);
    let tol = crate::fundamentals::validation::Tolerance::default();

    let checks = witness_cross_check(&primary, &witness, &tol);

    assert_eq!(checks.len(), 1);
    assert!(
        checks[0].outcome.is_pass(),
        "0 vs 0 is agreement and must be kept as a passing corroboration"
    );
}

#[test]
fn a_zero_primary_against_a_non_zero_witness_is_still_flagged() {
    // The guard is keyed on the AGGREGATOR (`expected`) cell only. A primary parse
    // of 0 against a filed non-zero witness is the opposite direction and stays a
    // disagreement — the guard must not make emission blind to a dropped primary.
    let primary = fact_set(&[("net_profit", 0)]);
    let witness = fact_set(&[("net_profit", 242_454_000)]);
    let tol = crate::fundamentals::validation::Tolerance::default();

    let checks = witness_cross_check(&primary, &witness, &tol);

    assert_eq!(checks.len(), 1);
    assert!(
        checks[0].outcome.is_fail(),
        "a zero primary against a non-zero aggregator value must still disagree"
    );
}

#[test]
fn the_page_url_appends_the_ticker_for_the_canonical_slug_redirect() {
    // ADR 0085 decision 4: ticker != slug (CDPROJEKT -> CD-PROJEKT), and we
    // deliberately do NOT maintain a mapping table — the host's 301 resolves it,
    // the same mechanism both live BiznesRadar adapters rely on.
    assert_eq!(
        page_url("CDPROJEKT"),
        "https://www.biznesradar.pl/raporty-finansowe-rachunek-zyskow-i-strat/CDPROJEKT"
    );
}
