use super::*;

// The source URLs for the live adapters come from each adapter module; the
// disabled-placeholder URLs are pinned here as an independent oracle (they no
// longer live as constants in `storage::mod`, since the catalog now sources them
// from the source-adapter registry — ADR 0050).
use crate::source_adapters::bankier_calendar::SOURCE_URL as BANKIER_CALENDAR_SOURCE_URL;
use crate::source_adapters::bankier_company::SOURCE_URL as BANKIER_COMPANY_SOURCE_URL;
use crate::source_adapters::bankier_rss::SOURCE_URL as BANKIER_RSS_SOURCE_URL;
use crate::source_adapters::gpw_market_events::SOURCE_URL as GPW_MARKET_EVENTS_SOURCE_URL;
use crate::source_adapters::newconnect_company_directory::SOURCE_URL as NEWCONNECT_DIRECTORY_SOURCE_URL;

const PORTAL_ANALIZ_SOURCE_URL: &str = "https://portalanaliz.pl/";
const BANKIER_FIRMA_RSS_SOURCE_URL: &str = "https://www.bankier.pl/rss/firma.xml";
const BANKIER_WIADOMOSCI_RSS_SOURCE_URL: &str = "https://www.bankier.pl/rss/wiadomosci.xml";
const STREFA_REPORT_CALENDAR_SOURCE_URL: &str = "https://strefainwestorow.pl/dane/raporty";
const MONEY_CALENDAR_SOURCE_URL: &str = "https://www.money.pl/gielda/raporty/";

#[test]
fn lists_seeded_source_adapters() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let adapters = state
        .list_source_adapters()
        .expect("source adapters should list");

    // The DB-seeded rows must match the code registry's non-developer set
    // (expected counts are DERIVED from `REGISTRY`, never hand-counted).
    // Registry↔seed *content* parity is guarded separately by
    // `registry_matches_seeded_catalog`.
    let non_developer_expected = crate::source_adapters::registry::REGISTRY
        .iter()
        .filter(|descriptor| {
            descriptor.visibility != crate::source_adapters::registry::SourceVisibility::Developer
        })
        .count();
    assert_eq!(adapters.len(), non_developer_expected);

    assert!(adapters
        .iter()
        .all(|adapter| adapter.visibility != "developer"));

    // The reconciliation witness: user-visible, enabled, and carries the witness
    // role so the Sources UI renders it as a health mechanism, not a feed.
    let witness = adapters
        .iter()
        .find(|adapter| adapter.id == "gpw-espi-ebi")
        .expect("gpw-espi-ebi witness should be user-visible");
    assert_eq!(witness.visibility, "optional");
    assert_eq!(witness.role, "witness");
    assert!(witness.enabled);
    assert!(witness.user_configurable);

    // The ownership breadth source (v0.56 T4, ADR 0072 §2c as amended 2026-07-16):
    // user-visible, enabled, and PRIMARY — it writes a full-picture aggregator
    // basis (the disclosed reports/ESPI witness it). `source_type` is "ownership"
    // (migration 0087 renamed the DB-seeded "ownership_witness").
    let ownership_source = adapters
        .iter()
        .find(|adapter| adapter.id == "biznesradar-akcjonariat")
        .expect("biznesradar-akcjonariat ownership source should be user-visible");
    assert_eq!(ownership_source.visibility, "optional");
    assert_eq!(ownership_source.role, "primary");
    assert_eq!(ownership_source.source_type, "ownership");
    assert!(ownership_source.enabled);

    let yahoo_eod = adapters
        .iter()
        .find(|adapter| adapter.id == "yahoo-eod")
        .expect("yahoo-eod adapter should exist");
    assert_eq!(yahoo_eod.display_name, "Yahoo Finance EOD Quotes");
    assert_eq!(yahoo_eod.source_type, "market_data");
    assert_eq!(yahoo_eod.fetch_mode, "public_json");
    assert_eq!(yahoo_eod.markets, vec!["GPW".to_owned()]);
    assert!(yahoo_eod.enabled);
    assert_eq!(yahoo_eod.visibility, "optional");

    // disclosure source: KNF short-selling register (v0.55, ADR 0069 decision 3).
    let knf_shorts = adapters
        .iter()
        .find(|adapter| adapter.id == "knf-short-selling")
        .expect("knf-short-selling adapter should exist");
    assert_eq!(knf_shorts.display_name, "KNF Short Selling Register");
    assert_eq!(knf_shorts.source_type, "disclosure");
    assert_eq!(knf_shorts.fetch_mode, "public_json");
    assert_eq!(knf_shorts.markets, vec!["GPW".to_owned()]);
    assert!(knf_shorts.enabled);
    assert_eq!(knf_shorts.visibility, "optional");

    // analyst-recommendation source: BiznesRadar Rekomendacje (v0.58 A1, ADR 0073).
    let recommendations = adapters
        .iter()
        .find(|adapter| adapter.id == "biznesradar-rekomendacje")
        .expect("biznesradar-rekomendacje adapter should exist");
    assert_eq!(recommendations.display_name, "BiznesRadar Rekomendacje");
    assert_eq!(recommendations.source_type, "analyst_recommendation");
    assert_eq!(recommendations.fetch_mode, "public_page");
    assert_eq!(recommendations.markets, vec!["GPW".to_owned()]);
    assert_eq!(recommendations.role, "primary");
    assert_eq!(recommendations.visibility, "optional");

    let developer_adapters = state
        .list_source_adapters_with_developer(true)
        .expect("developer source adapters should list");

    // Full catalog = the code registry, derived (guardrail 2026-07-19 — see the
    // non-developer assertion above).
    assert_eq!(
        developer_adapters.len(),
        crate::source_adapters::registry::REGISTRY.len()
    );

    let report_adapter = developer_adapters
        .iter()
        .find(|adapter| adapter.id == "gpw-espi-ebi")
        .expect("GPW report adapter should exist");
    assert_eq!(report_adapter.display_name, "GPW ESPI/EBI");
    assert_eq!(report_adapter.markets, vec!["GPW".to_owned()]);
    // v0.55 T3: re-enabled reconciliation witness, promoted to Optional.
    assert!(report_adapter.enabled);
    assert_eq!(report_adapter.visibility, "optional");
    assert!(report_adapter.user_configurable);

    let registry_adapter = adapters
        .iter()
        .find(|adapter| adapter.id == "gpw-company-registry")
        .expect("GPW registry adapter should exist");
    assert_eq!(registry_adapter.display_name, "GPW Company Directory");
    assert_eq!(registry_adapter.markets, vec!["GPW".to_owned()]);
    assert!(registry_adapter.enabled);
    assert_eq!(registry_adapter.visibility, "required");
    assert!(!registry_adapter.user_configurable);

    let newconnect_directory_adapter = adapters
        .iter()
        .find(|adapter| adapter.id == NEWCONNECT_DIRECTORY_ADAPTER_ID)
        .expect("NewConnect directory adapter should exist");
    assert_eq!(
        newconnect_directory_adapter.display_name,
        "NewConnect Company Directory"
    );
    assert_eq!(newconnect_directory_adapter.source_type, "company_registry");
    assert_eq!(newconnect_directory_adapter.fetch_mode, "public_page");
    assert_eq!(
        newconnect_directory_adapter.source_url,
        NEWCONNECT_DIRECTORY_SOURCE_URL
    );
    assert_eq!(
        newconnect_directory_adapter.markets,
        vec!["NEWCONNECT".to_owned()]
    );
    assert!(newconnect_directory_adapter.enabled);
    assert_eq!(newconnect_directory_adapter.visibility, "required");
    assert!(!newconnect_directory_adapter.user_configurable);

    let bankier_adapter = adapters
        .iter()
        .find(|adapter| adapter.id == BANKIER_RSS_ADAPTER_ID)
        .expect("Bankier RSS adapter should exist");
    assert_eq!(bankier_adapter.display_name, "Bankier Giełda RSS");
    assert_eq!(bankier_adapter.source_type, "public_media");
    assert_eq!(bankier_adapter.fetch_mode, "rss");
    assert_eq!(bankier_adapter.source_url, BANKIER_RSS_SOURCE_URL);
    assert_eq!(bankier_adapter.markets, vec!["GPW".to_owned()]);
    assert!(bankier_adapter.enabled);
    assert_eq!(bankier_adapter.visibility, "optional");
    assert!(bankier_adapter.user_configurable);

    let bankier_company_adapter = adapters
        .iter()
        .find(|adapter| adapter.id == BANKIER_COMPANY_ADAPTER_ID)
        .expect("Bankier company adapter should exist");
    assert_eq!(
        bankier_company_adapter.display_name,
        "Bankier Company Komunikaty"
    );
    assert_eq!(bankier_company_adapter.source_type, "official_report");
    assert_eq!(bankier_company_adapter.fetch_mode, "public_json");
    assert_eq!(
        bankier_company_adapter.source_url,
        BANKIER_COMPANY_SOURCE_URL
    );
    assert_eq!(bankier_company_adapter.markets, vec!["GPW".to_owned()]);
    assert!(bankier_company_adapter.enabled);
    assert!(bankier_company_adapter
        .policy_note
        .contains("active v1 official-report source"));

    let gpw_events_adapter = adapters
        .iter()
        .find(|adapter| adapter.id == GPW_MARKET_EVENTS_ADAPTER_ID)
        .expect("GPW market events adapter should exist");
    assert_eq!(gpw_events_adapter.display_name, "GPW Market Events RSS");
    assert_eq!(gpw_events_adapter.source_type, "official_calendar");
    assert_eq!(gpw_events_adapter.fetch_mode, "rss");
    assert_eq!(gpw_events_adapter.source_url, GPW_MARKET_EVENTS_SOURCE_URL);
    assert_eq!(gpw_events_adapter.markets, vec!["GPW".to_owned()]);
    assert!(gpw_events_adapter.enabled);
    assert!(gpw_events_adapter.policy_note.contains("exact ticker"));

    let bankier_calendar_adapter = adapters
        .iter()
        .find(|adapter| adapter.id == "bankier-kalendarium-html")
        .expect("Bankier Kalendarium adapter should exist");
    assert_eq!(bankier_calendar_adapter.display_name, "Bankier Kalendarium");
    assert_eq!(bankier_calendar_adapter.source_type, "public_calendar");
    assert_eq!(bankier_calendar_adapter.fetch_mode, "public_page");
    assert_eq!(
        bankier_calendar_adapter.source_url,
        BANKIER_CALENDAR_SOURCE_URL
    );
    assert_eq!(bankier_calendar_adapter.markets, vec!["GPW".to_owned()]);
    assert!(bankier_calendar_adapter.enabled);
    assert!(bankier_calendar_adapter
        .policy_note
        .contains("Active M9 public calendar source"));

    let strefa_calendar_adapter = developer_adapters
        .iter()
        .find(|adapter| adapter.id == "strefa-report-calendar")
        .expect("Strefa report calendar placeholder should exist");
    assert_eq!(
        strefa_calendar_adapter.display_name,
        "Strefa Report Calendar"
    );
    assert_eq!(strefa_calendar_adapter.source_type, "public_calendar");
    assert_eq!(strefa_calendar_adapter.fetch_mode, "public_page");
    assert_eq!(
        strefa_calendar_adapter.source_url,
        STREFA_REPORT_CALENDAR_SOURCE_URL
    );
    assert_eq!(strefa_calendar_adapter.markets, vec!["GPW".to_owned()]);
    assert!(!strefa_calendar_adapter.enabled);
    assert!(strefa_calendar_adapter
        .policy_note
        .contains("periodic-report publication dates"));

    let money_calendar_adapter = developer_adapters
        .iter()
        .find(|adapter| adapter.id == "money-calendar")
        .expect("Money calendar placeholder should exist");
    assert_eq!(money_calendar_adapter.display_name, "Money Calendar");
    assert_eq!(money_calendar_adapter.source_type, "public_calendar");
    assert_eq!(money_calendar_adapter.fetch_mode, "public_page");
    assert_eq!(money_calendar_adapter.source_url, MONEY_CALENDAR_SOURCE_URL);
    assert_eq!(money_calendar_adapter.markets, vec!["GPW".to_owned()]);
    assert!(!money_calendar_adapter.enabled);
    assert!(money_calendar_adapter
        .policy_note
        .contains("Fallback/cross-check candidate"));

    let bankier_firma_adapter = developer_adapters
        .iter()
        .find(|adapter| adapter.id == "bankier-firma-rss")
        .expect("Bankier Firma RSS placeholder should exist");
    assert_eq!(bankier_firma_adapter.display_name, "Bankier Firma RSS");
    assert_eq!(bankier_firma_adapter.source_type, "public_media");
    assert_eq!(bankier_firma_adapter.fetch_mode, "rss");
    assert_eq!(
        bankier_firma_adapter.source_url,
        BANKIER_FIRMA_RSS_SOURCE_URL
    );
    assert!(!bankier_firma_adapter.enabled);
    assert!(bankier_firma_adapter
        .policy_note
        .contains("matching-quality tests"));

    let bankier_wiadomosci_adapter = developer_adapters
        .iter()
        .find(|adapter| adapter.id == "bankier-wiadomosci-rss")
        .expect("Bankier Wiadomosci RSS placeholder should exist");
    assert_eq!(
        bankier_wiadomosci_adapter.display_name,
        "Bankier Wiadomosci RSS"
    );
    assert_eq!(bankier_wiadomosci_adapter.source_type, "public_media");
    assert_eq!(bankier_wiadomosci_adapter.fetch_mode, "rss");
    assert_eq!(
        bankier_wiadomosci_adapter.source_url,
        BANKIER_WIADOMOSCI_RSS_SOURCE_URL
    );
    assert!(!bankier_wiadomosci_adapter.enabled);
    assert!(bankier_wiadomosci_adapter
        .policy_note
        .contains("unsuitable for default v1 ingestion"));

    let portal_analiz_adapter = developer_adapters
        .iter()
        .find(|adapter| adapter.id == PORTAL_ANALIZ_ADAPTER_ID)
        .expect("Portal Analiz placeholder should exist");
    assert_eq!(portal_analiz_adapter.display_name, "Portal Analiz");
    assert_eq!(portal_analiz_adapter.source_type, "authenticated_research");
    assert_eq!(portal_analiz_adapter.fetch_mode, "authenticated");
    assert_eq!(portal_analiz_adapter.source_url, PORTAL_ANALIZ_SOURCE_URL);
    assert_eq!(portal_analiz_adapter.markets, vec!["GPW".to_owned()]);
    assert!(!portal_analiz_adapter.enabled);
    assert!(portal_analiz_adapter
        .policy_note
        .contains("Late-v1 planned"));
}

#[test]
fn records_source_adapter_error_state() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    state
        .record_source_adapter_error(ADAPTER_ID, "network timeout")
        .expect("source adapter error should record");

    let adapters = state
        .list_source_adapters_with_developer(true)
        .expect("source adapters should list");
    let adapter = adapters
        .iter()
        .find(|adapter| adapter.id == ADAPTER_ID)
        .expect("GPW adapter should exist");

    assert_eq!(adapter.last_error.as_deref(), Some("network timeout"));
    assert!(adapter.last_error_at.is_some());
}

#[test]
fn updates_optional_source_enabled_state_and_protects_other_tiers() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let updated = state
        .set_source_adapter_enabled(BANKIER_RSS_ADAPTER_ID, false)
        .expect("optional source should be configurable");

    assert_eq!(updated.id, BANKIER_RSS_ADAPTER_ID);
    assert!(!updated.enabled);
    assert_eq!(updated.visibility, "optional");
    assert_eq!(updated.health_status, "off");

    let adapters = state
        .list_source_adapters()
        .expect("source adapters should list");
    let bankier = adapters
        .iter()
        .find(|adapter| adapter.id == BANKIER_RSS_ADAPTER_ID)
        .expect("optional source should remain visible");
    assert!(!bankier.enabled);

    let required_result = state.set_source_adapter_enabled(GPW_REGISTRY_ADAPTER_ID, false);
    assert!(required_result.is_err());

    let newconnect_required_result =
        state.set_source_adapter_enabled(NEWCONNECT_DIRECTORY_ADAPTER_ID, false);
    assert!(newconnect_required_result.is_err());

    // gpw-espi-ebi is now Optional (witness role, v0.55 T3); use a still-Developer
    // source to assert Developer-tier toggles stay protected.
    let developer_result = state.set_source_adapter_enabled("portal-analiz", true);
    assert!(developer_result.is_err());
}

#[test]
fn records_source_adapter_attempt_state() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    state
        .record_source_adapter_attempt(ADAPTER_ID, "scheduler")
        .expect("source adapter attempt should record");

    let adapters = state
        .list_source_adapters_with_developer(true)
        .expect("source adapters should list");
    let adapter = adapters
        .iter()
        .find(|adapter| adapter.id == ADAPTER_ID)
        .expect("GPW adapter should exist");

    assert!(adapter.last_attempt_at.is_some());
    assert_eq!(adapter.last_trigger.as_deref(), Some("scheduler"));
}

/// Drift guard (ADR 0050 / ADR 0045): the source-adapter registry is the SSOT
/// for catalog metadata, but `source_type`, `fetch_mode`, and `markets` are also
/// seeded into the database by migrations. This binds the two so the Rust
/// registry and the seed migrations cannot silently diverge — every catalog row
/// must match its registry descriptor field-for-field.
#[test]
fn registry_matches_seeded_catalog() {
    use crate::source_adapters::registry as adapter_registry;

    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let adapters = state
        .list_source_adapters_with_developer(true)
        .expect("source adapters should list");

    // Every seeded adapter has a registry descriptor, and vice versa.
    assert_eq!(adapters.len(), adapter_registry::REGISTRY.len());

    for adapter in &adapters {
        let descriptor = adapter_registry::descriptor(&adapter.id).unwrap_or_else(|| {
            panic!(
                "catalog adapter {} missing a registry descriptor",
                adapter.id
            )
        });

        assert_eq!(
            adapter.display_name, descriptor.display_name,
            "{}",
            adapter.id
        );
        assert_eq!(adapter.source_url, descriptor.source_url, "{}", adapter.id);
        assert_eq!(
            adapter.source_type, descriptor.source_type,
            "{}",
            adapter.id
        );
        assert_eq!(adapter.fetch_mode, descriptor.fetch_mode, "{}", adapter.id);
        assert_eq!(
            adapter.rate_limit_policy, descriptor.rate_limit_policy,
            "{}",
            adapter.id
        );
        assert_eq!(
            adapter.policy_note, descriptor.policy_note,
            "{}",
            adapter.id
        );
        assert_eq!(
            adapter.visibility,
            descriptor.visibility.as_str(),
            "{}",
            adapter.id
        );
        assert_eq!(
            adapter.markets,
            descriptor
                .markets
                .iter()
                .map(|m| (*m).to_owned())
                .collect::<Vec<_>>(),
            "{}",
            adapter.id
        );
    }
}
