//! Tests for the `rebuild fundamentals` driver (ADR 0086 dec. 6, plan TOR C
//! slice C4).
//!
//! No test touches the network: the only aggregator fetcher installed is a
//! per-kind stub replaying a stored sample page. The rebuild is exercised over a
//! state carrying one of each spec-driven surface — a stored ESEF-routed
//! document, a stored WDF cover-note carrier, and a cached BiznesRadar page — so
//! a single run must repopulate facts from all three tiers, and a second run must
//! duplicate nothing.

use std::collections::HashMap;

use super::*;
use crate::source_adapters::bankier_company::ADAPTER_ID as BANKIER_ADAPTER_ID;
use crate::source_adapters::biznesradar_fundamentals::FundamentalsWitnessFetcher;
use crate::storage::{
    open_in_memory_database, AggregatorPageKind, AppState, CaptureReportDocumentInput, NewCompany,
};

const BILANS: &str = include_str!("../../../samples/biznesradar_bilans_cdr.html");

/// A balanced iXBRL statement — the ESEF-route document the rebuild re-extracts.
const ESEF: &str = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
  xmlns:xbrli="http://www.xbrl.org/2003/instance"
  xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
  <xbrli:context id="c"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
  <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
  <ix:nonFraction name="ifrs-full:Assets" contextRef="c" unitRef="pln" scale="3">45 000</ix:nonFraction>
  <ix:nonFraction name="ifrs-full:Liabilities" contextRef="c" unitRef="pln" scale="3">20 000</ix:nonFraction>
  <ix:nonFraction name="ifrs-full:Equity" contextRef="c" unitRef="pln" scale="3">25 000</ix:nonFraction>
</html>"#;

/// A Q1 periodic-report body carrying a well-formed WDF cover table (revenue
/// 100 000 tys → 100 000 000, net_profit, total_assets). Structurally equivalent
/// to the real notes, authored here — never owner-private data.
const WDF_BODY: &str = "RAPORT OKRESOWY\n\
Spis: WYBRANE DANE FINANSOWE\n\
Skonsolidowany raport kwartalny\n\
WYBRANE DANE FINANSOWE w tys. PLN w tys. EUR\n\
I. Przychody ze sprzedaży 100 000 90 000 23 256 20 930\n\
II. Zysk netto 8 000 7 000 1 860 1 628\n\
III. Aktywa razem 500 000 480 000 116 279 111 628\n\
Zastosowane kursy: 4,3000\n";

/// A fetcher replaying a canned balance page (income/cash-flow fall back to the
/// landing = "no coverage").
struct MapFetcher {
    by_kind: HashMap<&'static str, String>,
}

impl MapFetcher {
    fn balance() -> std::sync::Arc<Self> {
        let mut by_kind = HashMap::new();
        by_kind.insert(AggregatorPageKind::Balance.as_str(), BILANS.to_owned());
        std::sync::Arc::new(Self { by_kind })
    }
    fn body(&self, kind: AggregatorPageKind) -> String {
        self.by_kind
            .get(kind.as_str())
            .cloned()
            .unwrap_or_else(|| "<html><body>brak danych</body></html>".to_owned())
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

fn unique_dir() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "brawler-rebuild-fundamentals-{}-{n}",
        std::process::id()
    ))
}

/// Seed a company + a stored ESEF-routed periodic document + a stored WDF
/// cover-note carrier feed item, and install the canned aggregator fetcher.
fn seed_state() -> (AppState, String) {
    let dir = unique_dir();
    std::fs::create_dir_all(&dir).expect("temp dir");
    let state = AppState::with_data_dir(open_in_memory_database().expect("db"), dir.clone());
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

    // (1) Stored ESEF-route document — the period derives from the title/URL the
    // structured pipeline uses.
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://example.com/skonsolidowany-raport-roczny-2025-ssf.xhtml".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Skonsolidowany raport roczny 2025 SSF".to_owned()),
            attribution: None,
        })
        .expect("document");
    // A package, not a bare instance (ADR 0100 decision 3, epic #398): a bare
    // iXBRL instance has no presentation linkbase to read, so its facts get
    // no role rows and never survive Layer 2 projection.
    let esef_package = crate::test_support::esef_package_zip(
        ESEF,
        &[
            ("Assets", crate::test_support::BALANCE_SHEET_ROLE_SUFFIX),
            (
                "Liabilities",
                crate::test_support::BALANCE_SHEET_ROLE_SUFFIX,
            ),
            ("Equity", crate::test_support::BALANCE_SHEET_ROLE_SUFFIX),
        ],
    );
    std::fs::write(dir.join("report.xhtml"), &esef_package).expect("write esef");
    state
        .mark_report_document_fetched(
            &document.id,
            Some("report.xhtml"),
            Some("application/octet-stream"),
            None,
            Some(esef_package.len() as i64),
        )
        .expect("mark fetched");

    // (2) Stored WDF cover-note carrier feed item (raw seed — no ingest hook, so
    // the re-scan is what creates the espi_cover_note facts).
    {
        let connection = state.checkout_for_tests().expect("connection");
        connection
            .execute(
                "
                INSERT INTO feed_items (
                    id, type, source_adapter_id, source_name, source_url, title,
                    body_text, language, fetched_at, dedupe_key, attribution, display_company
                ) VALUES (?1, 'Official report', ?2, 'Bankier', ?3, ?4, ?5, 'pl',
                          '2026-05-01T10:00:00Z', ?6, 'GPW', 'GPW:CDR')
                ",
                rusqlite::params![
                    "feed_wdf_1",
                    BANKIER_ADAPTER_ID,
                    "https://www.bankier.pl/wiadomosc/X-9100010.html",
                    "Raport kwartalny Q1 2026",
                    WDF_BODY,
                    "bankier-company-komunikaty:article:9100010",
                ],
            )
            .expect("insert wdf feed item");
        connection
            .execute(
                "INSERT INTO feed_item_companies (feed_item_id, company_id, match_type)
                 VALUES (?1, ?2, 'bankier_tag_id')",
                rusqlite::params!["feed_wdf_1", company.id],
            )
            .expect("map feed item to company");
    }

    let state = state.with_fundamentals_witness_fetcher(MapFetcher::balance());
    (state, company.id)
}

/// Total facts carrying the given `source_tier`, from the rebuild verdict.
fn tier(summary: &RebuildFundamentalsSummary, tier: &str) -> i64 {
    summary
        .facts_by_tier
        .by_tier
        .iter()
        .find(|entry| entry.source_tier == tier)
        .map(|entry| entry.facts)
        .unwrap_or(0)
}

#[test]
fn rebuild_populates_facts_from_all_three_tiers() {
    let (state, _company_id) = seed_state();

    let summary = run_rebuild_fundamentals(&state).expect("rebuild runs");

    // Pass 1 — the aggregator wrote html_aggregator facts from the balance page.
    assert!(
        summary.aggregator.facts_written > 0,
        "aggregator pass writes facts: {summary:?}"
    );
    assert!(
        tier(&summary, "html_aggregator") > 0,
        "html_aggregator facts present after rebuild: {summary:?}"
    );

    // Pass 2 — the ESEF document was re-extracted into esef facts.
    assert!(
        summary.esef_documents_processed >= 1,
        "one ESEF document is re-extracted: {summary:?}"
    );
    assert!(
        tier(&summary, "esef") > 0,
        "esef facts present after rebuild: {summary:?}"
    );

    // Pass 3 — the stored WDF carrier was re-scanned into espi_cover_note facts.
    assert!(
        summary.wdf_carriers_scanned >= 1,
        "one WDF carrier is scanned: {summary:?}"
    );
    assert!(
        summary.wdf_facts_written > 0,
        "the WDF re-scan writes facts: {summary:?}"
    );
    assert!(
        tier(&summary, "espi_cover_note") > 0,
        "espi_cover_note facts present after rebuild: {summary:?}"
    );

    // No manual facts exist on a fresh rebuild (the automaton never stamps manual).
    assert_eq!(
        summary.facts_by_tier.manual_or_unprovenanced, 0,
        "no manual / unprovenanced facts on a fresh rebuild: {summary:?}"
    );
}

#[test]
fn a_second_rebuild_duplicates_nothing() {
    let (state, _company_id) = seed_state();

    let first = run_rebuild_fundamentals(&state).expect("first rebuild");
    let second = run_rebuild_fundamentals(&state).expect("second rebuild");

    // The per-tier totals are identical — the second run re-observes every slot.
    assert_eq!(
        first.facts_by_tier, second.facts_by_tier,
        "the fact totals per tier are unchanged by a re-run: {first:?} vs {second:?}"
    );

    // And every write path reports zero new facts on the second pass.
    assert_eq!(
        second.aggregator.facts_written, 0,
        "no new aggregator facts"
    );
    assert_eq!(second.esef_facts_emitted, 0, "no new ESEF facts");
    assert_eq!(second.wdf_facts_written, 0, "no new WDF facts");
    assert!(
        second.aggregator.facts_reobserved > 0,
        "the aggregator re-observes its slots: {second:?}"
    );
    assert!(
        second.esef_facts_reobserved > 0,
        "the ESEF re-extraction re-observes its slots: {second:?}"
    );
}

#[test]
fn per_tier_counts_are_the_sum_across_passes() {
    let (state, company_id) = seed_state();

    let summary = run_rebuild_fundamentals(&state).expect("rebuild runs");

    // The verdict's per-tier counts equal the provenance rows actually stored —
    // the summary is a faithful reading of the store, not an internal tally.
    let breakdown = state.count_facts_by_tier().expect("count");
    assert_eq!(
        summary.facts_by_tier, breakdown,
        "the verdict matches a fresh store read"
    );
    // Every stored fact belongs to one of the three rebuilt tiers.
    let total: i64 = breakdown.by_tier.iter().map(|entry| entry.facts).sum();
    assert!(total > 0, "the rebuild stored facts: {breakdown:?}");
    let stored = state
        .list_financial_facts(crate::storage::ListFinancialFactsInput {
            company_id: Some(company_id),
            period_id: None,
            definition_id: None,
        })
        .expect("facts");
    assert_eq!(
        total + breakdown.manual_or_unprovenanced,
        stored.len() as i64,
        "every stored fact is accounted for in the verdict"
    );
    assert!(summary.errors.is_empty(), "no pass errored: {summary:?}");
}

// ---------------------------------------------------------------------------
// Zero-effects invariant (epic #40 S5, ADR 0091)
// ---------------------------------------------------------------------------

/// A rebuild that walked passes and wrote no fact must NAME why — from its own
/// counters, or by delegating to the embedded pass-1 pull's reason. A rebuild
/// verdict reading "0 facts" with nothing beside it is the dishonest-success
/// class ADR 0091 exists to kill.
#[test]
fn every_zero_effect_rebuild_state_names_a_reason() {
    use crate::effects_honesty::{EffectVerdict, ExplainsEffect};
    use crate::jobs::aggregator_fundamentals_pull::{
        aggregator_effect_reason, AggregatorPullSummary,
    };
    use rebuild_effect_reason as reason;

    let with = |mutate: fn(&mut RebuildFundamentalsSummary)| {
        let mut summary = RebuildFundamentalsSummary {
            esef_documents_processed: 8,
            ..RebuildFundamentalsSummary::default()
        };
        mutate(&mut summary);
        summary.effect_verdict()
    };

    assert_eq!(
        with(|s| s.errors.push("aggregator pull failed: locked".to_owned())),
        EffectVerdict::NothingProduced {
            reason: reason::PASS_ERRORS
        }
    );
    assert_eq!(
        with(|s| s.esef_errors = 3),
        EffectVerdict::NothingProduced {
            reason: reason::PASS_ERRORS
        }
    );
    assert_eq!(
        with(|s| s.esef_facts_reobserved = 40),
        EffectVerdict::NothingProduced {
            reason: reason::ALREADY_RECORDED
        }
    );
    assert_eq!(
        with(|s| s.esef_divergences = 2),
        EffectVerdict::NothingProduced {
            reason: reason::ALREADY_RECORDED
        }
    );
    assert_eq!(
        with(|s| {
            s.wdf_carriers_scanned = 450;
            s.wdf_skipped_no_body = 448;
        }),
        EffectVerdict::NothingProduced {
            reason: reason::WDF_BODY_PRUNED
        }
    );

    // Pass 1 owns the only reason: the rebuild delegates rather than restating
    // the aggregator's vocabulary in its own words.
    let delegated = RebuildFundamentalsSummary {
        aggregator: AggregatorPullSummary {
            companies: 12,
            pages_unavailable: 24,
            ..AggregatorPullSummary::default()
        },
        ..RebuildFundamentalsSummary::default()
    };
    assert_eq!(
        delegated.effect_verdict(),
        EffectVerdict::NothingProduced {
            reason: aggregator_effect_reason::PAGES_UNAVAILABLE
        }
    );

    // Effects from any pass.
    assert_eq!(with(|s| s.esef_facts_emitted = 1), EffectVerdict::Produced);
    assert_eq!(with(|s| s.wdf_facts_written = 1), EffectVerdict::Produced);

    // Nothing anywhere to rebuild.
    assert_eq!(
        RebuildFundamentalsSummary::default().effect_verdict(),
        EffectVerdict::NoInputs
    );
}
