//! Ingest-time ESPI cover-note tier (ADR 0061 decision 1, tier 2a) — the
//! **integration** contract of the hook wired into
//! [`super::super::sources::ingest_bankier_company_items`]: a periodic-report
//! komunikat whose body carries the mandated "WYBRANE DANE FINANSOWE" table
//! yields validated `financial_facts` with `source_tier='espi_cover_note'` and a
//! provenance citation naming the FEED ITEM, at the moment the feed item lands.
//!
//! The row grammar itself is pinned by the parser's own suite (and by the
//! `#[ignore]` ground-truth corpus); these tests own the wiring: which items are
//! attempted, where the period comes from, that the validation gate is not
//! bypassed, that tier precedence holds against already-stored facts, that a
//! garbage body cannot break feed ingestion, and that abstentions / empty
//! reasons / underivable periods are observable rather than silent.
//!
//! Bodies are synthetic and authored here — structurally equivalent to the real
//! cover notes, never copied from `private/`.

use super::*;

/// A Q1 periodic-report body carrying a well-formed cover table. `revenue`
/// 100 000 tys → 100 000 000; `total_assets` 500 000 tys → 500 000 000.
const WDF_BODY: &str = "RAPORT OKRESOWY\n\
Spis: WYBRANE DANE FINANSOWE\n\
Skonsolidowany raport kwartalny\n\
WYBRANE DANE FINANSOWE w tys. PLN w tys. EUR\n\
I. Przychody ze sprzedaży 100 000 90 000 23 256 20 930\n\
II. Zysk netto 8 000 7 000 1 860 1 628\n\
III. Aktywa razem 500 000 480 000 116 279 111 628\n\
Zastosowane kursy: 4,3000\n";

/// Same form, but `Zysk netto` is a concatenated digit run the document's own
/// 4,3000 rate does not resolve to a single split — that row must abstain while
/// the unambiguous rows still emit.
const AMBIGUOUS_BODY: &str = "RAPORT OKRESOWY\n\
Spis: WYBRANE DANE FINANSOWE\n\
Skonsolidowany raport kwartalny\n\
WYBRANE DANE FINANSOWE w tys. PLN w tys. EUR\n\
I. Przychody ze sprzedaży 100 000 90 000 23 256 20 930\n\
II. Zysk netto 1801500\n\
III. Aktywa razem 500 000 480 000 116 279 111 628\n\
Zastosowane kursy: 4,3000\n";

/// A periodic report whose body carries no cover table at all.
const NO_TABLE_BODY: &str = "Skonsolidowany raport kwartalny\n\
Zarząd informuje, że treść raportu została przekazana w formie załącznika.\n\
!!! \u{fffd}\u{fffd} 12,, ((( ---\n";

fn company(state: &AppState, ticker: &str) -> Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: ticker.to_owned(),
            display_name: format!("{ticker} S.A."),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company created")
}

/// A Bankier komunikat whose title/URL name Q1 2026 — the period the tier
/// derives with the SAME title/URL derivation the document pipeline uses.
fn q1_item(company: &Company, article_id: &str, body: &str) -> BankierCompanyItem {
    item_with_title(company, article_id, "Raport kwartalny Q1 2026", body)
}

fn item_with_title(
    company: &Company,
    article_id: &str,
    title: &str,
    body: &str,
) -> BankierCompanyItem {
    BankierCompanyItem {
        company_id: company.id.clone(),
        qualified_ticker: company.qualified_ticker.clone(),
        title: title.to_owned(),
        link: format!("https://www.bankier.pl/wiadomosc/X-{article_id}.html"),
        summary: "Komunikat ESPI".to_owned(),
        published_at: Some("2026-05-01T09:00:00".to_owned()),
        fetched_at: "2026-05-01T10:00:00Z".to_owned(),
        article_id: article_id.to_owned(),
        pub_id: 3,
        dedupe_key: format!("bankier-company-komunikaty:article:{article_id}"),
        duplicate_signature: format!("official-secondary:GPW:{}:{article_id}", company.ticker),
        body_text: Some(body.to_owned()),
        attachments: Vec::new(),
        detail_fetch_attempted: true,
    }
}

/// Every stored fact for a company, keyed by metric key, with its provenance.
fn facts_by_metric(
    state: &AppState,
    company_id: &str,
) -> std::collections::BTreeMap<String, (FinancialFact, Option<FactProvenance>)> {
    let definitions = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: None,
        })
        .expect("definitions list");
    let facts = state
        .list_financial_facts(ListFinancialFactsInput {
            company_id: Some(company_id.to_owned()),
            period_id: None,
            definition_id: None,
        })
        .expect("facts list");
    facts
        .into_iter()
        .map(|fact| {
            let key = definitions
                .iter()
                .find(|d| d.id == fact.definition_id)
                .map(|d| d.metric_key.clone())
                .unwrap_or_else(|| fact.definition_id.clone());
            let provenance = state
                .fundamentals_provenance()
                .get_fact_provenance(&fact.id)
                .expect("provenance read");
            (key, (fact, provenance))
        })
        .collect()
}

/// Seeds an already-stored fact for Q1 2026 at a given tier, the way an earlier
/// document extraction would have left it.
fn seed_fact(state: &AppState, company_id: &str, metric_key: &str, value: &str, tier: &str) {
    state
        .kpi_extraction()
        .record_structured_fact(StructuredFactInput {
            company_id,
            fiscal_year: 2026,
            period_type: "Q1",
            period_end: Some("2026-03-31"),
            report_document_id: "repdoc_seed",
            metric_key,
            value_numeric: value,
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: tier,
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("seeded"),
        })
        .expect("seed fact");
}

fn diagnostic_stages(state: &AppState) -> Vec<(String, String)> {
    state
        .list_diagnostic_events(100)
        .expect("diagnostics list")
        .into_iter()
        .filter(|event| event.module == "espi_cover_note")
        .map(|event| (event.stage, event.metadata.to_string()))
        .collect()
}

/// The stored BiznesRadar witness sample — the SAME page the witness parser's
/// own drift guard pins. Its Q1-2026 column yields revenue 200 000 000,
/// net_profit 50 000 000, total_assets 45 000 000 (and more).
const WITNESS_SAMPLE: &str = include_str!("../../../samples/biznesradar_fundamenty_cdr.html");

/// A Q1 cover note whose figures MATCH the witness sample's 2026-03-31 column
/// (revenue 200 000 tys → 200 000 000; net 50 000; assets 45 000), so a cached
/// witness corroborates rather than disagrees. Same clean grammar as `WDF_BODY`.
const AGREEING_BODY: &str = "RAPORT OKRESOWY\n\
Spis: WYBRANE DANE FINANSOWE\n\
Skonsolidowany raport kwartalny\n\
WYBRANE DANE FINANSOWE w tys. PLN w tys. EUR\n\
I. Przychody ze sprzedaży 200 000 180 000 46 512 41 860\n\
II. Zysk netto 50 000 44 000 11 628 10 233\n\
III. Aktywa razem 45 000 40 000 10 465 9 302\n\
Zastosowane kursy: 4,3000\n";

/// Seeds a FRESH `Ok` witness page in the shared cadence cache for a company, so
/// the ingest-time cover-note comparison reads it WITHOUT fetching.
fn seed_witness_page(state: &AppState, company_id: &str, html: &str) {
    state
        .fundamentals_witness_cache()
        .put_kind(
            company_id,
            crate::storage::AggregatorPageKind::Income,
            "https://www.biznesradar.pl/raporty-finansowe-rachunek-zyskow-i-strat/CD-PROJEKT",
            WitnessPageStatus::Ok,
            Some(html),
        )
        .expect("seed witness cache page");
}

/// The recorded extraction outcome for a company's single ingested komunikat
/// slot (Q1 2026), keyed by the feed item the cover-note tier cites.
fn cover_note_outcome(state: &AppState, company_id: &str) -> Option<ExtractionOutcome> {
    let feed_item_id = state
        .list_feed_items()
        .expect("feed items")
        .first()
        .expect("a feed item landed")
        .id
        .clone();
    state
        .fundamentals_provenance()
        .get_extraction_outcome_for_slot(company_id, &feed_item_id, 2026, "Q1", "2026-03-31")
        .expect("outcome read")
}

// -- Witness corroboration at the cover-note seam (ADR 0085, EspiCoverNote) ----

#[test]
fn cover_note_facts_with_a_cached_agreeing_witness_record_corroboration() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = company(&state, "CDR");
    seed_witness_page(&state, &company.id, WITNESS_SAMPLE);

    state
        .ingest_bankier_company_items(&[q1_item(&company, "9100010", AGREEING_BODY)])
        .expect("ingest succeeds");

    // The cover-note facts landed unchanged (primary value stays).
    let facts = facts_by_metric(&state, &company.id);
    assert_eq!(
        facts.get("revenue").expect("revenue").0.value_numeric,
        "200000000"
    );

    let outcome =
        cover_note_outcome(&state, &company.id).expect("a corroboration outcome recorded");
    assert_eq!(outcome.acceptance, "accepted_via_witness");
    assert_eq!(outcome.reason_code, "emitted");
    assert_eq!(outcome.tier.as_deref(), Some("espi_cover_note"));
    let detail = outcome.detail_json.expect("corroboration detail");
    assert!(
        detail.contains("witnessCorroboration") && detail.contains("revenue"),
        "detail names the corroborated metrics: {detail}"
    );
}

#[test]
fn cover_note_facts_with_a_cached_disagreeing_witness_flag_without_overwriting() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = company(&state, "CDR");
    // The sample witness says revenue 200 000 000 / assets 45 000 000; `WDF_BODY`
    // says 100 000 000 / 500 000 000 — a real disagreement on every shared metric.
    seed_witness_page(&state, &company.id, WITNESS_SAMPLE);

    state
        .ingest_bankier_company_items(&[q1_item(&company, "9100011", WDF_BODY)])
        .expect("ingest succeeds");

    // The PRIMARY (cover-note) values are untouched — the witness flags, never
    // corrects (ADR 0085 decision 2).
    let facts = facts_by_metric(&state, &company.id);
    assert_eq!(
        facts.get("revenue").expect("revenue").0.value_numeric,
        "100000000"
    );
    assert_eq!(
        facts.get("total_assets").expect("assets").0.value_numeric,
        "500000000"
    );

    let outcome = cover_note_outcome(&state, &company.id).expect("a disagreement outcome recorded");
    assert_eq!(outcome.acceptance, "flagged");
    assert_eq!(outcome.reason_code, "witness_disagreement");
    let detail = outcome.detail_json.expect("disagreement diff");
    assert!(
        detail.contains("witnessDisagreements") && detail.contains("\"expected\":\"200000000\""),
        "diff carries the witness (expected) vs primary (actual) figures: {detail}"
    );
}

#[test]
fn cover_note_with_no_cached_witness_defers_without_fetching_and_never_false_agrees() {
    use crate::source_adapters::biznesradar_fundamentals::FundamentalsWitnessFetcher;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    // A fetcher that would count any request. The whole point of the cache-only
    // ingest path (ADR 0085 decision 3) is that it must NEVER be reached here.
    struct CountingFetcher(Arc<AtomicUsize>);
    impl FundamentalsWitnessFetcher for CountingFetcher {
        fn fetch_fundamentals(&self, _ticker: &str) -> Result<String, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(String::new())
        }
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let state = AppState::new(open_in_memory_database().expect("db"))
        .with_fundamentals_witness_fetcher(Arc::new(CountingFetcher(Arc::clone(&calls))));
    // Developer mode on so the "pending" diagnostic is recorded (the same
    // precondition the other diagnostic-observing tests use).
    state
        .set_developer_mode_enabled(true)
        .expect("developer mode enables");
    let company = company(&state, "CDR");
    // No witness page seeded → cache miss.

    state
        .ingest_bankier_company_items(&[q1_item(&company, "9100012", WDF_BODY)])
        .expect("ingest never fails from a missing witness");

    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "the ingest path must not fetch the witness synchronously"
    );
    // The facts still landed — the missing witness never blocks emission.
    assert!(!facts_by_metric(&state, &company.id).is_empty());
    // No agreement or disagreement was invented: the comparison is DEFERRED, so
    // no witness-verdict outcome row exists for the slot.
    let outcome = cover_note_outcome(&state, &company.id);
    assert!(
        outcome.is_none(),
        "a deferred comparison records no witness verdict, got {outcome:?}"
    );
    // The deferral is observable as a diagnostic, never a false agreement.
    let stages = diagnostic_stages(&state);
    assert!(
        stages.iter().any(|(stage, _)| stage == "witness_pending"),
        "a pending comparison is recorded as a diagnostic: {stages:?}"
    );
}

// -- Test 1: end-to-end ingest ------------------------------------------------

#[test]
fn ingesting_a_periodic_komunikat_persists_cover_note_facts_with_feed_item_provenance() {
    let connection = open_in_memory_database().expect("database initializes");
    let state = AppState::new(connection);
    let company = company(&state, "CDR");

    state
        .ingest_bankier_company_items(&[q1_item(&company, "9100001", WDF_BODY)])
        .expect("ingest succeeds");

    let feed_item_id = state
        .list_feed_items()
        .expect("feed items list")
        .first()
        .expect("the komunikat landed as a feed item")
        .id
        .clone();

    let facts = facts_by_metric(&state, &company.id);
    let (revenue, revenue_provenance) = facts
        .get("revenue")
        .expect("the cover-note revenue row landed as a fact");
    assert_eq!(revenue.value_numeric, "100000000");
    let (assets, _) = facts
        .get("total_assets")
        .expect("the cover-note total-assets row landed as a fact");
    assert_eq!(assets.value_numeric, "500000000");

    let provenance = revenue_provenance
        .as_ref()
        .expect("every persisted fact carries provenance");
    assert_eq!(provenance.source_tier, "espi_cover_note");
    let citation = provenance
        .citation
        .as_deref()
        .expect("the tier writes a citation");
    assert!(
        citation.contains(&feed_item_id),
        "the citation must identify the source komunikat (the carrier body is prunable), got {citation}"
    );
}

// -- Test 2: tier precedence --------------------------------------------------

#[test]
fn a_cover_note_never_outranks_esef_and_always_outranks_pdf() {
    let connection = open_in_memory_database().expect("database initializes");
    let state = AppState::new(connection);
    let company = company(&state, "PKN");

    // An ESEF fact and a PDF fact already occupy their Q1 2026 slots, both with
    // values the cover note disagrees with.
    seed_fact(&state, &company.id, "revenue", "111111111", "esef");
    seed_fact(&state, &company.id, "total_assets", "222222222", "pdf");

    state
        .ingest_bankier_company_items(&[q1_item(&company, "9100002", WDF_BODY)])
        .expect("ingest succeeds");

    let facts = facts_by_metric(&state, &company.id);

    let (revenue, revenue_provenance) = facts.get("revenue").expect("revenue fact");
    assert_eq!(
        revenue.value_numeric, "111111111",
        "an ESEF fact outranks the cover note and must be left untouched"
    );
    assert_eq!(
        revenue_provenance.as_ref().expect("provenance").source_tier,
        "esef",
        "the higher tier keeps its provenance"
    );

    let (assets, assets_provenance) = facts.get("total_assets").expect("total assets fact");
    assert_eq!(
        assets.value_numeric, "500000000",
        "the cover note outranks the PDF tier and must replace its value"
    );
    assert_eq!(
        assets_provenance.as_ref().expect("provenance").source_tier,
        "espi_cover_note",
        "the upgraded fact records the tier that produced its value"
    );

    let feed_item_id = state
        .list_feed_items()
        .expect("feed items list")
        .first()
        .expect("the komunikat landed")
        .id
        .clone();
    assert_eq!(
        assets.source_document_ref.as_deref(),
        Some(feed_item_id.as_str()),
        "an upgraded fact must point at the komunikat that supplied the winning value, \
         not at the document the outranked value came from"
    );
}

// -- Test 2b: a tier upgrade is a visible disagreement, not a silent swap -----

/// Guardrail (ADR 0061 dec. 3 drift loop / ADR 0084 "never silently wrong"):
/// when a higher tier replaces a stored value, the LOWER tier was wrong about
/// that slot — that is drift evidence. Recording only an `upgraded` counter
/// hides which metric changed and from what, so the learning loop and the
/// flagged-review surface have nothing to act on. This test reddens if the
/// per-upgrade detail (metric, previous value, previous tier, new value) stops
/// being recorded.
#[test]
fn upgrading_a_lower_tier_value_records_the_disagreement_in_detail() {
    let connection = open_in_memory_database().expect("database initializes");
    let state = AppState::new(connection);
    let company = company(&state, "PKN");
    state
        .set_developer_mode_enabled(true)
        .expect("developer mode enables");

    // A PDF-tier value the cover note disagrees with.
    seed_fact(&state, &company.id, "total_assets", "222222222", "pdf");

    state
        .ingest_bankier_company_items(&[q1_item(&company, "9100010", WDF_BODY)])
        .expect("ingest succeeds");

    let stages = diagnostic_stages(&state);
    let upgrade = stages
        .iter()
        .find(|(stage, _)| stage == "tier_upgrade")
        .unwrap_or_else(|| panic!("expected a tier_upgrade diagnostic; got: {stages:?}"))
        .clone();

    let metadata = upgrade.1;
    assert!(
        metadata.contains("total_assets"),
        "the diagnostic must name the metric whose stored value was replaced: {metadata}"
    );
    assert!(
        metadata.contains("222222222"),
        "the diagnostic must record the PREVIOUS value, else the drift evidence is lost: {metadata}"
    );
    assert!(
        metadata.contains("\"previousTier\":\"pdf\""),
        "the diagnostic must record which tier was outranked: {metadata}"
    );
    assert!(
        metadata.contains("500000000"),
        "the diagnostic must record the winning value: {metadata}"
    );
}

// -- Test 3: ingest robustness ------------------------------------------------

#[test]
fn a_garbage_body_ingests_the_feed_item_and_persists_no_facts() {
    let connection = open_in_memory_database().expect("database initializes");
    let state = AppState::new(connection);
    let company = company(&state, "PKO");
    state
        .set_developer_mode_enabled(true)
        .expect("developer mode enables");

    let result = state
        .ingest_bankier_company_items(&[q1_item(&company, "9100003", NO_TABLE_BODY)])
        .expect("feed ingestion must succeed regardless of the extraction outcome");

    assert_eq!(result.items_created, 1, "the feed item still lands");
    assert_eq!(
        state.list_feed_items().expect("feed items list").len(),
        1,
        "feed ingestion is never rolled back by an extraction problem"
    );
    assert!(
        facts_by_metric(&state, &company.id).is_empty(),
        "an unparseable body must persist nothing — never a guess"
    );

    let stages = diagnostic_stages(&state);
    assert!(
        stages.iter().any(|(stage, _)| stage == "empty"),
        "the empty outcome must be observable, got {stages:?}"
    );
}

// -- Test 4: abstention propagation -------------------------------------------

#[test]
fn an_fx_refused_row_abstains_while_the_unambiguous_rows_persist() {
    let connection = open_in_memory_database().expect("database initializes");
    let state = AppState::new(connection);
    let company = company(&state, "KGH");
    state
        .set_developer_mode_enabled(true)
        .expect("developer mode enables");

    state
        .ingest_bankier_company_items(&[q1_item(&company, "9100004", AMBIGUOUS_BODY)])
        .expect("ingest succeeds");

    let facts = facts_by_metric(&state, &company.id);
    assert!(
        facts.contains_key("revenue") && facts.contains_key("total_assets"),
        "the unambiguous rows still persist, got {:?}",
        facts.keys().collect::<Vec<_>>()
    );
    assert!(
        !facts.contains_key("net_profit"),
        "a row the PLN↔EUR cross-check refused must NEVER be persisted as a guess"
    );

    let stages = diagnostic_stages(&state);
    let emitted = stages
        .iter()
        .find(|(stage, _)| stage == "emitted")
        .expect("the emitting outcome is recorded");
    assert!(
        emitted.1.contains("\"abstained\":1"),
        "the abstention must be visible in the recorded outcome, got {}",
        emitted.1
    );
}

// -- Test 5: no derivable period ----------------------------------------------

#[test]
fn a_komunikat_with_no_derivable_period_persists_nothing_and_records_why() {
    let connection = open_in_memory_database().expect("database initializes");
    let state = AppState::new(connection);
    let company = company(&state, "PZU");
    state
        .set_developer_mode_enabled(true)
        .expect("developer mode enables");

    // A komunikat whose title names a report but no period at all. (Until v0.59
    // this sample was "Skonsolidowany raport kwartalny QSr 1/2026" — a form the
    // widened grammar now reads as Q1 2026, see `also_derives_the_qsr_form`
    // below. The behaviour under test is the abstention, not that particular
    // title, so the sample moved to one that genuinely states nothing.)
    state
        .ingest_bankier_company_items(&[item_with_title(
            &company,
            "9100005",
            "Skonsolidowany raport okresowy Grupy Kapitalowej",
            WDF_BODY,
        )])
        .expect("ingest succeeds");

    assert!(
        facts_by_metric(&state, &company.id).is_empty(),
        "an underivable period must abstain — the period is never guessed"
    );

    let stages = diagnostic_stages(&state);
    assert!(
        stages.iter().any(|(stage, _)| stage == "no_period"),
        "the abstention reason must be observable, got {stages:?}"
    );
}

// -- Test 5b: the widened grammar reaches the ingest tier too ------------------

#[test]
fn also_derives_the_qsr_form_the_derivation_used_to_miss() {
    // Card fc692da: `QSr N/YYYY` is a standard GPW quarterly-report title and was
    // previously underivable — the cover-note tier abstained on every one of
    // them. The document pipeline and this ingest tier share one derivation, so
    // widening it must light up here too, with facts actually persisted.
    let connection = open_in_memory_database().expect("database initializes");
    let state = AppState::new(connection);
    let company = company(&state, "PZU");

    state
        .ingest_bankier_company_items(&[item_with_title(
            &company,
            "9100006",
            "Skonsolidowany raport kwartalny QSr 1/2026",
            WDF_BODY,
        )])
        .expect("ingest succeeds");

    assert!(
        !facts_by_metric(&state, &company.id).is_empty(),
        "a QSr title now derives Q1 2026 and the cover note is read"
    );
}
